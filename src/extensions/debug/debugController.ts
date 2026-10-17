// Orchestrates a debug session: resolves a debug adapter, runs the DAP
// handshake, pushes breakpoints, and routes DAP events into the debug store.
//
// Adapter resolution: a launch config may carry `{ __adapter: {command, args} }`
// (e.g. provided by a debug-adapter extension via its descriptor factory).
// Otherwise a few common defaults are used. Actual debugging requires a real
// adapter to be installed/available on the machine.

import { DapClient } from "./dapClient";
import { breakpointStore } from "./breakpoints";
import { debugStore, type StackFrame, type Variable } from "./debugStore";
import { extensionUiStore } from "../extensionUiStore";
import { extensionService } from "../extensionService";
import { isRemoteDir } from "../remoteTargets";

interface DebugConfig {
  type: string;
  name: string;
  request: string;
  program?: string;
  cwd?: string;
  __adapter?: { command: string; args?: string[] };
  [key: string]: unknown;
}

function resolveAdapter(config: DebugConfig): { command: string; args: string[] } | null {
  if (config.__adapter?.command) {
    return { command: config.__adapter.command, args: config.__adapter.args ?? [] };
  }
  // Common conventions; a real adapter binary must exist on PATH.
  switch (config.type) {
    case "node":
    case "pwa-node":
      return { command: "node", args: ["--stdio"] }; // placeholder; real js-debug ships with the extension
    case "python":
    case "debugpy":
      return { command: "python", args: ["-m", "debugpy.adapter"] };
    case "lldb":
    case "codelldb":
      return { command: "codelldb", args: [] };
    default:
      return null;
  }
}

export interface DebugClient {
  connect(command: string, args: string[], cwd?: string, remoteDir?: string): Promise<void>;
  request<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T>;
  onEvent(handler: (event: string, body: unknown) => void): void;
  onLog(handler: (line: string) => void): void;
  dispose(): Promise<void>;
}
interface Dependencies {
  createClient?: () => DebugClient;
  projectDir?: () => string | null;
  initializedTimeoutMs?: number;
  stopTimeoutMs?: number;
  registerWindowListener?: boolean;
}
interface Session {
  client: DebugClient;
  /** Project the session was started for; frames and breakpoints belong to it. */
  project: string | null;
  epoch: number;
  pause: number;
  stepPause?: number;
  rejectInitialized?: (error: Error) => void;
}

function withinRoot(path: string, root: string | null): boolean {
  if (!root || path.startsWith("agentz-recovery:") || /[\0\r\n]/.test(path)) return false;
  const normalize = (value: string) => value.replace(/\\/g, "/").replace(/\/+$/, "");
  let p = normalize(path),
    r = normalize(root);
  if (p.split("/").some((part) => part === ".." || part === ".")) return false;
  if (/^[a-z]:\//i.test(r)) {
    p = p.toLowerCase();
    r = r.toLowerCase();
  }
  return p.startsWith(`${r}/`);
}

/** Project-relative path of `path` under `root`, or null when it is outside the project. */
function projectRelative(path: string | undefined, root: string | null): string | null {
  if (!path || !withinRoot(path, root)) return null;
  const normalize = (value: string) => value.replace(/\\/g, "/").replace(/\/+$/, "");
  return normalize(path).slice(normalize(root ?? "").length + 1);
}

