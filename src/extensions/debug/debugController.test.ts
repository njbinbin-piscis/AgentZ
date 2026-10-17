import { afterEach, beforeEach, expect, it, vi } from "vitest";
const { output, points } = vi.hoisted(() => ({
  output: vi.fn(),
  points: new Map<string, Set<number>>(),
}));
vi.mock("./dapClient", () => ({ DapClient: class {} }));
vi.mock("./breakpoints", () => ({ breakpointStore: { all: () => points } }));
vi.mock("../extensionUiStore", () => ({ extensionUiStore: { appendDebugOutput: output } }));
vi.mock("../extensionService", () => ({ extensionService: { projectDir: "fallback" } }));
import { DebugController, type DebugClient } from "./debugController";
import { debugStore } from "./debugStore";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((r, j) => {
    resolve = r;
    reject = j;
  });
  return { promise, resolve, reject };
}
class Client implements DebugClient {
  event: (event: string, body: unknown) => void = () => {};
  connect = vi.fn(async (_command: string, _args: string[], _cwd?: string, _remote?: string) => {});
  dispose = vi.fn(async () => {});
  handle: (command: string, args?: Record<string, unknown>) => unknown = (command) => {
    if (command === "initialize") return { supportsConfigurationDoneRequest: true };
    if (command === "launch") this.event("initialized", {});
    if (command === "stackTrace") return { stackFrames: [{ id: 1, name: "frame", line: 1 }] };
    if (command === "scopes") return { scopes: [{ variablesReference: 1 }] };
    if (command === "variables")
      return { variables: [{ name: "value", value: "old", variablesReference: 0 }] };
    return {};
  };
  calls: { command: string; args?: Record<string, unknown> }[] = [];
  request<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
    this.calls.push({ command, args });
    try {
      return Promise.resolve(this.handle(command, args)) as Promise<T>;
    } catch (error) {
      return Promise.reject(error);
    }
  }
  onEvent(handler: (event: string, body: unknown) => void): void {
    this.event = handler;
  }
  onLog(): void {}
}
const config = {
  type: "custom",
  request: "launch",
  name: "Test",
  __adapter: { command: "adapter" },
};
const flush = async () => {
  for (let i = 0; i < 12; i++) await Promise.resolve();
};
const controllers: DebugController[] = [];
function controller(clients: Client[], projectDir: () => string | null = () => "/project") {
  const c = new DebugController({
    createClient: () => clients.shift()!,
    projectDir,
    registerWindowListener: false,
    initializedTimeoutMs: 10,
    stopTimeoutMs: 10,
  });
  controllers.push(c);
  return c;
}
beforeEach(() => {
  output.mockClear();
  points.clear();
  debugStore.end();
});
afterEach(async () => {
  vi.useRealTimers();
  await Promise.all(controllers.splice(0).map((c) => c.stop()));
});

