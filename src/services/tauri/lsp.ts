// LSP (Language Server Protocol) client for the IDE.
// Connects to the Rust backend's WebSocket bridge and wires
// Monaco Editor providers for diagnostics, hover, completions,
// go-to-definition, and find-references.
//
// Protocol note: the Rust bridge owns the Content-Length framing. It accepts
// one bare JSON message per WebSocket text frame, adds the LSP header itself
// before writing to the language server's stdin, and strips the header off
// responses before forwarding them to us. We therefore send raw JSON objects
// (never Content-Length framed) and parse response payloads defensively.

import { invoke } from "@tauri-apps/api/core";
import type * as Monaco from "monaco-editor";
import { fileUriString } from "./editorUri";

// ─── Tauri command wrappers ──────────────────────────────────────────────

export interface LspLanguageInfo {
  language_id: string;
  name: string;
  extensions: string[];
  server_command: string;
  available: boolean;
}

export const lspApi = {
  /** List all supported LSP languages with availability status. */
  listLanguages: () =>
    invoke<LspLanguageInfo[]>("ide_lsp_list_languages"),

  /** Start an LSP server; returns the WebSocket port. */
  start: (projectDir: string, language: string) =>
    invoke<number>("ide_lsp_start", { projectDir, language }),

  /** Stop an LSP session for a given project+language. */
  stop: (projectDir: string, language: string) =>
    invoke<void>("ide_lsp_stop", { projectDir, language }),
};

/** Monaco editor language id (syntax + built-in TS/JS diagnostics). */
const EXT_TO_MONACO_LANG: Record<string, string> = {
  ".ts": "typescript",
  ".tsx": "typescriptreact",
  ".js": "javascript",
  ".jsx": "javascriptreact",
  ".mjs": "javascript",
  ".cjs": "javascript",
  ".rs": "rust",
  ".py": "python",
  ".pyi": "python",
  ".json": "json",
  ".html": "html",
  ".htm": "html",
  ".css": "css",
  ".scss": "scss",
  ".less": "less",
  ".md": "markdown",
  ".sql": "sql",
  ".yaml": "yaml",
  ".yml": "yaml",
  ".xml": "xml",
  ".sh": "shell",
  ".bash": "shell",
};

/** Detect Monaco language from a file path. */
export function monacoLanguageForFile(filePath: string): string | null {
  const lower = filePath.toLowerCase();
  for (const [ext, lang] of Object.entries(EXT_TO_MONACO_LANG)) {
    if (lower.endsWith(ext)) return lang;
  }
  return null;
}

// ─── File extension → LSP language mapping ─────────────────────────────

const EXT_TO_LSP_LANG: Record<string, string> = {
  ".rs": "rust",
  ".ts": "typescript",
  ".tsx": "typescript",
  ".js": "typescript",
  ".jsx": "typescript",
  ".mjs": "typescript",
  ".cjs": "typescript",
  ".py": "python",
  ".pyi": "python",
  ".c": "cpp",
  ".h": "cpp",
  ".cpp": "cpp",
  ".cc": "cpp",
  ".cxx": "cpp",
  ".hpp": "cpp",
  ".hxx": "cpp",
};

/** Detect LSP language from a file path. */
export function languageForFile(filePath: string): string | null {
  const lower = filePath.toLowerCase();
  for (const [ext, lang] of Object.entries(EXT_TO_LSP_LANG)) {
    if (lower.endsWith(ext)) return lang;
  }
  return null;
}

// ─── LSP JSON-RPC types ──────────────────────────────────────────────────

interface LspPosition {
  line: number;
  character: number;
}

interface LspRange {
  start: LspPosition;
  end: LspPosition;
}

interface LspLocation {
  uri: string;
  range: LspRange;
}

interface LspDiagnostic {
  range: LspRange;
  severity?: number; // 1=Error, 2=Warning, 3=Info, 4=Hint
  message: string;
  source?: string;
  code?: string | number;
}

