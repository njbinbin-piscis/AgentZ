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
  it("bounds failed automatic restarts instead of looping indefinitely", async () => {
    vi.useFakeTimers();
    const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const handle = await acquireLspSession("/exhausted-project", "rust");
      const health = vi.fn();
      const unsubscribe = handle.client.subscribeHealth(health);
      startMock.mockRejectedValue(new Error("server unavailable"));
      FakeWebSocket.instances[FakeWebSocket.instances.length - 1].close();
      await vi.advanceTimersByTimeAsync(1000);
      expect(startMock).toHaveBeenCalledTimes(4);
      await vi.advanceTimersByTimeAsync(10000);
      expect(startMock).toHaveBeenCalledTimes(4);
      expect(warning).toHaveBeenCalledOnce();
      expect(health).toHaveBeenCalledWith("recovering");
      expect(health).toHaveBeenLastCalledWith("unavailable");
      unsubscribe();
      handle.release();
    } finally { warning.mockRestore(); vi.useRealTimers(); }
  });
  it("recovers on a fresh bridge without replacing the shared client and stops after release", async () => {
    vi.useFakeTimers();
    try {
      const handle = await acquireLspSession("/recovery-project", "typescript");
      const original = handle.client;
      const health = vi.fn();
      original.subscribeHealth(health);
      const socket = FakeWebSocket.instances[FakeWebSocket.instances.length - 1];
      startMock.mockResolvedValue(10001);
      socket.close();
      await vi.advanceTimersByTimeAsync(100);
      const recovered = FakeWebSocket.instances[FakeWebSocket.instances.length - 1];
      expect(recovered).not.toBe(socket);
      expect(recovered.url).toBe("ws://127.0.0.1:10001");
      expect(health).toHaveBeenCalledWith("recovering");
      expect(health).toHaveBeenLastCalledWith("ready");
      const second = await acquireLspSession("/recovery-project", "typescript");
      expect(second.client).toBe(original);
      recovered.close();
      handle.release();
      second.release();
      const starts = startMock.mock.calls.length;
      await vi.advanceTimersByTimeAsync(1000);
      expect(startMock).toHaveBeenCalledTimes(starts);
    } finally { vi.useRealTimers(); }
  });
  it("resets the restart budget after each healthy restart so later crashes recover", async () => {
    vi.useFakeTimers();
    try {
      const handle = await acquireLspSession("/budget-project", "typescript");
      const health = vi.fn();
      handle.client.subscribeHealth(health);
      // Seven crashes, each followed by a healthy reconnect. The 4th+ crashes
      // previously exhausted the lifetime-per-session budget and were not restarted.
      for (let crash = 1; crash <= 7; crash++) {
        const socket = FakeWebSocket.instances[FakeWebSocket.instances.length - 1];
        const starts = startMock.mock.calls.length;
        socket.close();
        await vi.advanceTimersByTimeAsync(1000);
        expect(startMock).toHaveBeenCalledTimes(starts + 1);
        expect(FakeWebSocket.instances[FakeWebSocket.instances.length - 1]).not.toBe(socket);
        expect(health).toHaveBeenLastCalledWith("ready");
      }
      handle.release();
    } finally { vi.useRealTimers(); }
  });
  it("stops a true crash loop at the restart cap after healthy restarts", async () => {
    vi.useFakeTimers();
    const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const handle = await acquireLspSession("/loop-after-health", "typescript");
      FakeWebSocket.instances[FakeWebSocket.instances.length - 1].close();
      await vi.advanceTimersByTimeAsync(1000);
      FakeWebSocket.instances[FakeWebSocket.instances.length - 1].close();
      await vi.advanceTimersByTimeAsync(1000);
      expect(startMock).toHaveBeenCalledTimes(3);

      startMock.mockRejectedValue(new Error("server unavailable"));
      FakeWebSocket.instances[FakeWebSocket.instances.length - 1].close();
      await vi.advanceTimersByTimeAsync(10000);
      expect(startMock).toHaveBeenCalledTimes(3 + 3);
      handle.release();
    } finally { warning.mockRestore(); vi.useRealTimers(); }
  });
  it("stops a start-then-crash loop at the per-window recovery cap", async () => {
    vi.useFakeTimers();
    const warning = vi.spyOn(console, "warn").mockImplementation(() => {});
    try {
      const handle = await acquireLspSession("/flap-project", "typescript");
      const health = vi.fn();
      handle.client.subscribeHealth(health);
      // Each reconnect succeeds, so the per-crash budget never trips; the window cap must.
      for (let crash = 0; crash < 10; crash++) {
        FakeWebSocket.instances[FakeWebSocket.instances.length - 1].close();
        await vi.advanceTimersByTimeAsync(1000);
      }
      expect(startMock).toHaveBeenCalledTimes(11);
      FakeWebSocket.instances[FakeWebSocket.instances.length - 1].close();
      await vi.advanceTimersByTimeAsync(10000);
      expect(startMock).toHaveBeenCalledTimes(11);
      expect(health).toHaveBeenLastCalledWith("unavailable");
      handle.release();
    } finally { warning.mockRestore(); vi.useRealTimers(); }
  });
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