export class DebugController {
  private session: Session | undefined;
  private epoch = 0;
  // Project the UI is currently on, set synchronously by stopIfProjectChanged; undefined until first report.
  private currentProject: string | null | undefined = undefined;
  constructor(private readonly deps: Dependencies = {}) {
    if (deps.registerWindowListener !== false && typeof window !== "undefined")
      window.addEventListener("agentz-debug-start", (e) => {
        void this.start((e as CustomEvent).detail as DebugConfig);
      });
  }
  get isActive(): boolean {
    return !!this.session;
  }
  private owns(session: Session): boolean {
    return this.session === session && this.epoch === session.epoch;
  }
  private async bounded<T>(operation: Promise<T>, milliseconds: number, label: string): Promise<T> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      return await Promise.race([
        operation,
        new Promise<T>((_, reject) => {
          timer = setTimeout(() => reject(new Error(`${label} timed out`)), milliseconds);
        }),
      ]);
    } finally {
      if (timer) clearTimeout(timer);
    }
  }
  private detach(): Session | undefined {
    const previous = this.session;
    this.session = undefined;
    this.epoch += 1;
    if (previous) {
      previous.pause += 1;
      previous.rejectInitialized?.(new Error("Debug session revoked"));
    }
    debugStore.end();
    return previous;
  }
  private async cleanup(session: Session | undefined): Promise<void> {
    if (!session) return;
    const timeout = this.deps.stopTimeoutMs ?? 2000;
    try {
      await this.bounded(
        session.client.request("disconnect", { terminateDebuggee: true }),
        timeout,
        "disconnect",
      );
    } catch {
      /* Ownership was revoked before any cleanup IO. */
    } finally {
      await this.bounded(session.client.dispose(), timeout, "adapter stop").catch(() => undefined);
    }
  }
  async start(config: DebugConfig): Promise<void> {
    // Prefer the project the UI has already switched to (set synchronously); extensionService.projectDir lags behind a debounce.
    const project = this.currentProject !== undefined
      ? this.currentProject
      : this.deps.projectDir ? this.deps.projectDir() : extensionService.projectDir;
    const capturedConfig = structuredClone(config);
    const adapter = resolveAdapter(capturedConfig);
    const previous = this.detach();
    const epoch = this.epoch;
    await this.cleanup(previous);
    if (this.epoch !== epoch) return;
    if (!adapter) {
      extensionUiStore.appendDebugOutput(
        `No debug adapter resolved for type "${capturedConfig.type}". Install a debug extension.`,
      );
      return;
    }
    const client = this.deps.createClient?.() ?? new DapClient();
    const session: Session = { client, project: project ?? null, epoch, pause: 0 };
    this.session = session;
    debugStore.start();
    extensionUiStore.appendDebugOutput(
      `Launching adapter: ${adapter.command} ${adapter.args.join(" ")}`,
    );
    let initializedResolve!: () => void;
    const initialized = new Promise<void>((resolve, reject) => {
      initializedResolve = resolve;
      session.rejectInitialized = reject;
    });
    // Revocation can occur while connect/initialize are still pending.
    void initialized.catch(() => undefined);
    client.onLog((line) => {
      if (this.owns(session)) extensionUiStore.appendDebugOutput(line);
    });
    client.onEvent((event, body) => {
      if (!this.owns(session)) return;
      if (event === "initialized") initializedResolve();
      void this.onEvent(session, event, body);
    });
    try {
      await client.connect(
        adapter.command,
        adapter.args,
        capturedConfig.cwd,
        isRemoteDir(project) ? (project ?? undefined) : undefined,
      );
      if (!this.owns(session)) return;
      const capabilities = await client.request<{ supportsConfigurationDoneRequest?: boolean }>(
        "initialize",
        {
          clientID: "agentz",
          adapterID: capturedConfig.type,
          linesStartAt1: true,
          columnsStartAt1: true,
          pathFormat: "path",
          supportsRunInTerminalRequest: false,
        },
      );
      if (!this.owns(session)) return;
      const launched = client.request(
        capturedConfig.request === "attach" ? "attach" : "launch",
        capturedConfig,
      );
      void launched.catch(() => undefined);
      await this.bounded(
        Promise.race([initialized, launched.then(() => initialized)]),
        this.deps.initializedTimeoutMs ?? 10000,
        "initialized event",
      );
      if (!this.owns(session)) return;
      // Snapshot breakpoints once; no other project's paths or recovery drafts reach the adapter.
      const breakpoints = [...breakpointStore.all()].map(
        ([path, lines]) => [path, [...lines]] as const,
      );
      for (const [path, lines] of breakpoints) {
        if (!withinRoot(path, project)) continue;
        await client.request("setBreakpoints", {
          source: { path, name: path.split(/[\\/]/).pop() },
          breakpoints: lines.map((line) => ({ line })),
        });
        if (!this.owns(session)) return;
      }
      if (capabilities?.supportsConfigurationDoneRequest === true) {
        await client.request("configurationDone");
        if (!this.owns(session)) return;
      }
      await launched;
    } catch (error) {
      if (!this.owns(session)) return;
      extensionUiStore.appendDebugOutput(`Debug start failed: ${String(error)}`);
      await this.stop();
    } finally {
      session.rejectInitialized = undefined;
    }
  }
  private async onEvent(session: Session, event: string, body: unknown): Promise<void> {
    if (!this.owns(session)) return;
    const b = (body ?? {}) as Record<string, unknown>;
    if (event === "output") {
      const text = String(b.output ?? "");
      extensionUiStore.appendDebugOutput(text);
      debugStore.appendOutput(text);
    } else if (event === "continued") {
      session.pause += 1;
      session.stepPause = undefined;
      debugStore.setRunning(true);
    } else if (event === "stopped") {
      const threadId = Number(b.threadId ?? 1);
      if (Number.isSafeInteger(threadId) && threadId > 0) await this.onStopped(session, threadId);
    } else if (event === "terminated" || event === "exited") {
      extensionUiStore.appendDebugOutput("Debug session ended.");
      await this.stop();
    }
  }
  private async onStopped(session: Session, threadId: number): Promise<void> {
    const pause = ++session.pause;
    session.stepPause = undefined;
    const current = () => this.owns(session) && session.pause === pause;
    debugStore.setStopped(threadId, []);
    try {
      const stack = await session.client.request<{
        stackFrames?: { id: number; name: string; line: number; source?: { path?: string } }[];
      }>("stackTrace", { threadId, levels: 20 });
      if (!current()) return;
      const frames: StackFrame[] = (stack.stackFrames ?? []).map((frame) => ({
        id: frame.id,
        name: frame.name,
        line: frame.line,
        source: frame.source?.path,
      }));
      debugStore.setStopped(threadId, frames);
      if (!frames[0]) return;
      const scopes = await session.client.request<{ scopes?: { variablesReference: number }[] }>(
        "scopes",
        { frameId: frames[0].id },
      );
      if (!current()) return;
      const reference = scopes.scopes?.[0]?.variablesReference;
      if (!reference) return;
      const variables = await session.client.request<{ variables?: Variable[] }>("variables", {
        variablesReference: reference,
      });
      if (current()) debugStore.setVariables(variables.variables ?? []);
    } catch (error) {
      if (current())
        extensionUiStore.appendDebugOutput(`Paused inspection failed: ${String(error)}`);
    }
  }
  async continue(): Promise<void> {
    await this.step("continue");
  }
  async stepOver(): Promise<void> {
    await this.step("next");
  }
  async stepInto(): Promise<void> {
    await this.step("stepIn");
  }
  async stepOut(): Promise<void> {
    await this.step("stepOut");
  }
  private async step(command: string): Promise<void> {
    const session = this.session;
    if (!session) return;
    const snapshot = debugStore.getSnapshot();
    if (snapshot.running || snapshot.threadId === null) return;
    if (session.stepPause !== undefined) return;
    const pause = session.pause;
    session.stepPause = pause;
    try {
      await session.client.request(command, { threadId: snapshot.threadId });
      if (this.owns(session) && session.pause === pause) {
        session.pause += 1;
        debugStore.setRunning(true);
      }
    } catch (error) {
      if (this.owns(session) && session.pause === pause)
        extensionUiStore.appendDebugOutput(`Debug step failed: ${String(error)}`);
    } finally {
      if (session.stepPause === pause) session.stepPause = undefined;
    }
  }
  async stop(): Promise<void> {
    await this.cleanup(this.detach());
  }
  /** Stops the active session once the project it was started for is no longer current. */
  async stopIfProjectChanged(project: string | null): Promise<void> {
    const changed = this.currentProject !== undefined && this.currentProject !== project;
    this.currentProject = project;
    // Invalidate any start() still awaiting cleanup for the previous project, so it cannot create a session.
    if (changed) this.epoch += 1;
    if (!this.session || this.session.project === project) return;
    await this.stop();
  }
  /** Project-relative path for a stack frame source, or null when it is outside the session project. */
  projectRelativePath(source: string | undefined): string | null {
    return projectRelative(source, this.session?.project ?? null);
  }
}

export const debugController = new DebugController();