interface LspCompletionItem {
  label: string;
  kind?: number;
  detail?: string;
  documentation?: string | { value: string };
  insertText?: string;
  insertTextFormat?: number; // 2 = snippet
  sortText?: string;
  filterText?: string;
  textEdit?: { range: LspRange; newText: string };
  additionalTextEdits?: { range: LspRange; newText: string }[];
}

// ─── Timings ─────────────────────────────────────────────────────────────

/** Handshake budget for the initialize round-trip. */
const HANDSHAKE_TIMEOUT_MS = 10_000;
/** Budget for read-only queries (hover / completion / definition / references). */
const REQUEST_TIMEOUT_MS = 5_000;
/** The bridge answers `initialize` with a canned response whose id is 0. */
const BRIDGE_INIT_ID = 0;

interface Pending {
  settled: boolean;
  timeoutId: ReturnType<typeof setTimeout>;
  timeoutRejects: boolean;
  settle: (value: unknown) => void;
  fail: (err: Error) => void;
  label: string;
}

// ─── LspClient ───────────────────────────────────────────────────────────

/**
 * Manages a single WebSocket connection to one LSP bridge.
 * Sends bare JSON-RPC messages (the bridge owns Content-Length framing) and
 * correlates responses via numeric request ids. Tracks open documents and
 * diagnostics per URI so one client can serve several files.
 */
export class LspClient {
  private ws: WebSocket | null = null;
  private nextId = 1;
  private pending = new Map<number, Pending>();
  private url: string;
  private diagnosticsByUri = new Map<string, LspDiagnostic[]>();
  private diagnosticsListeners = new Map<string, Set<(d: LspDiagnostic[]) => void>>();
  private docVersions = new Map<string, number>();
  private connectPromise: Promise<void> | null = null;

  constructor(port: number) {
    this.url = `ws://127.0.0.1:${port}`;
  }

  /** Connect to the bridge and perform the LSP handshake. */
  async connect(projectDir: string, language: string): Promise<void> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return;
    if (this.connectPromise) return this.connectPromise;

    this.connectPromise = new Promise<void>((resolve, reject) => {
      const ws = new WebSocket(this.url);
      this.ws = ws;

      ws.onopen = async () => {
        try {
          const rootUri = fileUriString(projectDir);
          await this.sendRequest(
            "initialize",
            {
              processId: null,
              rootUri,
              capabilities: {
                textDocument: {
                  hover: { contentFormat: ["markdown", "plaintext"] },
                  completion: { completionItem: { snippetSupport: true } },
                  definition: { linkSupport: true },
                  references: {},
                  rename: { prepareSupport: true },
                  publishDiagnostics: { relatedInformation: true },
                },
              },
              workspaceFolders: [{ uri: rootUri, name: "project" }],
            },
            {
              label: "initialize",
              timeoutMs: HANDSHAKE_TIMEOUT_MS,
              timeoutRejects: true,
              // The bridge intercepts initialize and replies with id 0.
              aliases: [BRIDGE_INIT_ID],
            },
          );

          this.send({ jsonrpc: "2.0", method: "initialized", params: {} });

          if (import.meta.env.DEV) {
            console.log(`[LSP] Connected to ${this.url} for ${language}`);
          }
          resolve();
        } catch (err) {
          reject(err instanceof Error ? err : new Error(String(err)));
        }
      };

      ws.onmessage = (evt) => {
        this.handleMessage(evt.data as string);
      };

      ws.onerror = () => {
        console.error("[LSP] WebSocket error");
        reject(new Error("WebSocket connection failed"));
      };

      ws.onclose = () => {
        if (import.meta.env.DEV) console.log("[LSP] WebSocket closed");
        this.ws = null;
        this.connectPromise = null;
        // Fail anything still waiting so callers never hang.
        this.rejectAllPending(new Error("LSP disconnected"));
      };
    });

