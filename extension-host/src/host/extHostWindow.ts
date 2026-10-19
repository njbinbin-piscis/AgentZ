// ExtHostWindow: messages, status bar items, output channels, quick input, and
// terminals. Each delegates to its MainThread* counterpart on the renderer.

import { IRPCProtocol } from "../common/proxyIdentifier";
import {
  MainContext,
  MainThreadMessageServiceShape,
  MainThreadOutputShape,
  MainThreadQuickOpenShape,
  MainThreadStatusBarShape,
  MainThreadTerminalShape,
} from "../common/protocol";
import {
  CancellationTokenSource,
  Disposable,
  EventEmitter,
  LogLevel,
  StatusBarAlignment,
} from "./types-impl";

export type StatusBarCommandValue =
  | string
  | { command: string; title?: string; arguments?: unknown[] }
  | undefined;

export class StatusBarItem {
  private _text = "";
  private _tooltip?: string | { value?: string };
  private _command?: StatusBarCommandValue;
  private _color?: string | { id?: string };
  private _backgroundColor?: string | { id?: string };
  private _name?: string;
  private _accessibilityInformation?: { label: string; role?: string };
  private visible = false;

  constructor(
    public readonly id: string,
    public readonly alignment: StatusBarAlignment,
    public readonly priority: number,
    private readonly proxy: MainThreadStatusBarShape,
  ) {}

  get text(): string {
    return this._text;
  }
  set text(v: string) {
    this._text = v ?? "";
    this.sync();
  }

  get tooltip(): string | { value?: string } | undefined {
    return this._tooltip;
  }
  set tooltip(v: string | { value?: string } | undefined) {
    this._tooltip = v;
    this.sync();
  }

  get command(): StatusBarCommandValue {
    return this._command;
  }
  set command(v: StatusBarCommandValue) {
    this._command = v;
    this.sync();
  }

  get color(): string | { id?: string } | undefined {
    return this._color;
  }
  set color(v: string | { id?: string } | undefined) {
    this._color = v;
    this.sync();
  }

  get backgroundColor(): string | { id?: string } | undefined {
    return this._backgroundColor;
  }
  set backgroundColor(v: string | { id?: string } | undefined) {
    this._backgroundColor = v;
    this.sync();
  }

  get name(): string | undefined {
    return this._name;
  }
  set name(v: string | undefined) {
    this._name = v;
    this.sync();
  }

  get accessibilityInformation(): { label: string; role?: string } | undefined {
    return this._accessibilityInformation;
  }
  set accessibilityInformation(v: { label: string; role?: string } | undefined) {
    this._accessibilityInformation = v;
  }

  show(): void {
    this.visible = true;
    this.sync();
  }
  hide(): void {
    this.visible = false;
    this.proxy.$dispose(this.id);
  }
  private sync(): void {
    if (!this.visible) return;
    const tooltipStr =
      typeof this._tooltip === "string"
        ? this._tooltip
        : this._tooltip && typeof this._tooltip === "object" && typeof this._tooltip.value === "string"
          ? this._tooltip.value
          : undefined;
    const commandId =
      typeof this._command === "string"
        ? this._command
        : this._command && typeof this._command === "object"
          ? this._command.command
          : undefined;
    const commandArgs =
      this._command && typeof this._command === "object" && Array.isArray(this._command.arguments)
        ? this._command.arguments
        : undefined;
    const colorStr =
      typeof this._color === "string"
        ? this._color
        : this._color && typeof this._color === "object"
          ? this._color.id
          : undefined;
    const bgColorStr =
      typeof this._backgroundColor === "string"
        ? this._backgroundColor
        : this._backgroundColor && typeof this._backgroundColor === "object"
          ? this._backgroundColor.id
          : undefined;
    this.proxy.$setEntry({
      id: this.id,
      text: this._text,
      tooltip: tooltipStr,
      command: commandId,
      arguments: commandArgs,
      alignment: this.alignment,
      priority: this.priority,
      color: colorStr,
      backgroundColor: bgColorStr,
      name: this._name,
    });
  }
  dispose(): void {
    this.visible = false;
    this.proxy.$dispose(this.id);
  }
}

export class OutputChannel {
  readonly logLevel = LogLevel.Info;
  private readonly logLevelEmitter = new EventEmitter<LogLevel>();
  readonly onDidChangeLogLevel = this.logLevelEmitter.event;

