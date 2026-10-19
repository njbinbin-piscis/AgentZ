// Clean-room implementations of the concrete vscode.* runtime types extensions
// instantiate (Position, Range, Uri, CompletionItem, ...). Positions are
// 0-indexed, matching the VS Code API contract.

import * as dto from "../common/dto";

export class Disposable {
  static from(...disposables: { dispose(): unknown }[]): Disposable {
    return new Disposable(() => {
      for (const d of disposables) {
        try {
          if (d && typeof d.dispose === "function") d.dispose();
        } catch {
          /* ignore */
        }
      }
    });
  }
  private callback: (() => unknown) | undefined;
  constructor(callback: () => unknown) {
    this.callback = callback;
  }
  dispose(): void {
    if (this.callback) {
      this.callback();
      this.callback = undefined;
    }
  }
}

export interface Event<T> {
  (listener: (e: T) => unknown, thisArgs?: unknown, disposables?: { dispose(): unknown }[]): Disposable;
}

export class EventEmitter<T> {
  private listeners: { fn: (e: T) => unknown; thisArgs?: unknown }[] = [];
  readonly event: Event<T> = (listener, thisArgs, disposables) => {
    const entry = { fn: listener, thisArgs };
    this.listeners.push(entry);
    const d = new Disposable(() => {
      const i = this.listeners.indexOf(entry);
      if (i >= 0) this.listeners.splice(i, 1);
    });
    if (Array.isArray(disposables)) {
      disposables.push(d);
    }
    return d;
  };
  fire(data: T): void {
    for (const l of this.listeners.slice()) {
      try {
        l.fn.call(l.thisArgs, data);
      } catch {
        /* swallow listener errors */
      }
    }
  }
  dispose(): void {
    this.listeners = [];
  }
}

export class CancellationError extends Error {
  constructor() {
    super("Canceled");
    this.name = "Canceled";
  }
}

export class CancellationTokenSource {
  private emitter = new EventEmitter<void>();
  private _cancelled = false;
  readonly token: {
    readonly isCancellationRequested: boolean;
    readonly onCancellationRequested: Event<void>;
  };
  constructor() {
    const self = this;
    this.token = {
      get isCancellationRequested() {
        return self._cancelled;
      },
      onCancellationRequested: this.emitter.event,
    };
  }
  cancel(): void {
    if (!this._cancelled) {
      this._cancelled = true;
      this.emitter.fire();
    }
  }
  dispose(): void {
    this.emitter.dispose();
  }
}