    return this.connectPromise;
  }

  /** Notify the server that a document was opened (resets its version to 1). */
  openDocument(filePath: string, languageId: string, content: string) {
    const uri = fileUriString(filePath);
    this.docVersions.set(uri, 1);
    this.send({
      jsonrpc: "2.0",
      method: "textDocument/didOpen",
      params: {
        textDocument: { uri, languageId, version: 1, text: content },
      },
    });
  }

  /** Notify the server that a document was closed. */
  closeDocument(filePath: string) {
    const uri = fileUriString(filePath);
    this.docVersions.delete(uri);
    this.diagnosticsByUri.delete(uri);
    this.send({
      jsonrpc: "2.0",
      method: "textDocument/didClose",
      params: { textDocument: { uri } },
    });
  }

  /** Send a textDocument/didChange notification with a monotonically
   * increasing per-document version (required by the LSP spec). */
  sendDidChange(filePath: string, content: string) {
    const uri = fileUriString(filePath);
    const version = (this.docVersions.get(uri) ?? 1) + 1;
    this.docVersions.set(uri, version);
    this.send({
      jsonrpc: "2.0",
      method: "textDocument/didChange",
      params: {
        textDocument: { uri, version },
        contentChanges: [{ text: content }],
      },
    });
  }

  /** Request diagnostics for a file (pull model, falling back to cached push). */
  async requestDiagnostics(filePath: string): Promise<LspDiagnostic[]> {
    const uri = fileUriString(filePath);
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      try {
        const result = await this.sendRequest(
          "textDocument/diagnostic",
          { textDocument: { uri } },
          { label: "diagnostic", timeoutMs: 3000 },
        );
        const items = (result as { items?: LspDiagnostic[] } | null)?.items;
        if (items) return items;
      } catch {
        // Server may not implement pull diagnostics — fall back to cache.
      }
    }
    return this.diagnosticsByUri.get(uri) ?? [];
  }

  /** Request hover info at a position. */
  async requestHover(
    filePath: string,
    line: number,
    character: number,
  ): Promise<string | null> {
    const result = await this.sendRequest("textDocument/hover", {
      textDocument: { uri: fileUriString(filePath) },
      position: { line, character },
    });
    const r = result as { contents?: unknown } | undefined;
    if (!r?.contents) return null;
    const c = r.contents as
      | string
      | { value: string }
      | { kind: string; value: string };
    if (typeof c === "string") return c;
    if ("value" in c && typeof c.value === "string") return c.value;
    return JSON.stringify(c);
  }

  /** Request raw completion items at a position. */
  async requestCompletions(
    filePath: string,
    line: number,
    character: number,
  ): Promise<LspCompletionItem[] | null> {
    const result = await this.sendRequest("textDocument/completion", {
      textDocument: { uri: fileUriString(filePath) },
      position: { line, character },
      context: { triggerKind: 1 },
    });
    const r = result as
      | { items?: LspCompletionItem[] }
      | LspCompletionItem[]
      | undefined;
    if (!r) return null;
    const items = Array.isArray(r) ? r : r.items ?? [];
    return items;
  }

  /** Request go-to-definition at a position (raw LSP locations). */
  async requestDefinition(
    filePath: string,
    line: number,
    character: number,
  ): Promise<LspLocation[] | null> {
    const result = await this.sendRequest("textDocument/definition", {
      textDocument: { uri: fileUriString(filePath) },
      position: { line, character },
    });
    if (!result) return null;
    const locations: LspLocation[] = Array.isArray(result)
      ? (result as LspLocation[])
      : [result as LspLocation];
    return locations;
  }

  /** Request find-references at a position (raw LSP locations). */
  async requestReferences(
    filePath: string,
    line: number,
    character: number,
  ): Promise<LspLocation[] | null> {
    const result = await this.sendRequest("textDocument/references", {
      textDocument: { uri: fileUriString(filePath) },
      position: { line, character },
      context: { includeDeclaration: true },
    });
    const locations = result as LspLocation[] | undefined;
    if (!locations?.length) return null;
    return locations;
  }

  /**
   * Subscribe to diagnostics for one document URI. Fires immediately with the
   * last known set (if any). Returns an unsubscribe function.
   */
  setDiagnosticsCallback(
    uri: string,
    cb: (diags: LspDiagnostic[]) => void,
  ): () => void {
    let listeners = this.diagnosticsListeners.get(uri);
    if (!listeners) {
      listeners = new Set();
      this.diagnosticsListeners.set(uri, listeners);
    }
    listeners.add(cb);
    const cached = this.diagnosticsByUri.get(uri);
    if (cached) cb(cached);
    return () => {
      const set = this.diagnosticsListeners.get(uri);
      if (!set) return;
      set.delete(cb);
      if (set.size === 0) this.diagnosticsListeners.delete(uri);
    };
  }

  /** Disconnect and clean up. Fails any in-flight requests. */
  disconnect() {
    this.rejectAllPending(new Error("LSP disconnected"));
    this.docVersions.clear();
    this.diagnosticsByUri.clear();
    this.diagnosticsListeners.clear();
    if (this.ws) {
      try {
        this.ws.close();
      } catch {
        // ignore
      }
      this.ws = null;
    }
    this.connectPromise = null;
  }

  // ─── Private helpers ──────────────────────────────────────────────────

  /** Send one bare JSON message (the bridge adds LSP framing). */
  private send(msg: unknown) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(JSON.stringify(msg));
  }

  /**
   * Register a request and send it. Pending is installed *before* the send so
   * a synchronous response can never be missed. `aliases` lets callers accept a
   * response under an id the server substitutes (e.g. the bridge's canned
   * initialize reply uses id 0).
   */
  private sendRequest(
    method: string,
    params: unknown,
    opts: {
      label?: string;
      timeoutMs?: number;
      timeoutRejects?: boolean;
      aliases?: number[];
    } = {},
  ): Promise<unknown> {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      return Promise.resolve(null);
    }

    const id = this.nextId++;
    const timeoutMs = opts.timeoutMs ?? REQUEST_TIMEOUT_MS;
    const timeoutRejects = opts.timeoutRejects ?? false;

    return new Promise<unknown>((resolve, reject) => {
      const pending: Pending = {
        settled: false,
        timeoutRejects,
        timeoutId: setTimeout(() => {
          if (pending.settled) return;
          pending.settled = true;
          this.removePending(pending);
          if (timeoutRejects) {
            reject(new Error(`LSP request timed out: ${opts.label ?? method}`));
          } else {
            resolve(null);
          }
        }, timeoutMs),
        settle: (value: unknown) => {
          if (pending.settled) return;
          pending.settled = true;
          clearTimeout(pending.timeoutId);
          this.removePending(pending);
          resolve(value);
        },
        fail: (err: Error) => {
          if (pending.settled) return;
          pending.settled = true;
          clearTimeout(pending.timeoutId);
          this.removePending(pending);
          reject(err);
        },
        label: opts.label ?? method,
      };

      this.pending.set(id, pending);
      for (const alias of opts.aliases ?? []) this.pending.set(alias, pending);

      this.send({ jsonrpc: "2.0", id, method, params });
    });
  }

  /** Remove every map entry that points at the given pending record. */
  private removePending(pending: Pending) {
    for (const [key, value] of this.pending) {
      if (value === pending) this.pending.delete(key);
    }
  }

  private rejectAllPending(err: Error) {
    const records = new Set(this.pending.values());
    this.pending.clear();
    for (const pending of records) {
      if (pending.settled) continue;
      // Let fail() flip `settled`, clear the timer and reject. removePending()
      // inside fail() is a no-op because the map was cleared above.
      pending.fail(err);
    }
  }

  /** Strip a Content-Length header if one is present (defensive; the bridge
   * normally forwards bare JSON to us). */
  private parseBody(data: string): string | null {
    const idx = data.indexOf("\r\n\r\n");
    if (idx === -1) return data;
    return data.slice(idx + 4);
  }

  /** Handle incoming WebSocket messages. */
  private handleMessage(data: string) {
    const body = this.parseBody(data);
    if (!body) return;
    let msg: {
      id?: number;
      result?: unknown;
      error?: { message?: string };
      method?: string;
      params?: unknown;
    };
    try {
      msg = JSON.parse(body);
    } catch {
      return; // ignore non-JSON frames
    }

    // Response to one of our requests.
    if (msg.id !== undefined && this.pending.has(msg.id)) {
      const pending = this.pending.get(msg.id)!;
      if (msg.error) {
        pending.fail(new Error(msg.error.message ?? "LSP request failed"));
      } else {
        pending.settle(msg.result ?? null);
      }
      return;
    }

    // Pushed diagnostics — route to the matching URI only.
    if (msg.method === "textDocument/publishDiagnostics") {
      const params = msg.params as
        | { uri: string; diagnostics: LspDiagnostic[] }
        | undefined;
      if (params?.uri) {
        this.diagnosticsByUri.set(params.uri, params.diagnostics ?? []);
        this.diagnosticsListeners
          .get(params.uri)
          ?.forEach((cb) => cb(params.diagnostics ?? []));
      }
    }
  }
}