  constructor(
    public readonly name: string,
    private readonly id: string,
    private readonly proxy: MainThreadOutputShape,
  ) {
    this.proxy.$register(id, name);
  }
  append(value: string): void {
    this.proxy.$append(this.id, value);
  }
  appendLine(value: string): void {
    this.proxy.$append(this.id, value + "\n");
  }
  trace(message: string, ...args: unknown[]): void {
    this.appendLine(`[trace] ${message}${args.length ? " " + args.map(String).join(" ") : ""}`);
  }
  debug(message: string, ...args: unknown[]): void {
    this.appendLine(`[debug] ${message}${args.length ? " " + args.map(String).join(" ") : ""}`);
  }
  info(message: string, ...args: unknown[]): void {
    this.appendLine(`[info] ${message}${args.length ? " " + args.map(String).join(" ") : ""}`);
  }
  warn(message: string, ...args: unknown[]): void {
    this.appendLine(`[warn] ${message}${args.length ? " " + args.map(String).join(" ") : ""}`);
  }
  error(error: string | Error, ...args: unknown[]): void {
    const msg = error instanceof Error ? error.stack ?? error.message : String(error);
    this.appendLine(`[error] ${msg}${args.length ? " " + args.map(String).join(" ") : ""}`);
  }
  clear(): void {
    this.proxy.$clear(this.id);
  }
  show(preserveFocus?: boolean): void {
    this.proxy.$show(this.id, !!preserveFocus);
  }
  hide(): void {
    /* no-op */
  }
  replace(value: string): void {
    this.clear();
    this.append(value);
  }
  dispose(): void {
    this.logLevelEmitter.dispose();
  }
}

export class Terminal {
  readonly processId: Promise<number | undefined> = Promise.resolve(undefined);
  readonly state = { isInteractedWith: false };
  exitStatus: { readonly code: number | undefined; readonly reason: number } | undefined = undefined;

  constructor(
    public readonly name: string,
    private readonly id: string,
    private readonly proxy: MainThreadTerminalShape,
    public readonly creationOptions: Record<string, unknown> = {},
    private readonly onDisposeCallback?: (term: Terminal) => void,
  ) {}
  sendText(text: string, addNewLine = true): void {
    this.proxy.$sendText(this.id, text, addNewLine);
  }
  show(preserveFocus?: boolean): void {
    void preserveFocus;
    this.proxy.$show(this.id);
  }
  hide(): void {
    /* no-op */
  }
  dispose(): void {
    this.exitStatus = { code: 0, reason: 1 };
    this.proxy.$dispose(this.id);
    this.onDisposeCallback?.(this);
  }
}

export class ExtHostWindow {
  private readonly msgProxy: MainThreadMessageServiceShape;
  private readonly statusProxy: MainThreadStatusBarShape;
  private readonly outputProxy: MainThreadOutputShape;
  private readonly quickProxy: MainThreadQuickOpenShape;
  private readonly terminalProxy: MainThreadTerminalShape;
  private statusSeq = 0;
  private quickSeq = 0;
  private readonly _terminals: Terminal[] = [];
  private _activeTerminal: Terminal | undefined;

  readonly onDidOpenTerminalEmitter = new EventEmitter<Terminal>();
  readonly onDidCloseTerminalEmitter = new EventEmitter<Terminal>();
  readonly onDidChangeActiveTerminalEmitter = new EventEmitter<Terminal | undefined>();
  readonly onDidChangeTerminalStateEmitter = new EventEmitter<Terminal>();

  constructor(rpc: IRPCProtocol) {
    this.msgProxy = rpc.getProxy(MainContext.MainThreadMessageService);
    this.statusProxy = rpc.getProxy(MainContext.MainThreadStatusBar);
    this.outputProxy = rpc.getProxy(MainContext.MainThreadOutput);
    this.quickProxy = rpc.getProxy(MainContext.MainThreadQuickOpen);
    this.terminalProxy = rpc.getProxy(MainContext.MainThreadTerminal);
  }

  get terminals(): readonly Terminal[] {
    return this._terminals;
  }

  get activeTerminal(): Terminal | undefined {
    return this._activeTerminal;
  }

  private async showMessage(severity: number, message: string, rest: unknown[]): Promise<unknown> {
    let options: { modal?: boolean; detail?: string } = {};
    let rawItems: unknown[];
    const first = rest[0];
    if (
      rest.length > 0 &&
      typeof first === "object" &&
      first !== null &&
      !Array.isArray(first) &&
      typeof (first as { title?: unknown }).title !== "string"
    ) {
      options = first as { modal?: boolean; detail?: string };
      rawItems = rest.slice(1);
    } else {
      rawItems = rest;
    }
    const titles = rawItems.map((item) => {
      if (typeof item === "string") return item;
      if (typeof item === "object" && item !== null && typeof (item as { title?: unknown }).title === "string") {
        return (item as { title: string }).title;
      }
      return String(item);
    });
    const pickedTitle = await this.msgProxy.$showMessage(severity, message, options, titles);
    if (pickedTitle === undefined) return undefined;
    const matchedObject = rawItems.find(
      (item) =>
        typeof item === "object" &&
        item !== null &&
        typeof (item as { title?: unknown }).title === "string" &&
        (item as { title: string }).title === pickedTitle,
    );
    return matchedObject !== undefined ? matchedObject : pickedTitle;
  }

