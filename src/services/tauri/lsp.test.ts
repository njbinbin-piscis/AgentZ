// Regression tests for the LSP client + Monaco provider wiring.
//
// These pin the fixes for the audit's LSP-01/02/04 items:
//   * transport uses bare JSON (the Rust bridge owns Content-Length framing)
//   * the handshake resolves against the bridge's canned response (id 0)
//   * disconnect fails in-flight requests instead of hanging
//   * URIs match what `<Editor path>` produces, markers don't cross-contaminate
//   * completion ranges come from the word under the cursor, not (1,1)-(1,1)
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { URI } from "monaco-editor/esm/vs/base/common/uri.js";
import { LspClient, registerLspProviders } from "./lsp";
import { fileUriString } from "./editorUri";

// ─── Fake WebSocket that can act as an LSP bridge ─────────────────────────

type Server = (msg: Record<string, unknown>, ws: FakeWebSocket) => void;

class FakeWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  static instances: FakeWebSocket[] = [];
  /** Applied to every socket created afterwards, so the responder is in place
   * before `onopen` fires and the handshake frame is sent. */
  static defaultServer: Server | null = null;

  readyState = FakeWebSocket.CONNECTING;
  sent: string[] = [];
  server: Server | null = FakeWebSocket.defaultServer;
  onopen: (() => void) | null = null;
  onmessage: ((ev: { data: string }) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  onclose: (() => void) | null = null;

  constructor(public url: string) {
    FakeWebSocket.instances.push(this);
    queueMicrotask(() => {
      if (this.readyState !== FakeWebSocket.CONNECTING) return;
      this.readyState = FakeWebSocket.OPEN;
      this.onopen?.();
    });
  }

  send(data: string) {
    this.sent.push(data);
    // Respond synchronously to exercise "response arrives before we yield":
    // the client must register pending *before* sending.
    this.server?.(JSON.parse(data) as Record<string, unknown>, this);
  }

  close() {
    this.readyState = FakeWebSocket.CLOSED;
    this.onclose?.();
  }

  /** Simulate the peer dropping the connection. */
  simulateDrop() {
    this.readyState = FakeWebSocket.CLOSED;
    this.onclose?.();
  }

  emit(obj: unknown) {
    this.onmessage?.({ data: JSON.stringify(obj) });
  }
}

const CONTENT_LENGTH = "Content-Length";

function lastSent(ws: FakeWebSocket): Record<string, unknown> {
  return JSON.parse(ws.sent[ws.sent.length - 1]) as Record<string, unknown>;
}

function sentMethods(ws: FakeWebSocket): string[] {
  return ws.sent.map((f) => (JSON.parse(f) as { method?: string }).method ?? "");
}

/** Default bridge behaviour: canned initialize + minimal responses. */
const defaultServer: Server = (msg, ws) => {
  if (msg.method === "initialize") {
    ws.emit({ jsonrpc: "2.0", id: 0, result: { capabilities: {} } });
    return;
  }
  if (msg.id !== undefined && typeof msg.method === "string") {
    switch (msg.method) {
      case "textDocument/hover":
        ws.emit({
          jsonrpc: "2.0",
          id: msg.id,
          result: { contents: { kind: "markdown", value: "**hover**" } },
        });
        break;
      case "textDocument/completion":
        ws.emit({ jsonrpc: "2.0", id: msg.id, result: { items: [{ label: "foo" }] } });
        break;
      case "textDocument/diagnostic":
        ws.emit({ jsonrpc: "2.0", id: msg.id, result: { items: [] } });
        break;
      default:
        break;
    }
  }
};