// ─── LSP ↔ Monaco type converters ────────────────────────────────────────

function toMonacoRange(range: LspRange): Monaco.IRange {
  return {
    startLineNumber: range.start.line + 1,
    startColumn: range.start.character + 1,
    endLineNumber: range.end.line + 1,
    endColumn: range.end.character + 1,
  };
}

function toMonacoCompletionItem(
  item: LspCompletionItem,
  model: Monaco.editor.ITextModel,
  position: Monaco.Position,
): Monaco.languages.CompletionItem {
  let docString: string | undefined;
  if (typeof item.documentation === "object" && item.documentation && "value" in item.documentation) {
    docString = (item.documentation as { value: string }).value;
  } else {
    docString = item.documentation as string | undefined;
  }

  // Prefer the server-provided edit range; otherwise replace the word under the
  // cursor rather than a degenerate (1,1)-(1,1) range that mangles the buffer.
  let range: Monaco.IRange;
  if (item.textEdit?.range) {
    range = toMonacoRange(item.textEdit.range);
  } else {
    const word = model.getWordUntilPosition(position);
    range = {
      startLineNumber: position.lineNumber,
      startColumn: word.startColumn,
      endLineNumber: position.lineNumber,
      endColumn: word.endColumn,
    };
  }

  return {
    label: item.label,
    kind: lspKindToMonaco(item.kind ?? 1),
    detail: item.detail,
    documentation: docString,
    insertText: item.insertText ?? item.label,
    insertTextRules: item.insertTextFormat === 2 ? 4 /* InsertAsSnippet */ : undefined,
    sortText: item.sortText,
    filterText: item.filterText,
    range,
  };
}