  showInformationMessage(message: string, ...rest: unknown[]): Promise<unknown> {
    return this.showMessage(2, message, rest);
  }
  showWarningMessage(message: string, ...rest: unknown[]): Promise<unknown> {
    return this.showMessage(1, message, rest);
  }
  showErrorMessage(message: string, ...rest: unknown[]): Promise<unknown> {
    return this.showMessage(0, message, rest);
  }

  createStatusBarItem(
    idOrAlignment?: string | StatusBarAlignment,
    alignmentOrPriority?: StatusBarAlignment | number,
    priorityArg?: number,
  ): StatusBarItem {
    let id: string;
    let alignment: StatusBarAlignment;
    let priority: number;
    if (typeof idOrAlignment === "string") {
      id = `${idOrAlignment}-${this.statusSeq++}`;
      alignment = (alignmentOrPriority as StatusBarAlignment) ?? StatusBarAlignment.Left;
      priority = priorityArg ?? 0;
    } else {
      id = `status-${this.statusSeq++}`;
      alignment = idOrAlignment ?? StatusBarAlignment.Left;
      priority = (alignmentOrPriority as number) ?? 0;
    }
    return new StatusBarItem(id, alignment, priority, this.statusProxy);
  }

  createOutputChannel(name: string, _optionsOrLanguageId?: string | { log?: boolean }): OutputChannel {
    return new OutputChannel(name, `output-${name}`, this.outputProxy);
  }

  async showQuickPick(
    items: unknown,
    options?: { placeHolder?: string; title?: string; canPickMany?: boolean },
  ): Promise<unknown> {
    const resolved = (await Promise.resolve(items)) as unknown[];
    if (!Array.isArray(resolved)) return undefined;
    const list = resolved.map((it) =>
      typeof it === "string"
        ? { label: it }
        : {
            label: (it as { label: string }).label,
            description: (it as { description?: string }).description,
            detail: (it as { detail?: string }).detail,
            picked: (it as { picked?: boolean }).picked,
            kind: (it as { kind?: number }).kind,
          },
    );
    const placeHolder = options?.placeHolder ?? options?.title;
    const result = await this.quickProxy.$showQuickPick(
      this.quickSeq++,
      list,
      placeHolder,
      !!options?.canPickMany,
    );
    if (!result) return undefined;
    const pickLabel = (r: { label: string; description?: string }) => {
      const original = resolved.find((it) =>
        typeof it === "string"
          ? it === r.label
          : (it as { label: string }).label === r.label &&
            ((it as { description?: string }).description ?? undefined) === (r.description ?? undefined),
      ) ?? resolved.find((it) => (typeof it === "string" ? it === r.label : (it as { label: string }).label === r.label));
      return original ?? r;
    };
    return Array.isArray(result) ? result.map(pickLabel) : pickLabel(result);
  }

  showInputBox(options?: {
    prompt?: string;
    title?: string;
    value?: string;
    placeHolder?: string;
    password?: boolean;
  }): Promise<string | undefined> {
    return this.quickProxy.$showInputBox(
      options?.prompt ?? options?.title,
      options?.value,
      options?.placeHolder,
      !!options?.password,
    );
  }

  createQuickPick<T extends { label: string; description?: string; detail?: string; picked?: boolean; kind?: number }>(): {
    value: string;
    placeholder: string | undefined;
    title: string | undefined;
    items: readonly T[];
    selectedItems: readonly T[];
    activeItems: readonly T[];
    canSelectMany: boolean;
    busy: boolean;
    enabled: boolean;
    ignoreFocusOut: boolean;
    onDidChangeValue: (listener: (v: string) => void) => Disposable;
    onDidAccept: (listener: () => void) => Disposable;
    onDidHide: (listener: () => void) => Disposable;
    onDidChangeSelection: (listener: (items: readonly T[]) => void) => Disposable;
    onDidChangeActive: (listener: (items: readonly T[]) => void) => Disposable;
    show: () => void;
    hide: () => void;
    dispose: () => void;
  } {
    const changeValueEmitter = new EventEmitter<string>();
    const acceptEmitter = new EventEmitter<void>();
    const hideEmitter = new EventEmitter<void>();
    const selectionEmitter = new EventEmitter<readonly T[]>();
    const activeEmitter = new EventEmitter<readonly T[]>();
    const qp = {
      value: "",
      placeholder: undefined as string | undefined,
      title: undefined as string | undefined,
      items: [] as readonly T[],
      selectedItems: [] as readonly T[],
      activeItems: [] as readonly T[],
      canSelectMany: false,
      busy: false,
      enabled: true,
      ignoreFocusOut: false,
      onDidChangeValue: changeValueEmitter.event,
      onDidAccept: acceptEmitter.event,
      onDidHide: hideEmitter.event,
      onDidChangeSelection: selectionEmitter.event,
      onDidChangeActive: activeEmitter.event,
      show: () => {
        void this.showQuickPick(qp.items, {
          placeHolder: qp.placeholder ?? qp.title,
          canPickMany: qp.canSelectMany,
        }).then((picked) => {
          if (picked !== undefined) {
            const arr = (Array.isArray(picked) ? picked : [picked]) as T[];
            qp.selectedItems = arr;
            qp.activeItems = arr;
            selectionEmitter.fire(arr);
            acceptEmitter.fire();
          }
          hideEmitter.fire();
        });
      },
      hide: () => {
        hideEmitter.fire();
      },
      dispose: () => {
        changeValueEmitter.dispose();
        acceptEmitter.dispose();
        hideEmitter.dispose();
        selectionEmitter.dispose();
        activeEmitter.dispose();
      },
    };
    return qp;
  }