async function connectClient(server: Server = defaultServer): Promise<{ client: LspClient; ws: FakeWebSocket }> {
  FakeWebSocket.defaultServer = server;
  const client = new LspClient(4321);
  const connecting = client.connect("/proj", "typescript");
  // The socket is constructed synchronously inside connect().
  const ws = FakeWebSocket.instances[FakeWebSocket.instances.length - 1];
  await connecting;
  return { client, ws };
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  FakeWebSocket.defaultServer = null;
  vi.stubGlobal("WebSocket", FakeWebSocket);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

// ─── Transport (LSP-04) ───────────────────────────────────────────────────

describe("LSP transport", () => {
  it("sends bare JSON, never Content-Length framing", async () => {
    const { client, ws } = await connectClient();
    client.sendDidChange("/proj/a.ts", "const x = 1;");

    for (const frame of ws.sent) {
      expect(frame.startsWith(CONTENT_LENGTH)).toBe(false);
      expect(() => JSON.parse(frame)).not.toThrow();
    }
    expect(lastSent(ws).method).toBe("textDocument/didChange");
  });

  it("keeps multi-byte characters intact (UTF-8 safe, no length header)", async () => {
    const { client, ws } = await connectClient();
    const text = "const 中文 = '😀'; // Ω";
    client.sendDidChange("/proj/a.ts", text);

    const frame = ws.sent[ws.sent.length - 1];
    expect(frame.startsWith(CONTENT_LENGTH)).toBe(false);
    const parsed = JSON.parse(frame) as {
      params: { contentChanges: { text: string }[] };
    };
    expect(parsed.params.contentChanges[0].text).toBe(text);
  });

  it("completes the handshake against the bridge's canned initialize (id 0)", async () => {
    const { ws } = await connectClient();
    expect(ws.sent[0].includes("initialize")).toBe(true);
    expect(sentMethods(ws)).toContain("initialized");
  });

  it("rejects connect() when initialize returns an error", async () => {
    const client = new LspClient(4321);
    FakeWebSocket.defaultServer = (msg, ws) => {
      if (msg.method === "initialize") {
        ws.emit({ jsonrpc: "2.0", id: 0, error: { message: "nope", code: -1 } });
      }
    };
    await expect(client.connect("/proj", "typescript")).rejects.toThrow(/nope/);
  });
});

// ─── Disconnect / timeout robustness (LSP-03) ─────────────────────────────

describe("LSP lifecycle robustness", () => {
  it("fails in-flight requests when the socket closes", async () => {
    const { client, ws } = await connectClient();
    ws.server = () => {}; // stop answering
    const hover = client.requestHover("/proj/a.ts", 0, 0);
    ws.simulateDrop();
    await expect(hover).rejects.toThrow(/disconnected/);
  });

  it("resolves read-only requests to null on timeout (no hang)", async () => {
    vi.useFakeTimers();
    const client = new LspClient(4321);
    FakeWebSocket.defaultServer = (msg, ws) => {
      if (msg.method === "initialize") {
        ws.emit({ jsonrpc: "2.0", id: 0, result: { capabilities: {} } });
      }
      // hover is deliberately never answered
    };
    const connecting = client.connect("/proj", "typescript");
    await connecting;

    const hover = client.requestHover("/proj/a.ts", 0, 0);
    await vi.advanceTimersByTimeAsync(5001);
    await expect(hover).resolves.toBeNull();
  });
});

// ─── Document lifecycle / versions (LSP-02) ───────────────────────────────

describe("document lifecycle", () => {
  it("tracks per-document versions monotonically across A→B→A", async () => {
    const { client, ws } = await connectClient();
    client.openDocument("/proj/a.ts", "typescript", "a");
    client.sendDidChange("/proj/a.ts", "a1");
    client.sendDidChange("/proj/a.ts", "a2");
    client.openDocument("/proj/b.ts", "typescript", "b");
    client.sendDidChange("/proj/b.ts", "b1");
    client.closeDocument("/proj/b.ts");
    client.sendDidChange("/proj/a.ts", "a3");

    const versions: Record<string, number[]> = {};
    for (const frame of ws.sent) {
      const msg = JSON.parse(frame) as {
        method?: string;
        params?: { textDocument?: { uri: string; version?: number } };
      };
      if (
        (msg.method === "textDocument/didOpen" ||
          msg.method === "textDocument/didChange") &&
        msg.params?.textDocument?.version !== undefined
      ) {
        const uri = msg.params.textDocument.uri;
        (versions[uri] ??= []).push(msg.params.textDocument.version);
      }
    }
    expect(versions[fileUriString("/proj/a.ts")]).toEqual([1, 2, 3, 4]);
    expect(versions[fileUriString("/proj/b.ts")]).toEqual([1, 2]);
  });

  it("emits didClose for the closed document", async () => {
    const { client, ws } = await connectClient();
    client.openDocument("/proj/a.ts", "typescript", "a");
    client.closeDocument("/proj/a.ts");
    const closes = ws.sent.filter((f) => f.includes("didClose"));
    expect(closes).toHaveLength(1);
    expect(closes[0]).toContain(fileUriString("/proj/a.ts"));
  });
});

// ─── Diagnostics routing ──────────────────────────────────────────────────

describe("diagnostics routing", () => {
  it("only notifies listeners of the matching URI", async () => {
    const { client, ws } = await connectClient();
    const aUri = fileUriString("/proj/a.ts");
    const bUri = fileUriString("/proj/b.ts");
    const aFn = vi.fn();
    const bFn = vi.fn();
    client.setDiagnosticsCallback(aUri, aFn);
    client.setDiagnosticsCallback(bUri, bFn);

    ws.emit({
      jsonrpc: "2.0",
      method: "textDocument/publishDiagnostics",
      params: {
        uri: aUri,
        diagnostics: [
          {
            range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } },
            message: "x",
          },
        ],
      },
    });

    expect(aFn).toHaveBeenCalledTimes(1);
    expect(bFn).not.toHaveBeenCalled();
  });
});

// ─── URI matching (LSP-01) ────────────────────────────────────────────────