function toMonacoLocation(
  monaco: typeof Monaco,
  loc: LspLocation,
): Monaco.languages.Location {
  return {
    uri: monaco.Uri.parse(loc.uri),
    range: toMonacoRange(loc.range),
  };
}

function lspKindToMonaco(kind: number): Monaco.languages.CompletionItemKind {
  // LSP CompletionItemKind → Monaco CompletionItemKind
  const map: Record<number, number> = {
    1: 0, // Text
    2: 1, // Method
    3: 2, // Function
    4: 3, // Constructor
    5: 4, // Field
    6: 5, // Variable
    7: 6, // Class
    8: 7, // Interface
    9: 8, // Module
    10: 9, // Property
    11: 10, // Unit
    12: 11, // Value
    13: 12, // Enum
    14: 13, // Keyword
    15: 14, // Snippet
    16: 15, // Color
    17: 16, // File
    18: 17, // Reference
    19: 18, // Folder
    20: 19, // EnumMember
    21: 20, // Constant
    22: 21, // Struct
    23: 22, // Event
    24: 23, // Operator
    25: 24, // TypeParameter
  };
  return (map[kind] ?? 0) as Monaco.languages.CompletionItemKind;
}

// ─── Monaco provider registration helpers ─────────────────────────────────

export interface LspProvidersRegistration {
  dispose: () => void;
  client: LspClient;
}

/**
 * Register LSP-powered providers on a Monaco languages namespace for a single
 * file, and bind diagnostics for that file's URI to Monaco markers.
 *
 * The client is shared across files (see `lspSession`), so `dispose()` only
 * unregisters the providers/listeners — it does NOT disconnect the client.
 */