it("old connect completion and EOF cannot stop or publish into replacement session", async () => {
  const old = new Client(),
    fresh = new Client(),
    pending = deferred<void>();
  old.connect.mockImplementation(() => pending.promise);
  const c = controller([old, fresh]);
  const first = c.start(config);
  await flush();
  old.event("output", { output: "PREVIOUS_SESSION" });
  await c.start(config);
  expect(c.isActive).toBe(true);
  old.event("exited", {});
  old.event("output", { output: "OLD_OUTPUT" });
  pending.resolve();
  await first;
  expect(old.calls.some((call) => call.command === "initialize")).toBe(false);
  expect(debugStore.getSnapshot().active).toBe(true);
  expect(debugStore.getSnapshot().output).not.toContain("OLD_OUTPUT");
  expect(debugStore.getSnapshot().output).not.toContain("PREVIOUS_SESSION");
});
it("stop synchronously revokes UI ownership while old bounded cleanup is pending", async () => {
  vi.useFakeTimers();
  const old = new Client(),
    fresh = new Client(),
    disconnect = deferred<unknown>();
  const normal = old.handle;
  old.handle = (command) => (command === "disconnect" ? disconnect.promise : normal(command));
  const c = controller([old, fresh]);
  await c.start(config);
  const stopped = c.stop();
  expect(c.isActive).toBe(false);
  expect(debugStore.getSnapshot().active).toBe(false);
  await c.start(config);
  await vi.advanceTimersByTimeAsync(11);
  await stopped;
  expect(c.isActive).toBe(true);
  expect(old.dispose).toHaveBeenCalledOnce();
  expect(fresh.dispose).not.toHaveBeenCalled();
});
it("missing initialized event fails rather than silently proceeding after a deadline", async () => {
  vi.useFakeTimers();
  const client = new Client();
  client.handle = () => ({});
  const c = controller([client]);
  const start = c.start(config);
  await flush();
  await vi.advanceTimersByTimeAsync(11);
  await start;
  expect(c.isActive).toBe(false);
  expect(
    client.calls.some(
      (call) => call.command === "setBreakpoints" || call.command === "configurationDone",
    ),
  ).toBe(false);
  expect(output.mock.calls.flat().join(" ")).toContain("initialized event timed out");
});
it("negotiates configurationDone and exposes breakpoint/configuration errors", async () => {
  const unsupported = new Client(),
    failed = new Client();
  const normal = unsupported.handle;
  unsupported.handle = (command) => (command === "initialize" ? {} : normal(command));
  const c = controller([unsupported, failed]);
  await c.start(config);
  expect(unsupported.calls.some((call) => call.command === "configurationDone")).toBe(false);
  const next = failed.handle;
  failed.handle = (command) => {
    if (command === "configurationDone") throw new Error("configuration refused");
    return next(command);
  };
  await c.start(config);
  expect(c.isActive).toBe(false);
  expect(output.mock.calls.flat().join(" ")).toContain("configuration refused");
});
it("second pause and continue invalidate older stack/scopes/variable responses", async () => {
  const client = new Client(),
    oldVariables = deferred<unknown>();
  let variableRequests = 0;
  const normal = client.handle;
  client.handle = (command) =>
    command === "variables" && ++variableRequests === 1 ? oldVariables.promise : normal(command);
  const c = controller([client]);
  await c.start(config);
  client.event("stopped", { threadId: 1 });
  await flush();
  client.event("stopped", { threadId: 2 });
  await flush();
  expect(debugStore.getSnapshot().threadId).toBe(2);
  client.event("continued", {});
  oldVariables.resolve({ variables: [{ name: "stale", value: "SECRET", variablesReference: 0 }] });
  await flush();
  expect(debugStore.getSnapshot()).toMatchObject({
    running: true,
    threadId: null,
    frames: [],
    variables: [],
  });
});
it("failed step preserves paused state and same-tick step requests are coalesced", async () => {
  const client = new Client(),
    step = deferred<unknown>();
  const normal = client.handle;
  client.handle = (command) => (command === "next" ? step.promise : normal(command));
  const c = controller([client]);
  await c.start(config);
  client.event("stopped", { threadId: 1 });
  await flush();
  const first = c.stepOver();
  const duplicate = c.stepOver();
  expect(client.calls.filter((call) => call.command === "next")).toHaveLength(1);
  step.reject(new Error("cannot step"));
  await Promise.all([first, duplicate]);
  expect(debugStore.getSnapshot().running).toBe(false);
  expect(debugStore.getSnapshot().frames).toHaveLength(1);
});