export class Uri implements dto.UriComponents {
  static parse(value: string): Uri {
    const m = /^([a-zA-Z][a-zA-Z0-9+.-]*):(\/\/([^/?#]*))?([^?#]*)(\?([^#]*))?(#(.*))?$/.exec(value);
    if (!m) return new Uri("file", "", value, "", "");
    return new Uri(m[1] || "", m[3] || "", m[4] || "", m[6] || "", m[8] || "");
  }
  static file(path: string): Uri {
    let p = path.replace(/\\/g, "/");
    if (p[0] !== "/") p = "/" + p;
    return new Uri("file", "", p, "", "");
  }
  static joinPath(base: Uri, ...segments: string[]): Uri {
    const path = [base.path.replace(/\/$/, ""), ...segments].join("/");
    return base.with({ path });
  }
  static from(c: dto.UriComponents): Uri {
    return new Uri(c.scheme, c.authority ?? "", c.path, c.query ?? "", c.fragment ?? "");
  }
  constructor(
    public readonly scheme: string,
    public readonly authority: string,
    public readonly path: string,
    public readonly query: string,
    public readonly fragment: string,
  ) {}
  get fsPath(): string {
    let p = this.path;
    if (/^\/[a-zA-Z]:/.test(p)) {
      p = p.slice(1);
    }
    if (/^[a-zA-Z]:/.test(p) && typeof process !== "undefined" && process.platform === "win32") {
      return p.replace(/\//g, "\\");
    }
    return p;
  }
  with(change: { scheme?: string; authority?: string; path?: string; query?: string; fragment?: string }): Uri {
    return new Uri(
      change.scheme ?? this.scheme,
      change.authority ?? this.authority,
      change.path ?? this.path,
      change.query ?? this.query,
      change.fragment ?? this.fragment,
    );
  }
  toString(): string {
    let r = `${this.scheme}://${this.authority}${this.path}`;
    if (this.query) r += `?${this.query}`;
    if (this.fragment) r += `#${this.fragment}`;
    return r;
  }
  toJSON(): dto.UriComponents {
    return { scheme: this.scheme, authority: this.authority, path: this.path, query: this.query, fragment: this.fragment };
  }
}

export class Position {
  constructor(public readonly line: number, public readonly character: number) {}
  isBefore(other: Position): boolean {
    return this.line < other.line || (this.line === other.line && this.character < other.character);
  }
  isBeforeOrEqual(other: Position): boolean {
    return this.isBefore(other) || this.isEqual(other);
  }
  isAfter(other: Position): boolean {
    return other.isBefore(this);
  }
  isAfterOrEqual(other: Position): boolean {
    return this.isAfter(other) || this.isEqual(other);
  }
  isEqual(other: Position): boolean {
    return this.line === other.line && this.character === other.character;
  }
  compareTo(other: Position): number {
    if (this.line < other.line) return -1;
    if (this.line > other.line) return 1;
    if (this.character < other.character) return -1;
    if (this.character > other.character) return 1;
    return 0;
  }
  translate(lineDeltaOrChange?: number | { lineDelta?: number; characterDelta?: number }, characterDelta = 0): Position {
    if (typeof lineDeltaOrChange === "object" && lineDeltaOrChange !== null) {
      return new Position(
        this.line + (lineDeltaOrChange.lineDelta ?? 0),
        this.character + (lineDeltaOrChange.characterDelta ?? 0),
      );
    }
    return new Position(this.line + (typeof lineDeltaOrChange === "number" ? lineDeltaOrChange : 0), this.character + characterDelta);
  }
  with(lineOrChange?: number | { line?: number; character?: number }, character = this.character): Position {
    if (typeof lineOrChange === "object" && lineOrChange !== null) {
      return new Position(lineOrChange.line ?? this.line, lineOrChange.character ?? this.character);
    }
    return new Position(typeof lineOrChange === "number" ? lineOrChange : this.line, character);
  }
}

export class Range {
  readonly start: Position;
  readonly end: Position;
  constructor(startLine: number | Position, startChar: number | Position, endLine?: number, endChar?: number) {
    if (startLine instanceof Position && startChar instanceof Position) {
      if (startLine.isAfter(startChar)) {
        this.start = startChar;
        this.end = startLine;
      } else {
        this.start = startLine;
        this.end = startChar;
      }
    } else {
      this.start = new Position(startLine as number, startChar as number);
      this.end = new Position(endLine as number, endChar as number);
    }
  }
  get isEmpty(): boolean {
    return this.start.isEqual(this.end);
  }
  get isSingleLine(): boolean {
    return this.start.line === this.end.line;
  }
  contains(positionOrRange: Position | Range): boolean {
    if (positionOrRange instanceof Range) {
      return this.contains(positionOrRange.start) && this.contains(positionOrRange.end);
    }
    return !positionOrRange.isBefore(this.start) && !positionOrRange.isAfter(this.end);
  }
  isEqual(other: Range): boolean {
    return !!other && this.start.isEqual(other.start) && this.end.isEqual(other.end);
  }
  intersection(range: Range): Range | undefined {
    const start = this.start.isAfter(range.start) ? this.start : range.start;
    const end = this.end.isBefore(range.end) ? this.end : range.end;
    if (start.isAfter(end)) return undefined;
    return new Range(start, end);
  }
  union(other: Range): Range {
    const start = this.start.isBefore(other.start) ? this.start : other.start;
    const end = this.end.isAfter(other.end) ? this.end : other.end;
    return new Range(start, end);
  }
  with(startOrChange?: Position | { start?: Position; end?: Position }, end = this.end): Range {
    if (startOrChange && !(startOrChange instanceof Position) && typeof startOrChange === "object") {
      return new Range(startOrChange.start ?? this.start, startOrChange.end ?? this.end);
    }
    return new Range((startOrChange as Position) ?? this.start, end);
  }
}

export class Selection extends Range {
  readonly anchor: Position;
  readonly active: Position;
  constructor(
    anchorLineOrPos: number | Position,
    anchorCharOrPos: number | Position,
    activeLine?: number,
    activeChar?: number,
  ) {
    const anchor =
      anchorLineOrPos instanceof Position
        ? anchorLineOrPos
        : new Position(anchorLineOrPos, anchorCharOrPos as number);
    const active =
      anchorCharOrPos instanceof Position
        ? anchorCharOrPos
        : new Position(activeLine ?? 0, activeChar ?? 0);
    super(anchor, active);
    this.anchor = anchor;
    this.active = active;
  }
  get isReversed(): boolean {
    return this.anchor.isEqual(this.end) && !this.isEmpty;
  }
}

export class Location {
  constructor(public uri: Uri, public range: Range) {}
}

export class MarkdownString {
  isTrusted?: boolean | { readonly enabledCommands: readonly string[] };
  supportThemeIcons?: boolean;
  supportHtml?: boolean;
  baseUri?: Uri;
  constructor(public value = "", supportThemeIcons = false) {
    this.supportThemeIcons = supportThemeIcons;
  }
  appendText(value: string): MarkdownString {
    this.value += value;
    return this;
  }
  appendMarkdown(value: string): MarkdownString {
    this.value += value;
    return this;
  }
  appendCodeblock(code: string, lang = ""): MarkdownString {
    this.value += `\n\`\`\`${lang}\n${code}\n\`\`\`\n`;
    return this;
  }
}

export enum CompletionItemKind {
  Text = 0, Method = 1, Function = 2, Constructor = 3, Field = 4, Variable = 5,
  Class = 6, Interface = 7, Module = 8, Property = 9, Unit = 10, Value = 11,
  Enum = 12, Keyword = 13, Snippet = 14, Color = 15, File = 16, Reference = 17,
  Folder = 18, EnumMember = 19, Constant = 20, Struct = 21, Event = 22,
  Operator = 23, TypeParameter = 24, User = 25, Issue = 26,
}

export enum CompletionItemTag {
  Deprecated = 1,
}

export enum CompletionTriggerKind {
  Invoke = 0,
  TriggerCharacter = 1,
  TriggerForIncompleteCompletions = 2,
}

export class CompletionItem {
  detail?: string;
  documentation?: string | MarkdownString;
  sortText?: string;
  filterText?: string;
  insertText?: string | SnippetString;
  range?: Range | { inserting: Range; replacing: Range };
  commitCharacters?: string[];
  preselect?: boolean;
  additionalTextEdits?: TextEdit[];
  command?: { command: string; title: string; arguments?: unknown[] };
  tags?: readonly CompletionItemTag[];
  constructor(public label: string | { label: string; detail?: string; description?: string }, public kind?: CompletionItemKind) {}
}

export class CompletionList {
  constructor(public items: CompletionItem[] = [], public isIncomplete = false) {}
}

export class SnippetString {
  constructor(public value = "") {}
  appendText(s: string): SnippetString {
    this.value += s;
    return this;
  }
  appendTabstop(n = 0): SnippetString {
    this.value += `$${n}`;
    return this;
  }
  appendPlaceholder(s: string | ((snippet: SnippetString) => unknown), n = 0): SnippetString {
    if (typeof s === "function") {
      const nested = new SnippetString();
      s(nested);
      this.value += `\${${n}:${nested.value}}`;
    } else {
      this.value += `\${${n}:${s}}`;
    }
    return this;
  }
  appendChoice(values: readonly string[], n = 0): SnippetString {
    this.value += `\${${n}|${values.join(",")}|}`;
    return this;
  }
  appendVariable(name: string, defaultValue?: string | ((snippet: SnippetString) => unknown)): SnippetString {
    if (typeof defaultValue === "string") {
      this.value += `\${${name}:${defaultValue}}`;
    } else {
      this.value += `$${name}`;
    }
    return this;
  }
}

export class SnippetTextEdit {
  static replace(range: Range, snippet: SnippetString): SnippetTextEdit {
    return new SnippetTextEdit(range, snippet);
  }
  static insert(position: Position, snippet: SnippetString): SnippetTextEdit {
    return new SnippetTextEdit(new Range(position, position), snippet);
  }
  constructor(public range: Range, public snippet: SnippetString) {}
}

export class Hover {
  contents: (MarkdownString | string)[];
  constructor(contents: MarkdownString | string | (MarkdownString | string)[], public range?: Range) {
    this.contents = Array.isArray(contents) ? contents : [contents];
  }
}

export enum DiagnosticSeverity {
  Error = 0,
  Warning = 1,
  Information = 2,
  Hint = 3,
}

export enum DiagnosticTag {
  Unnecessary = 1,
  Deprecated = 2,
}

export class DiagnosticRelatedInformation {
  constructor(public location: Location, public message: string) {}
}

export class Diagnostic {
  source?: string;
  code?: string | number | { value: string | number; target: Uri };
  relatedInformation?: DiagnosticRelatedInformation[];
  tags?: DiagnosticTag[];
  constructor(public range: Range, public message: string, public severity = DiagnosticSeverity.Error) {}
}

export class TextEdit {
  static replace(range: Range, newText: string): TextEdit {
    return new TextEdit(range, newText);
  }
  static insert(position: Position, newText: string): TextEdit {
    return new TextEdit(new Range(position, position), newText);
  }
  static delete(range: Range): TextEdit {
    return new TextEdit(range, "");
  }
  static setEndOfLine(eol: EndOfLine): TextEdit {
    const edit = new TextEdit(new Range(0, 0, 0, 0), "");
    (edit as { eol?: EndOfLine }).eol = eol;
    return edit;
  }
  constructor(public range: Range, public newText: string) {}
}

export class WorkspaceEdit {
  private map = new Map<string, TextEdit[]>();
  replace(uri: Uri, range: Range, newText: string): void {
    this.push(uri, new TextEdit(range, newText));
  }
  insert(uri: Uri, position: Position, newText: string): void {
    this.push(uri, TextEdit.insert(position, newText));
  }
  delete(uri: Uri, range: Range): void {
    this.push(uri, TextEdit.delete(range));
  }
  has(uri: Uri): boolean {
    return this.map.has(uri.toString());
  }
  set(uri: Uri, edits: readonly (TextEdit | SnippetTextEdit)[] | null | undefined): void {
    if (!edits) {
      this.map.delete(uri.toString());
      return;
    }
    const normalized = edits.map((e) =>
      e instanceof SnippetTextEdit ? new TextEdit(e.range, e.snippet.value) : e,
    );
    this.map.set(uri.toString(), normalized);
  }
  get(uri: Uri): TextEdit[] {
    return this.map.get(uri.toString()) ?? [];
  }
  createFile(_uri: Uri, _options?: { overwrite?: boolean; ignoreIfExists?: boolean; contents?: Uint8Array }): void {
    /* recorded as file op */
  }
  deleteFile(_uri: Uri, _options?: { recursive?: boolean; ignoreIfNotExists?: boolean }): void {
    /* recorded as file op */
  }
  renameFile(_oldUri: Uri, _newUri: Uri, _options?: { overwrite?: boolean; ignoreIfExists?: boolean }): void {
    /* recorded as file op */
  }
  get size(): number {
    return this.map.size;
  }
  private push(uri: Uri, edit: TextEdit): void {
    const key = uri.toString();
    const arr = this.map.get(key) ?? [];
    arr.push(edit);
    this.map.set(key, arr);
  }
  entries(): [Uri, TextEdit[]][] {
    return [...this.map.entries()].map(([k, v]) => [Uri.parse(k), v]);
  }
}

export class CodeActionKind {
  static readonly Empty = new CodeActionKind("");
  static readonly QuickFix = new CodeActionKind("quickfix");
  static readonly Refactor = new CodeActionKind("refactor");
  static readonly RefactorExtract = new CodeActionKind("refactor.extract");
  static readonly RefactorInline = new CodeActionKind("refactor.inline");
  static readonly RefactorMove = new CodeActionKind("refactor.move");
  static readonly RefactorRewrite = new CodeActionKind("refactor.rewrite");
  static readonly Source = new CodeActionKind("source");
  static readonly SourceOrganizeImports = new CodeActionKind("source.organizeImports");
  static readonly SourceFixAll = new CodeActionKind("source.fixAll");
  static readonly Notebook = new CodeActionKind("notebook");
  constructor(public readonly value: string) {}
  append(parts: string): CodeActionKind {
    return new CodeActionKind(this.value ? `${this.value}.${parts}` : parts);
  }
  intersects(other: CodeActionKind): boolean {
    return this.contains(other) || other.contains(this);
  }
  contains(other: CodeActionKind): boolean {
    return this.value === other.value || other.value.startsWith(`${this.value}.`);
  }
}

export class CodeAction {
  kind?: CodeActionKind;
  diagnostics?: Diagnostic[];
  edit?: WorkspaceEdit;
  command?: { command: string; title: string; arguments?: unknown[] };
  isPreferred?: boolean;
  disabled?: { readonly reason: string };
  constructor(public title: string, kind?: CodeActionKind) {
    this.kind = kind;
  }
}

export class CodeLens {
  constructor(
    public range: Range,
    public command?: { title: string; command: string; tooltip?: string; arguments?: unknown[] },
  ) {}
  get isResolved(): boolean {
    return !!this.command;
  }
}

export enum DocumentHighlightKind {
  Text = 0,
  Read = 1,
  Write = 2,
}

export class DocumentHighlight {
  constructor(public range: Range, public kind: DocumentHighlightKind = DocumentHighlightKind.Text) {}
}

export class DocumentLink {
  tooltip?: string;
  constructor(public range: Range, public target?: Uri) {}
}

export enum SymbolKind {
  File = 0, Module = 1, Namespace = 2, Package = 3, Class = 4, Method = 5,
  Property = 6, Field = 7, Constructor = 8, Enum = 9, Interface = 10,
  Function = 11, Variable = 12, Constant = 13, String = 14, Number = 15,
  Boolean = 16, Array = 17, Object = 18, Key = 19, Null = 20, EnumMember = 21,
  Struct = 22, Event = 23, Operator = 24, TypeParameter = 25,
}

export enum SymbolTag {
  Deprecated = 1,
}

export class DocumentSymbol {
  children: DocumentSymbol[] = [];
  detail = "";
  tags?: readonly SymbolTag[];
  constructor(public name: string, detail: string, public kind: SymbolKind, public range: Range, public selectionRange: Range) {
    this.detail = detail;
  }
}

export class SymbolInformation {
  location: Location;
  containerName: string;
  tags?: SymbolTag[];
  constructor(
    public name: string,
    public kind: SymbolKind,
    containerNameOrRange: string | Range,
    locationOrUri?: Location | Uri,
    containerName?: string,
  ) {
    if (locationOrUri instanceof Location) {
      this.location = locationOrUri;
      this.containerName = typeof containerNameOrRange === "string" ? containerNameOrRange : "";
    } else if (locationOrUri instanceof Uri && containerNameOrRange instanceof Range) {
      this.location = new Location(locationOrUri, containerNameOrRange);
      this.containerName = containerName ?? "";
    } else {
      this.location = new Location(
        Uri.file(""),
        containerNameOrRange instanceof Range ? containerNameOrRange : new Range(0, 0, 0, 0),
      );
      this.containerName = typeof containerNameOrRange === "string" ? containerNameOrRange : "";
    }
  }
}

export class CallHierarchyItem {
  tags?: readonly SymbolTag[];
  constructor(
    public kind: SymbolKind,
    public name: string,
    public detail: string,
    public uri: Uri,
    public range: Range,
    public selectionRange: Range,
  ) {}
}

export class CallHierarchyIncomingCall {
  constructor(public from: CallHierarchyItem, public fromRanges: Range[]) {}
}

export class CallHierarchyOutgoingCall {
  constructor(public to: CallHierarchyItem, public fromRanges: Range[]) {}
}

export class TypeHierarchyItem {
  tags?: readonly SymbolTag[];
  constructor(
    public kind: SymbolKind,
    public name: string,
    public detail: string,
    public uri: Uri,
    public range: Range,
    public selectionRange: Range,
  ) {}
}

export enum InlayHintKind {
  Type = 1,
  Parameter = 2,
}

export class InlayHintLabelPart {
  tooltip?: string | MarkdownString;
  location?: Location;
  command?: { title: string; command: string; arguments?: unknown[] };
  constructor(public value: string) {}
}

export class InlayHint {
  tooltip?: string | MarkdownString;
  paddingLeft?: boolean;
  paddingRight?: boolean;
  textEdits?: TextEdit[];
  constructor(
    public position: Position,
    public label: string | InlayHintLabelPart[],
    public kind?: InlayHintKind,
  ) {}
}

export enum FoldingRangeKind {
  Comment = 1,
  Imports = 2,
  Region = 3,
}

export class FoldingRange {
  constructor(public start: number, public end: number, public kind?: FoldingRangeKind) {}
}

export class SelectionRange {
  constructor(public range: Range, public parent?: SelectionRange) {}
}

export class Color {
  constructor(
    public readonly red: number,
    public readonly green: number,
    public readonly blue: number,
    public readonly alpha: number,
  ) {}
}

export class ColorInformation {
  constructor(public range: Range, public color: Color) {}
}

export class ColorPresentation {
  textEdit?: TextEdit;
  additionalTextEdits?: TextEdit[];
  constructor(public label: string) {}
}

export class InlineValueText {
  constructor(public readonly range: Range, public readonly text: string) {}
}

export class InlineValueVariableLookup {
  constructor(
    public readonly range: Range,
    public readonly variableName?: string,
    public readonly caseSensitiveLookup: boolean = true,
  ) {}
}

export class InlineValueEvaluatableExpression {
  constructor(public readonly range: Range, public readonly expression?: string) {}
}

export enum InlineCompletionTriggerKind {
  Invoke = 0,
  Automatic = 1,
}

export class InlineCompletionItem {
  filterText?: string;
  range?: Range;
  command?: { title: string; command: string; arguments?: unknown[] };
  constructor(
    public insertText: string | SnippetString,
    range?: Range,
    command?: { title: string; command: string; arguments?: unknown[] },
  ) {
    this.range = range;
    this.command = command;
  }
}

export class InlineCompletionList {
  constructor(public items: InlineCompletionItem[] = []) {}
}

export class LinkedEditingRanges {
  constructor(public readonly ranges: Range[], public readonly wordPattern?: RegExp) {}
}

export class SemanticTokensLegend {
  constructor(
    public readonly tokenTypes: string[],
    public readonly tokenModifiers: string[] = [],
  ) {}
}

export class SemanticTokens {
  constructor(public readonly data: Uint32Array, public readonly resultId?: string) {}
}

export class SemanticTokensEdit {
  constructor(
    public readonly start: number,
    public readonly deleteCount: number,
    public readonly data?: Uint32Array,
  ) {}
}

export class SemanticTokensEdits {
  constructor(
    public readonly edits: SemanticTokensEdit[],
    public readonly resultId?: string,
  ) {}
}

export class SemanticTokensBuilder {
  private _data: number[] = [];
  constructor(public readonly legend?: SemanticTokensLegend) {}
  push(
    lineOrRange: number | Range,
    char?: number,
    length?: number,
    tokenType?: number | string,
    tokenModifiers?: number | string[],
  ): void {
    void lineOrRange;
    void char;
    void length;
    void tokenType;
    void tokenModifiers;
  }
  build(resultId?: string): SemanticTokens {
    return new SemanticTokens(new Uint32Array(this._data), resultId);
  }
}

export class SignatureHelp {
  signatures: SignatureInformation[] = [];
  activeSignature = 0;
  activeParameter = 0;
}
export class SignatureInformation {
  parameters: ParameterInformation[] = [];
  documentation?: string | MarkdownString;
  activeParameter?: number;
  constructor(public label: string, documentation?: string | MarkdownString) {
    this.documentation = documentation;
  }
}
export class ParameterInformation {
  constructor(public label: string | [number, number], public documentation?: string | MarkdownString) {}
}

export enum IndentAction {
  None = 0,
  Indent = 1,
  IndentOutdent = 2,
  Outdent = 3,
}

export enum InputBoxValidationSeverity {
  Info = 1,
  Warning = 2,
  Error = 3,
}

export enum StatusBarAlignment {
  Left = 1,
  Right = 2,
}

export enum ConfigurationTarget {
  Global = 1,
  Workspace = 2,
  WorkspaceFolder = 3,
}

export enum ViewColumn {
  Active = -1,
  Beside = -2,
  One = 1,
  Two = 2,
  Three = 3,
  Four = 4,
  Five = 5,
  Six = 6,
  Seven = 7,
  Eight = 8,
  Nine = 9,
}

export enum TreeItemCollapsibleState {
  None = 0,
  Collapsed = 1,
  Expanded = 2,
}

export class ThemeIcon {
  static readonly File = new ThemeIcon("file");
  static readonly Folder = new ThemeIcon("folder");
  constructor(public readonly id: string, public readonly color?: ThemeColor) {}
}

export class ThemeColor {
  constructor(public readonly id: string) {}
}

export class TreeItem {
  label?: string | { label: string; highlights?: [number, number][] };
  description?: string | boolean;
  tooltip?: string | MarkdownString;
  iconPath?: ThemeIcon | Uri | string | { light: Uri | string; dark: Uri | string };
  command?: { command: string; title: string; arguments?: unknown[] };
  contextValue?: string;
  resourceUri?: Uri;
  id?: string;
  collapsibleState: TreeItemCollapsibleState = TreeItemCollapsibleState.None;
  constructor(
    labelOrUri: string | { label: string; highlights?: [number, number][] } | Uri,
    collapsibleState?: TreeItemCollapsibleState,
  ) {
    if (labelOrUri instanceof Uri) {
      this.resourceUri = labelOrUri;
    } else if (typeof labelOrUri === "string") {
      this.label = labelOrUri;
    } else if (labelOrUri && typeof labelOrUri === "object" && "label" in labelOrUri) {
      this.label = labelOrUri.label;
    }
    if (collapsibleState !== undefined) this.collapsibleState = collapsibleState;
  }
}

export class RelativePattern {
  public readonly baseUri: Uri;
  public readonly base: string;
  constructor(base: string | Uri | { uri: Uri }, public pattern: string) {
    if (typeof base === "string") {
      this.baseUri = Uri.file(base);
      this.base = base;
    } else if (base && typeof base === "object" && "uri" in base && base.uri) {
      this.baseUri = base.uri;
      this.base = base.uri.fsPath;
    } else if (base && typeof base === "object" && "fsPath" in base) {
      this.baseUri = base as Uri;
      this.base = (base as Uri).fsPath;
    } else {
      this.baseUri = Uri.file("");
      this.base = "";
    }
  }
}

export enum FileType {
  Unknown = 0,
  File = 1,
  Directory = 2,
  SymbolicLink = 64,
}

export enum EndOfLine {
  LF = 1,
  CRLF = 2,
}

export enum TextEditorRevealType {
  Default = 0,
  InCenter = 1,
  InCenterIfOutsideViewport = 2,
  AtTop = 3,
}

export enum QuickPickItemKind {
  Separator = -1,
  Default = 0,
}

export enum ProgressLocation {
  SourceControl = 1,
  Window = 10,
  Notification = 15,
}

export enum LogLevel {
  Off = 0,
  Trace = 1,
  Debug = 2,
  Info = 3,
  Warning = 4,
  Error = 5,
}

export enum UIKind {
  Desktop = 1,
  Web = 2,
}

export enum ExtensionKind {
  UI = 1,
  Workspace = 2,
}

export enum ExtensionMode {
  Production = 1,
  Development = 2,
  Test = 3,
}

export enum TextDocumentSaveReason {
  Manual = 1,
  AfterDelay = 2,
  FocusOut = 3,
}

export enum OverviewRulerLane {
  Left = 1,
  Center = 2,
  Right = 4,
  Full = 7,
}

export enum CodeActionTriggerKind {
  Invoke = 1,
  Automatic = 2,
}

export enum SignatureHelpTriggerKind {
  Invoke = 1,
  TriggerCharacter = 2,
  ContentChange = 3,
}

export enum NotebookCellKind {
  Markup = 1,
  Code = 2,
}

export enum NotebookCellStatusBarAlignment {
  Left = 1,
  Right = 2,
}

export enum NotebookControllerAffinity {
  Default = 1,
  Preferred = 2,
}

export class NotebookRange {
  get isEmpty(): boolean {
    return this.start === this.end;
  }
  constructor(public readonly start: number, public readonly end: number) {}
  with(change: { start?: number; end?: number }): NotebookRange {
    return new NotebookRange(change.start ?? this.start, change.end ?? this.end);
  }
}

export class NotebookCellData {
  metadata?: Record<string, unknown>;
  executionSummary?: unknown;
  outputs?: unknown[];
  constructor(
    public kind: NotebookCellKind,
    public value: string,
    public languageId: string,
  ) {}
}

export class NotebookData {
  metadata?: Record<string, unknown>;
  constructor(public cells: NotebookCellData[]) {}
}

export class NotebookEdit {
  static replaceCells(range: NotebookRange, newCells: NotebookCellData[]): NotebookEdit {
    return new NotebookEdit(range, newCells);
  }
  static insertCells(index: number, newCells: NotebookCellData[]): NotebookEdit {
    return new NotebookEdit(new NotebookRange(index, index), newCells);
  }
  static deleteCells(range: NotebookRange): NotebookEdit {
    return new NotebookEdit(range, []);
  }
  constructor(public range: NotebookRange, public newCells: NotebookCellData[]) {}
}

export enum TaskRevealKind {
  Always = 1,
  Silent = 2,
  Never = 3,
}

export enum TaskPanelKind {
  Shared = 1,
  Dedicated = 2,
  New = 3,
}

export enum TaskScope {
  Global = 1,
  Workspace = 2,
}

export enum ShellQuoting {
  Escape = 1,
  Strong = 2,
  Weak = 3,
}

export class TaskGroup {
  static readonly Clean = new TaskGroup("clean", "Clean");
  static readonly Build = new TaskGroup("build", "Build");
  static readonly Rebuild = new TaskGroup("rebuild", "Rebuild");
  static readonly Test = new TaskGroup("test", "Test");
  isDefault?: boolean;
  constructor(public readonly id: string, public readonly label: string) {}
}

export class ProcessExecution {
  args: string[];
  options?: Record<string, unknown>;
  constructor(
    public process: string,
    argsOrOptions?: string[] | Record<string, unknown>,
    options?: Record<string, unknown>,
  ) {
    if (Array.isArray(argsOrOptions)) {
      this.args = argsOrOptions;
      this.options = options;
    } else {
      this.args = [];
      this.options = argsOrOptions;
    }
  }
}

export class ShellExecution {
  args?: (string | { value: string; quoting: ShellQuoting })[];
  options?: Record<string, unknown>;
  commandLine?: string;
  command?: string | { value: string; quoting: ShellQuoting };
  constructor(
    commandLineOrCommand: string | { value: string; quoting: ShellQuoting },
    argsOrOptions?: (string | { value: string; quoting: ShellQuoting })[] | Record<string, unknown>,
    options?: Record<string, unknown>,
  ) {
    if (Array.isArray(argsOrOptions)) {
      this.command = commandLineOrCommand;
      this.args = argsOrOptions;
      this.options = options;
    } else {
      this.commandLine =
        typeof commandLineOrCommand === "string" ? commandLineOrCommand : commandLineOrCommand.value;
      this.options = argsOrOptions;
    }
  }
}

export class CustomExecution {
  constructor(public readonly callback: (resolvedDefinition: Record<string, unknown>) => Thenable<unknown>) {}
}

export class Task {
  group?: TaskGroup;
  detail?: string;
  presentationOptions: Record<string, unknown> = {};
  isBackground = false;
  runOptions: Record<string, unknown> = {};
  constructor(
    public definition: Record<string, unknown>,
    public scope: unknown,
    public name: string,
    public source: string,
    public execution?: ProcessExecution | ShellExecution | CustomExecution,
    public problemMatchers: string[] = [],
  ) {}
}

export enum TestRunProfileKind {
  Run = 1,
  Debug = 2,
  Coverage = 3,
}

export class TestTag {
  constructor(public readonly id: string) {}
}

export class TestMessage {
  expectedOutput?: string;
  actualOutput?: string;
  location?: Location;
  contextValue?: string;
  static diff(message: string | MarkdownString, expected: string, actual: string): TestMessage {
    const m = new TestMessage(message);
    m.expectedOutput = expected;
    m.actualOutput = actual;
    return m;
  }
  constructor(public message: string | MarkdownString) {}
}

export class TestRunRequest {
  constructor(
    public readonly include?: readonly unknown[],
    public readonly exclude?: readonly unknown[],
    public readonly profile?: unknown,
    public readonly continuous?: boolean,
    public readonly preserveFocus: boolean = true,
  ) {}
}

export class TabInputText {
  constructor(public readonly uri: Uri) {}
}

export class TabInputTextDiff {
  constructor(public readonly original: Uri, public readonly modified: Uri) {}
}

export class TabInputCustom {
  constructor(public readonly uri: Uri, public readonly viewType: string) {}
}

export class TabInputWebview {
  constructor(public readonly viewType: string) {}
}

export class TabInputNotebook {
  constructor(public readonly uri: Uri, public readonly notebookType: string) {}
}

export class TabInputNotebookDiff {
  constructor(
    public readonly original: Uri,
    public readonly modified: Uri,
    public readonly notebookType: string,
  ) {}
}

export class TabInputTerminal {
  constructor() {}
}

export class DataTransferItem {
  constructor(public readonly value: unknown) {}
  asString(): Thenable<string> {
    return Promise.resolve(typeof this.value === "string" ? this.value : JSON.stringify(this.value));
  }
  asFile(): undefined {
    return undefined;
  }
}

export class DataTransfer implements Iterable<[string, DataTransferItem]> {
  private map = new Map<string, DataTransferItem>();
  get(mimeType: string): DataTransferItem | undefined {
    return this.map.get(mimeType.toLowerCase());
  }
  set(mimeType: string, value: DataTransferItem): void {
    this.map.set(mimeType.toLowerCase(), value);
  }
  forEach(cb: (item: DataTransferItem, mimeType: string) => void): void {
    this.map.forEach((v, k) => cb(v, k));
  }
  [Symbol.iterator](): Iterator<[string, DataTransferItem]> {
    return this.map.entries();
  }
}

export class FileDecoration {
  propagate?: boolean;
  constructor(
    public badge?: string,
    public tooltip?: string,
    public color?: ThemeColor,
  ) {}
}

export class TerminalLink {
  constructor(
    public startIndex: number,
    public length: number,
    public tooltip?: string,
  ) {}
}

export class Breakpoint {
  readonly id: string = `bp-${Math.random().toString(36).slice(2)}`;
  constructor(
    public readonly enabled: boolean = true,
    public readonly condition?: string,
    public readonly hitCondition?: string,
    public readonly logMessage?: string,
  ) {}
}

export class SourceBreakpoint extends Breakpoint {
  constructor(
    public readonly location: Location,
    enabled?: boolean,
    condition?: string,
    hitCondition?: string,
    logMessage?: string,
  ) {
    super(enabled, condition, hitCondition, logMessage);
  }
}

export class FunctionBreakpoint extends Breakpoint {
  constructor(
    public readonly functionName: string,
    enabled?: boolean,
    condition?: string,
    hitCondition?: string,
    logMessage?: string,
  ) {
    super(enabled, condition, hitCondition, logMessage);
  }
}

export class DebugAdapterExecutable {
  constructor(
    public readonly command: string,
    public readonly args: string[] = [],
    public readonly options?: Record<string, unknown>,
  ) {}
}

export class DebugAdapterServer {
  constructor(public readonly port: number, public readonly host?: string) {}
}

export class DebugAdapterNamedPipeServer {
  constructor(public readonly path: string) {}
}

export class DebugAdapterInlineImplementation {
  constructor(public readonly implementation: unknown) {}
}

export enum DebugConsoleMode {
  Separate = 0,
  MergeWithParent = 1,
}

export enum LanguageStatusSeverity {
  Information = 0,
  Warning = 1,
  Error = 2,
}

export enum ColorThemeKind {
  Light = 1,
  Dark = 2,
  HighContrast = 3,
  HighContrastLight = 4,
}

export enum EnvironmentVariableMutatorType {
  Replace = 1,
  Append = 2,
  Prepend = 3,
}

export class FileSystemError extends Error {
  readonly code: string;

  constructor(messageOrUri?: string | Uri, code = "Unknown") {
    const msg =
      typeof messageOrUri === "string"
        ? messageOrUri
        : messageOrUri
          ? `${code}: ${messageOrUri.toString()}`
          : code;
    super(msg);
    this.code = code;
    this.name = `FileSystemError (${code})`;
  }

  static FileNotFound(messageOrUri?: string | Uri): FileSystemError {
    return new FileSystemError(messageOrUri, "FileNotFound");
  }

  static FileExists(messageOrUri?: string | Uri): FileSystemError {
    return new FileSystemError(messageOrUri, "FileExists");
  }

  static FileNotADirectory(messageOrUri?: string | Uri): FileSystemError {
    return new FileSystemError(messageOrUri, "FileNotADirectory");
  }

  static FileIsADirectory(messageOrUri?: string | Uri): FileSystemError {
    return new FileSystemError(messageOrUri, "FileIsADirectory");
  }

  static NoPermissions(messageOrUri?: string | Uri): FileSystemError {
    return new FileSystemError(messageOrUri, "NoPermissions");
  }

  static Unavailable(messageOrUri?: string | Uri): FileSystemError {
    return new FileSystemError(messageOrUri, "Unavailable");
  }
}