export function registerLspProviders(
  monaco: typeof Monaco,
  client: LspClient,
  filePath: string,
): LspProvidersRegistration {
  const disposables: Monaco.IDisposable[] = [];
  const uriString = fileUriString(filePath);
  const langId = languageForFile(filePath) ?? "plaintext";
  const matchesFile = (model: Monaco.editor.ITextModel) =>
    model.uri.toString() === uriString;

  // ── Diagnostics via markers ──────────────────────────────────────────
  const unsubscribeDiagnostics = client.setDiagnosticsCallback(uriString, (diags) => {
    const model = monaco.editor.getModel(monaco.Uri.parse(uriString));
    if (!model) {
      // The model for this file is not mounted right now; skip rather than
      // falling back to an unrelated model (which used to cross-contaminate).
      if (import.meta.env.DEV) console.warn("[LSP] no model for", uriString);
      return;
    }
    const markers: Monaco.editor.IMarkerData[] = diags.map((d) => ({
      severity:
        d.severity === 1
          ? monaco.MarkerSeverity.Error
          : d.severity === 2
            ? monaco.MarkerSeverity.Warning
            : d.severity === 4
              ? monaco.MarkerSeverity.Hint
              : monaco.MarkerSeverity.Info,
      message: d.message,
      source: d.source,
      code: typeof d.code === "string" ? d.code : String(d.code ?? ""),
      startLineNumber: d.range.start.line + 1,
      startColumn: d.range.start.character + 1,
      endLineNumber: d.range.end.line + 1,
      endColumn: d.range.end.character + 1,
    }));
    monaco.editor.setModelMarkers(model, "lsp", markers);
  });
  disposables.push({ dispose: unsubscribeDiagnostics });

  // ── Hover provider ───────────────────────────────────────────────────
  disposables.push(
    monaco.languages.registerHoverProvider(langId, {
      provideHover: async (model, position) => {
        if (!matchesFile(model)) return null;
        try {
          const result = await client.requestHover(
            filePath,
            position.lineNumber - 1,
            position.column - 1,
          );
          if (!result) return null;
          return {
            contents: [{ value: result }],
            range: {
              startLineNumber: position.lineNumber,
              startColumn: position.column,
              endLineNumber: position.lineNumber,
              endColumn: position.column,
            },
          };
        } catch {
          return null;
        }
      },
    }),
  );

  // ── Completion provider ──────────────────────────────────────────────
  disposables.push(
    monaco.languages.registerCompletionItemProvider(langId, {
      provideCompletionItems: async (model, position) => {
        if (!matchesFile(model)) return null;
        try {
          const items = await client.requestCompletions(
            filePath,
            position.lineNumber - 1,
            position.column - 1,
          );
          if (!items?.length) return null;
          return {
            suggestions: items.map((item) =>
              toMonacoCompletionItem(item, model, position),
            ),
          };
        } catch {
          return null;
        }
      },
      triggerCharacters: [".", ":", '"', "'", "/", "@", "#"],
    }),
  );

  // ── Definition provider ──────────────────────────────────────────────
  disposables.push(
    monaco.languages.registerDefinitionProvider(langId, {
      provideDefinition: async (model, position) => {
        if (!matchesFile(model)) return null;
        try {
          const locations = await client.requestDefinition(
            filePath,
            position.lineNumber - 1,
            position.column - 1,
          );
          return locations?.map((loc) => toMonacoLocation(monaco, loc)) ?? null;
        } catch {
          return null;
        }
      },
    }),
  );

  // ── Reference provider ───────────────────────────────────────────────
  disposables.push(
    monaco.languages.registerReferenceProvider(langId, {
      provideReferences: async (model, position) => {
        if (!matchesFile(model)) return null;
        try {
          const locations = await client.requestReferences(
            filePath,
            position.lineNumber - 1,
            position.column - 1,
          );
          return locations?.map((loc) => toMonacoLocation(monaco, loc)) ?? null;
        } catch {
          return null;
        }
      },
    }),
  );

  return {
    dispose: () => {
      disposables.forEach((d) => d.dispose());
    },
    client,
  };
}