  createInputBox(): {
    value: string;
    placeholder: string | undefined;
    prompt: string | undefined;
    title: string | undefined;
    password: boolean;
    busy: boolean;
    enabled: boolean;
    ignoreFocusOut: boolean;
    validationMessage: string | undefined;
    onDidChangeValue: (listener: (v: string) => void) => Disposable;
    onDidAccept: (listener: () => void) => Disposable;
    onDidHide: (listener: () => void) => Disposable;
    show: () => void;
    hide: () => void;
    dispose: () => void;
  } {
    const changeValueEmitter = new EventEmitter<string>();
    const acceptEmitter = new EventEmitter<void>();
    const hideEmitter = new EventEmitter<void>();
    const ib = {
      value: "",
      placeholder: undefined as string | undefined,
      prompt: undefined as string | undefined,
      title: undefined as string | undefined,
      password: false,
      busy: false,
      enabled: true,
      ignoreFocusOut: false,
      validationMessage: undefined as string | undefined,
      onDidChangeValue: changeValueEmitter.event,
      onDidAccept: acceptEmitter.event,
      onDidHide: hideEmitter.event,
      show: () => {
        void this.showInputBox({
          prompt: ib.prompt ?? ib.title,
          value: ib.value,
          placeHolder: ib.placeholder,
          password: ib.password,
        }).then((val) => {
          if (val !== undefined) {
            ib.value = val;
            changeValueEmitter.fire(val);
            acceptEmitter.fire();
          }
          hideEmitter.fire();
        });
      },
      hide: () => {
        hideEmitter.fire();
      },
      dispose: () => {
        changeValueEmitter.dispose();
        acceptEmitter.dispose();
        hideEmitter.dispose();
      },
    };
    return ib;
  }

  createTerminal(nameOrOptions?: string | { name?: string; cwd?: string | { fsPath?: string } }): Terminal {
    const name = typeof nameOrOptions === "string" ? nameOrOptions : nameOrOptions?.name ?? "Extension";
    const id = `term-${Math.random().toString(36).slice(2)}`;
    const opts = typeof nameOrOptions === "object" && nameOrOptions !== null ? nameOrOptions : { name };
    void this.terminalProxy.$createTerminal(id, name);
    const term = new Terminal(name, id, this.terminalProxy, opts, (closed) => {
      const idx = this._terminals.indexOf(closed);
      if (idx >= 0) this._terminals.splice(idx, 1);
      if (this._activeTerminal === closed) {
        this._activeTerminal = this._terminals[this._terminals.length - 1];
        this.onDidChangeActiveTerminalEmitter.fire(this._activeTerminal);
      }
      this.onDidCloseTerminalEmitter.fire(closed);
    });
    this._terminals.push(term);
    this._activeTerminal = term;
    this.onDidOpenTerminalEmitter.fire(term);
    this.onDidChangeActiveTerminalEmitter.fire(term);
    return term;
  }

  setStatusBarMessage(text: string, hideAfterOrThenable?: number | Thenable<unknown>): Disposable {
    const item = this.createStatusBarItem(StatusBarAlignment.Left, 0);
    item.text = text;
    item.show();
    if (typeof hideAfterOrThenable === "number") {
      setTimeout(() => item.dispose(), hideAfterOrThenable);
    } else if (
      hideAfterOrThenable &&
      typeof (hideAfterOrThenable as Thenable<unknown>).then === "function"
    ) {
      void Promise.resolve(hideAfterOrThenable).finally(() => item.dispose());
    }
    return new Disposable(() => item.dispose());
  }

  withProgress<R>(
    _options: unknown,
    task: (progress: { report: (v: unknown) => void }, token: unknown) => Thenable<R>,
  ): Thenable<R> {
    return task({ report: () => undefined }, new CancellationTokenSource().token);
  }
}
