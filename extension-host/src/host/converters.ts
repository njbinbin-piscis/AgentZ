// Host-side converters: rich vscode.* types -> serializable DTOs.

import * as dto from "../common/dto";
import * as types from "./types-impl";

export function fromPosition(p: types.Position): dto.IPosition {
  return { line: p.line, character: p.character };
}

export function toPosition(p: dto.IPosition): types.Position {
  return new types.Position(p.line, p.character);
}

export function fromRange(r: types.Range): dto.IRange {
  return {
    startLine: r.start.line,
    startCharacter: r.start.character,
    endLine: r.end.line,
    endCharacter: r.end.character,
  };
}

export function toRange(r: dto.IRange): types.Range {
  return new types.Range(r.startLine, r.startCharacter, r.endLine, r.endCharacter);
}

export function fromUri(u: types.Uri): dto.UriComponents {
  return u.toJSON();
}

function fromMarkdown(value: string | types.MarkdownString): string | dto.MarkdownStringDto {
  if (typeof value === "string") return value;
  return { value: value.value, isTrusted: Boolean(value.isTrusted), supportThemeIcons: value.supportThemeIcons };
}

export function fromCompletionItem(item: types.CompletionItem): dto.CompletionItemDto {
  const isSnippet = item.insertText instanceof types.SnippetString;
  const insertText = item.insertText instanceof types.SnippetString ? item.insertText.value : item.insertText;
  const labelStr = typeof item.label === "string" ? item.label : item.label.label;
  const detailStr =
    item.detail ?? (typeof item.label === "object" && item.label !== null ? item.label.detail : undefined);
  const resolvedRange = item.range
    ? "inserting" in item.range
      ? item.range.replacing ?? item.range.inserting
      : item.range
    : undefined;
  return {
    label: labelStr,
    kind: item.kind,
    detail: detailStr,
    documentation: item.documentation ? fromMarkdown(item.documentation) : undefined,
    sortText: item.sortText,
    filterText: item.filterText,
    insertText: insertText ?? labelStr,
    insertTextIsSnippet: isSnippet,
    range: resolvedRange ? fromRange(resolvedRange) : undefined,
    commitCharacters: item.commitCharacters,
    preselect: item.preselect,
  };
}

export function fromHover(hover: types.Hover): dto.HoverDto {
  return {
    contents: hover.contents.map(fromMarkdown),
    range: hover.range ? fromRange(hover.range) : undefined,
  };
}

export function fromLocation(loc: types.Location): dto.LocationDto {
  return { uri: fromUri(loc.uri), range: fromRange(loc.range) };
}

export function fromDiagnostic(d: types.Diagnostic): dto.DiagnosticDto {
  const rawCode = d.code;
  const codeVal: string | number | undefined =
    typeof rawCode === "string" || typeof rawCode === "number"
      ? rawCode
      : rawCode?.value;
  return {
    range: fromRange(d.range),
    message: d.message,
    severity: d.severity,
    source: d.source,
    code: codeVal,
    relatedInformation: d.relatedInformation?.map((r) => ({
      location: fromLocation(r.location),
      message: r.message,
    })),
  };
}

export function fromDocumentSymbol(s: types.DocumentSymbol): dto.DocumentSymbolDto {
  return {
    name: s.name,
    detail: s.detail,
    kind: s.kind,
    range: fromRange(s.range),
    selectionRange: fromRange(s.selectionRange),
    children: s.children?.map(fromDocumentSymbol),
  };
}

export function fromTextEdit(e: types.TextEdit): dto.TextEditDto {
  return { range: fromRange(e.range), text: e.newText };
}

export function fromWorkspaceEdit(edit: types.WorkspaceEdit): dto.WorkspaceEditDto {
  return {
    edits: edit.entries().map(([uri, edits]) => ({
      resource: fromUri(uri),
      edits: edits.map(fromTextEdit),
    })),
  };
}

export function fromCodeAction(a: types.CodeAction): dto.CodeActionDto {
  return {
    title: a.title,
    kind: a.kind?.value,
    isPreferred: a.isPreferred,
    diagnostics: a.diagnostics?.map(fromDiagnostic),
    edit: a.edit ? fromWorkspaceEdit(a.edit) : undefined,
    command: a.command ? { id: a.command.command, title: a.command.title, arguments: a.command.arguments } : undefined,
  };
}

export function fromSignatureHelp(h: types.SignatureHelp): dto.SignatureHelpDto {
  return {
    activeSignature: h.activeSignature,
    activeParameter: h.activeParameter,
    signatures: h.signatures.map((s) => ({
      label: s.label,
      documentation: s.documentation ? fromMarkdown(s.documentation) : undefined,
      parameters: s.parameters.map((p) => ({
        label: typeof p.label === "string" ? p.label : String(p.label),
        documentation: p.documentation ? (typeof p.documentation === "string" ? p.documentation : p.documentation.value) : undefined,
      })),
    })),
  };
}

export function fromTreeItem(item: types.TreeItem, handle: string, parentHandle?: string): dto.TreeItemDto {
  let iconId: string | undefined;
  if (item.iconPath instanceof types.ThemeIcon) iconId = item.iconPath.id;
  const labelStr = typeof item.label === "string" ? item.label : item.label?.label;
  const descStr = typeof item.description === "string" ? item.description : undefined;
  const tooltipStr = typeof item.tooltip === "string" ? item.tooltip : item.tooltip?.value;
  return {
    handle,
    parentHandle,
    label: labelStr,
    description: descStr,
    tooltip: tooltipStr,
    iconId,
    collapsibleState: item.collapsibleState,
    contextValue: item.contextValue,
    resourceUri: item.resourceUri ? fromUri(item.resourceUri) : undefined,
    command: item.command
      ? { id: item.command.command, title: item.command.title, arguments: item.command.arguments }
      : undefined,
  };
}