describe("URI canonicalisation", () => {
  it("fileUriString round-trips through Uri.parse unchanged", () => {
    const p = "/home/u/proj/src/a.ts";
    const s = fileUriString(p);
    expect(s).toBe(URI.file(p).toString());
    expect(URI.parse(s).toString()).toBe(s); // what <Editor path> yields
  });
});

// ─── Provider wiring (LSP-01: markers + completion range) ─────────────────

interface FakeModel {
  uri: { toString: () => string };
  getWordUntilPosition: () => { word: string; startColumn: number; endColumn: number };
}

function makeModel(
  uriString: string,
  word: { word: string; startColumn: number; endColumn: number },
): FakeModel {
  return { uri: { toString: () => uriString }, getWordUntilPosition: () => word };
}

function makeFakeMonaco() {
  const models = new Map<string, FakeModel>();
  const markerCalls: { modelUri: string; markers: unknown[] }[] = [];
  const providers: Record<string, unknown> = {};

  const monaco = {
    Uri: URI,
    MarkerSeverity: { Hint: 1, Info: 2, Warning: 4, Error: 8 },
    editor: {
      getModel: (uri: { toString: () => string }) => models.get(uri.toString()) ?? null,
      setModelMarkers: (model: FakeModel | null, _owner: string, markers: unknown[]) => {
        if (!model) return;
        markerCalls.push({ modelUri: model.uri.toString(), markers });
      },
    },
    languages: {
      registerHoverProvider: (_lang: string, p: unknown) => {
        providers.hover = p;
        return { dispose() {} };
      },
      registerCompletionItemProvider: (_lang: string, p: unknown) => {
        providers.completion = p;
        return { dispose() {} };
      },
      registerDefinitionProvider: (_lang: string, p: unknown) => {
        providers.definition = p;
        return { dispose() {} };
      },
      registerReferenceProvider: (_lang: string, p: unknown) => {
        providers.reference = p;
        return { dispose() {} };
      },
    },
  };

  return { monaco, models, markerCalls, providers };
}

function asMonaco(m: ReturnType<typeof makeFakeMonaco>["monaco"]) {
  return m as unknown as typeof import("monaco-editor");
}

const diag = (message: string) => ({
  range: { start: { line: 0, character: 0 }, end: { line: 0, character: 1 } },
  severity: 1,
  message,
});

describe("Monaco provider wiring", () => {
  it("routes markers to the matching model and skips when it is absent", async () => {
    const { client, ws } = await connectClient();
    const { monaco, models, markerCalls } = makeFakeMonaco();
    const aUri = fileUriString("/proj/a.ts");
    const bUri = fileUriString("/proj/b.ts");

    registerLspProviders(asMonaco(monaco), client, "/proj/a.ts");

    // Model A not mounted yet → diagnostics skipped, not mis-attributed.
    ws.emit({
      jsonrpc: "2.0",
      method: "textDocument/publishDiagnostics",
      params: { uri: aUri, diagnostics: [diag("boom")] },
    });
    expect(markerCalls).toHaveLength(0);

    // Mount A and B → markers land on A only.
    models.set(aUri, makeModel(aUri, { word: "", startColumn: 1, endColumn: 1 }));
    models.set(bUri, makeModel(bUri, { word: "", startColumn: 1, endColumn: 1 }));
    ws.emit({
      jsonrpc: "2.0",
      method: "textDocument/publishDiagnostics",
      params: { uri: aUri, diagnostics: [diag("boom")] },
    });
    expect(markerCalls).toHaveLength(1);
    expect(markerCalls[0].modelUri).toBe(aUri);
  });

  it("computes the completion range from the word under the cursor", async () => {
    const { client } = await connectClient((msg, sock) => {
      if (msg.method === "initialize") {
        sock.emit({ jsonrpc: "2.0", id: 0, result: { capabilities: {} } });
      } else if (msg.method === "textDocument/completion") {
        sock.emit({ jsonrpc: "2.0", id: msg.id, result: { items: [{ label: "foo" }] } });
      }
    });
    const { monaco, models, providers } = makeFakeMonaco();
    const aUri = fileUriString("/proj/a.ts");
    const model = makeModel(aUri, { word: "fo", startColumn: 3, endColumn: 5 });
    models.set(aUri, model);

    registerLspProviders(asMonaco(monaco), client, "/proj/a.ts");
    const completion = providers.completion as {
      provideCompletionItems: (
        model: FakeModel,
        position: { lineNumber: number; column: number },
      ) => Promise<{ suggestions: { range: unknown }[] } | null>;
    };

    const result = await completion.provideCompletionItems(model, { lineNumber: 1, column: 5 });
    expect(result?.suggestions[0].range).toEqual({
      startLineNumber: 1,
      startColumn: 3,
      endLineNumber: 1,
      endColumn: 5,
    });
  });
});
