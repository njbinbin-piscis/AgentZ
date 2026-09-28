// Regression tests for the shared LSP session pool (audit item LSP-02):
// tabs of the same language must share one connection, and the backend session
// must only be stopped once the last consumer releases it.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { startMock, stopMock } = vi.hoisted(() => ({
  startMock: vi.fn(async (_projectDir: string, _language: string) => 9999),
  stopMock: vi.fn(async (_projectDir: string, _language: string) => undefined),
}));

vi.mock("./lsp", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./lsp")>();
  return {
    ...actual,
    lspApi: {
      listLanguages: vi.fn(),
      start: startMock,
      stop: stopMock,
    },
  };
});

import { acquireLspSession } from "./lspSession";

class FakeWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  static instances: FakeWebSocket[] = [];
  static defaultServer: ((msg: Record<string, unknown>, ws: FakeWebSocket) => void) | null = null;

  readyState = FakeWebSocket.CONNECTING;
  server = FakeWebSocket.defaultServer;
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
    this.server?.(JSON.parse(data) as Record<string, unknown>, this);
  }

  close() {
    this.readyState = FakeWebSocket.CLOSED;
    this.onclose?.();
  }

  emit(obj: unknown) {
    this.onmessage?.({ data: JSON.stringify(obj) });
  }
}

beforeEach(() => {
  FakeWebSocket.instances = [];
  FakeWebSocket.defaultServer = (msg, ws) => {
    if (msg.method === "initialize") {
      ws.emit({ jsonrpc: "2.0", id: 0, result: { capabilities: {} } });
    }
  };
  vi.stubGlobal("WebSocket", FakeWebSocket);
  startMock.mockClear();
  stopMock.mockClear();
  startMock.mockResolvedValue(9999);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("LSP session pool", () => {
  it("shares one connection for the same project+language", async () => {
    const a = await acquireLspSession("/proj-a", "typescript");
    const b = await acquireLspSession("/proj-a", "typescript");

    expect(a.client).toBe(b.client);
    expect(startMock).toHaveBeenCalledTimes(1);
    expect(FakeWebSocket.instances).toHaveLength(1);

    a.release();
    b.release();
  });

  it("keeps the backend session alive while another consumer holds it", async () => {
    const a = await acquireLspSession("/proj-b", "typescript");
    const b = await acquireLspSession("/proj-b", "typescript");

    a.release();
    expect(stopMock).not.toHaveBeenCalled();

    b.release();
    expect(stopMock).toHaveBeenCalledTimes(1);
    expect(stopMock).toHaveBeenCalledWith("/proj-b", "typescript");
  });

  it("disconnects the client when the last consumer releases", async () => {
    const a = await acquireLspSession("/proj-c", "typescript");
    const ws = FakeWebSocket.instances[0];
    expect(ws.readyState).toBe(FakeWebSocket.OPEN);

    a.release();
    expect(ws.readyState).toBe(FakeWebSocket.CLOSED);
    expect(stopMock).toHaveBeenCalledWith("/proj-c", "typescript");
  });

  it("drops a failed session so a later acquire can retry", async () => {
    startMock.mockRejectedValueOnce(new Error("server missing"));

    await expect(acquireLspSession("/proj-d", "typescript")).rejects.toThrow(
      /server missing/,
    );

    const retry = await acquireLspSession("/proj-d", "typescript");
    expect(retry.client).toBeTruthy();
    expect(startMock).toHaveBeenCalledTimes(2);
    retry.release();
  });

  it("isolates sessions by language", async () => {
    const ts = await acquireLspSession("/proj-e", "typescript");
    const py = await acquireLspSession("/proj-e", "python");
    expect(ts.client).not.toBe(py.client);
    expect(startMock).toHaveBeenCalledTimes(2);
    ts.release();
    py.release();
  });
});