it.each(["stackTrace", "scopes"])("continued invalidates pending %s before further paused queries", async command => {
  const client = new Client(), pending = deferred<unknown>();
  const normal = client.handle;
  client.handle = requested => requested === command ? pending.promise : normal(requested);
  const c = controller([client]);
  await c.start(config);
  client.event("stopped", { threadId: 1 }); await flush();
  client.event("continued", {});
  pending.resolve(command === "stackTrace" ? { stackFrames: [{ id: 1, name: "stale", line: 1 }] } : { scopes: [{ variablesReference: 1 }] });
  await flush();
  expect(debugStore.getSnapshot()).toMatchObject({ running: true, frames: [], variables: [], threadId: null });
  expect(client.calls.some(call => call.command === "variables")).toBe(false);
});
it("captures project before awaiting and filters foreign/recovery breakpoints", async () => {
  const client = new Client(),
    connected = deferred<void>();
  client.connect.mockImplementation(() => connected.promise);
  let project = "agentz-remote://host/project";
  points.set(`${project}/a.ts`, new Set([3]));
  points.set("agentz-remote://other/project/a.ts", new Set([4]));
  points.set("agentz-recovery:old:a.ts", new Set([5]));
  const c = controller([client], () => project);
  const start = c.start(config);
  await flush();
  project = "agentz-remote://other/new";
  connected.resolve();
  await start;
  expect(client.connect.mock.calls[0][3]).toBe("agentz-remote://host/project");
  expect(
    client.calls
      .filter((call) => call.command === "setBreakpoints")
      .map((call) => call.args?.source),
  ).toEqual([{ path: "agentz-remote://host/project/a.ts", name: "a.ts" }]);
});

it("project switch or close stops the active session and clears debug state", async () => {
  const client = new Client();
  const c = controller([client]);
  await c.start(config);
  expect(debugStore.getSnapshot().active).toBe(true);
  await c.stopIfProjectChanged("/project");
  expect(c.isActive).toBe(true);
  await c.stopIfProjectChanged("/other-project");
  expect(c.isActive).toBe(false);
  expect(debugStore.getSnapshot().active).toBe(false);
  expect(client.calls.some((call) => call.command === "disconnect")).toBe(true);
  expect(client.dispose).toHaveBeenCalledOnce();
});

it("maps stack frame sources to project-relative paths only inside the session project", async () => {
  const client = new Client();
  const normal = client.handle;
  client.handle = (command) =>
    command === "stackTrace"
      ? {
          stackFrames: [
            { id: 1, name: "inside", line: 3, source: { path: "/project/src/a.ts" } },
            { id: 2, name: "outside", line: 4, source: { path: "/elsewhere/b.ts" } },
            { id: 3, name: "traversal", line: 5, source: { path: "/project/../secret.ts" } },
          ],
        }
      : normal(command);
  const c = controller([client]);
  await c.start(config);
  client.event("stopped", { threadId: 1 });
  await flush();
  expect(debugStore.getSnapshot().frames.map((frame) => frame.source)).toEqual([
    "/project/src/a.ts",
    "/elsewhere/b.ts",
    "/project/../secret.ts",
  ]);
  expect(c.projectRelativePath("/project/src/a.ts")).toBe("src/a.ts");
  expect(c.projectRelativePath("/elsewhere/b.ts")).toBeNull();
  expect(c.projectRelativePath("/project/../secret.ts")).toBeNull();
  expect(c.projectRelativePath(undefined)).toBeNull();
});

it("project switch stops the old session synchronously even while projectDir still reads the old value", async () => {
  const client = new Client();
  const c = controller([client], () => "/old");
  await c.stopIfProjectChanged("/old");
  await c.start(config);
  expect(c.isActive).toBe(true);
  await c.stopIfProjectChanged("/new");
  expect(c.isActive).toBe(false);
  expect(client.dispose).toHaveBeenCalledOnce();
});
it("a session started after a switch is tagged with the new project and survives a repeat report", async () => {
  const client = new Client();
  const c = controller([client], () => "/old");
  await c.stopIfProjectChanged("/old");
  await c.stopIfProjectChanged("/new");
  await c.start(config);
  expect(c.isActive).toBe(true);
  await c.stopIfProjectChanged("/new");
  expect(c.isActive).toBe(true);
  expect(client.dispose).not.toHaveBeenCalled();
});
