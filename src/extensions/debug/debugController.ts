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
  __adapter?: AdapterSpec;
  [key: string]: unknown;
}

/** `{port}` in `args` is replaced by the loopback port a `tcp` adapter listens on. */
export interface AdapterSpec {
  command: string;
  args?: string[];
  tcp?: boolean;
}

function resolveAdapter(config: DebugConfig): { command: string; args: string[]; tcp: boolean } | null {
  if (config.__adapter?.command) {
    return { command: config.__adapter.command, args: config.__adapter.args ?? [], tcp: config.__adapter.tcp === true };
  }
  // Common conventions; a real adapter binary must exist on PATH.
  switch (config.type) {
    case "python":
    case "debugpy":
      return { command: "python", args: ["-m", "debugpy.adapter"], tcp: false };
    case "go":
    case "delve":
      return { command: "dlv", args: ["dap", "--listen=127.0.0.1:{port}"], tcp: true };
    case "lldb":
    case "codelldb":
      return { command: "codelldb", args: [], tcp: false };
    // Node.js needs js-debug, which the launch UI downloads first and passes as __adapter.
    default:
      return null;
  }
}

export interface DebugClient {
  connect(command: string, args: string[], cwd?: string, remoteDir?: string, tcp?: boolean): Promise<void>;
  /** Another connection to the same adapter (js-debug opens one per debug target). */
  openChild?(): Promise<DebugClient>;
  onRequest?(handler: (command: string, args: unknown) => unknown): void;
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
  /** Extra debug-target connections of a multi-session adapter. */
  children: Set<DebugClient>;
  /** The connection whose thread is currently stopped; steps and evaluations go there. */
  focus?: DebugClient;
  initializeArgs: Record<string, unknown>;
  /** Project the session was started for; frames and breakpoints belong to it. */
  project: string | null;
  epoch: number;
  pause: number;
  stepPause?: number;
  rejectInitialized?: (error: Error) => void;
  /** True once configurationDone went out; breakpoint edits are pushed live from then on. */
  ready?: boolean;
  /** Files the adapter currently holds breakpoints for, so removed ones can be cleared. */
  sent?: Map<string, string>;
  unsubscribeBreakpoints?: () => void;
}

function notify(name: string, detail?: unknown): void {
  if (typeof window !== "undefined") window.dispatchEvent(new CustomEvent(name, { detail }));
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
      previous.unsubscribeBreakpoints?.();
      previous.unsubscribeBreakpoints = undefined;
    }
    debugStore.end();
    return previous;
  }
  private async cleanup(session: Session | undefined): Promise<void> {
    if (!session) return;
    const timeout = this.deps.stopTimeoutMs ?? 2000;
    await Promise.all(
      [...session.children].map(async (child) => {
        try {
          await this.bounded(child.request("disconnect", { terminateDebuggee: true }), timeout, "disconnect");
        } catch {
          /* best effort */
        } finally {
          await this.bounded(child.dispose(), timeout, "adapter stop").catch(() => undefined);
        }
      }),
    );
    session.children.clear();
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
    const session: Session = {
      client,
      children: new Set(),
      initializeArgs: this.initializeArgs(capturedConfig.type, adapter.tcp),
      project: project ?? null,
      epoch,
      pause: 0,
    };
    this.session = session;
    debugStore.start();
    notify("agentz-debug-started");
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
    this.wire(session, client, false, () => initializedResolve());
    try {
      await client.connect(
        adapter.command,
        adapter.args,
        capturedConfig.cwd,
        isRemoteDir(project) ? (project ?? undefined) : undefined,
        adapter.tcp,
      );
      if (!this.owns(session)) return;
      const capabilities = await client.request<{ supportsConfigurationDoneRequest?: boolean }>(
        "initialize",
        session.initializeArgs,
      );
      if (!this.owns(session)) return;
      // The adapter command is ours, not part of the debuggee's launch arguments.
      const launchArgs: Record<string, unknown> = { ...capturedConfig };
      delete launchArgs.__adapter;
      const launched = client.request(
        capturedConfig.request === "attach" ? "attach" : "launch",
        launchArgs,
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
      session.sent = new Map();
      for (const [path, lines] of breakpoints) {
        if (!withinRoot(path, project)) continue;
        await client.request("setBreakpoints", {
          source: { path, name: path.split(/[\\/]/).pop() },
          breakpoints: lines.map((line) => ({ line })),
        });
        session.sent.set(path, JSON.stringify([...lines].sort((a, b) => a - b)));
        if (!this.owns(session)) return;
      }
      if (capabilities?.supportsConfigurationDoneRequest === true) {
        await client.request("configurationDone");
        if (!this.owns(session)) return;
      }
      session.ready = true;
      session.unsubscribeBreakpoints = breakpointStore.subscribe(() => void this.pushBreakpoints(session));
      // Breakpoints toggled while the adapter was starting up.
      void this.pushBreakpoints(session);
      // Servers answer `launch` only once the program is up; failures end the session, success needs no wait.
      void launched.then(undefined, async (error) => {
        if (!this.owns(session)) return;
        extensionUiStore.appendDebugOutput(`Debug launch failed: ${String(error)}`);
        await this.stop();
      });
    } catch (error) {
      if (!this.owns(session)) return;
      extensionUiStore.appendDebugOutput(`Debug start failed: ${String(error)}`);
      if (capturedConfig.type === "python" || capturedConfig.type === "debugpy") {
        const py = adapter.command;
        extensionUiStore.appendDebugOutput(
          `Python debugging needs debugpy in the interpreter being used. Install it with: "${py}" -m pip install debugpy`,
        );
      } else if (capturedConfig.type === "go" || capturedConfig.type === "delve") {
        extensionUiStore.appendDebugOutput(
          "Go debugging needs Delve (dlv) on PATH. Install it with: go install github.com/go-delve/delve/cmd/dlv@latest",
        );
      } else if (capturedConfig.type === "node" || capturedConfig.type === "pwa-node") {
        extensionUiStore.appendDebugOutput("Node.js debugging needs node on PATH.");
      }
      await this.stop();
    } finally {
      session.rejectInitialized = undefined;
    }
  }
  private initializeArgs(type: string, startDebugging: boolean): Record<string, unknown> {
    return {
      clientID: "agentz",
      adapterID: type,
      linesStartAt1: true,
      columnsStartAt1: true,
      pathFormat: "path",
      supportsRunInTerminalRequest: false,
      supportsStartDebuggingRequest: startDebugging,
    };
  }
  /** Routes a connection's logs, events and reverse requests into the session. */
  private wire(session: Session, client: DebugClient, isChild: boolean, onInitialized: () => void): void {
    client.onLog((line) => {
      if (this.owns(session)) extensionUiStore.appendDebugOutput(line);
    });
    client.onEvent((event, body) => {
      if (!this.owns(session)) return;
      if (event === "initialized") onInitialized();
      void this.onEvent(session, event, body, client, isChild);
    });
    client.onRequest?.((command, args) => this.onReverseRequest(session, client, command, args));
  }
  private onReverseRequest(session: Session, client: DebugClient, command: string, args: unknown): unknown {
    if (!this.owns(session)) throw new Error("Debug session ended");
    if (command === "startDebugging" && client.openChild) {
      const a = (args ?? {}) as { request?: string; configuration?: Record<string, unknown> };
      void this.startChild(session, client, a.request === "attach" ? "attach" : "launch", a.configuration ?? {});
      return {};
    }
    throw new Error(`Unsupported request from the debug adapter: ${command}`);
  }
  /** A multi-session adapter asked for another debug target (e.g. the Node.js process). */
  private async startChild(
    session: Session,
    parent: DebugClient,
    request: "launch" | "attach",
    configuration: Record<string, unknown>,
  ): Promise<void> {
    let child: DebugClient | undefined;
    try {
      child = await parent.openChild!();
      if (!this.owns(session)) {
        await child.dispose();
        return;
      }
      const opened = child;
      session.children.add(opened);
      let initializedResolve!: () => void;
      const initialized = new Promise<void>((resolve) => (initializedResolve = resolve));
      this.wire(session, opened, true, () => initializedResolve());
      const capabilities = await opened.request<{ supportsConfigurationDoneRequest?: boolean }>(
        "initialize",
        session.initializeArgs,
      );
      const launched = opened.request(request, configuration);
      void launched.catch(() => undefined);
      await this.bounded(
        Promise.race([initialized, launched.then(() => initialized)]),
        this.deps.initializedTimeoutMs ?? 10000,
        "initialized event",
      );
      if (!this.owns(session)) return;
      for (const [path, lines] of breakpointStore.all()) {
        if (!withinRoot(path, session.project)) continue;
        await opened.request("setBreakpoints", {
          source: { path, name: path.split(/[\\/]/).pop() },
          breakpoints: [...lines].sort((a, b) => a - b).map((line) => ({ line })),
        });
      }
      if (capabilities?.supportsConfigurationDoneRequest === true) await opened.request("configurationDone");
      await launched;
    } catch (error) {
      if (this.owns(session)) extensionUiStore.appendDebugOutput(`Debug target failed to start: ${String(error)}`);
      if (child) {
        session.children.delete(child);
        await child.dispose().catch(() => undefined);
      }
    }
  }
  /** A debug target ended; the session lives on until the root connection ends. */
  private retireChild(session: Session, client: DebugClient): void {
    session.children.delete(client);
    if (session.focus === client) {
      session.focus = undefined;
      session.pause += 1;
      session.stepPause = undefined;
      debugStore.setRunning(true);
    }
    void this.bounded(client.dispose(), this.deps.stopTimeoutMs ?? 2000, "adapter stop").catch(() => undefined);
  }
  private active(session: Session): DebugClient {
    return session.focus ?? session.client;
  }
  private async onEvent(
    session: Session,
    event: string,
    body: unknown,
    client: DebugClient = session.client,
    isChild = false,
  ): Promise<void> {
    if (!this.owns(session)) return;
    const b = (body ?? {}) as Record<string, unknown>;
    if (event === "output") {
      const text = String(b.output ?? "");
      extensionUiStore.appendDebugOutput(text);
      debugStore.appendOutput(text);
    } else if (event === "continued") {
      if (session.focus && session.focus !== client) return;
      session.pause += 1;
      session.stepPause = undefined;
      debugStore.setRunning(true);
    } else if (event === "stopped") {
      const threadId = Number(b.threadId ?? 1);
      if (Number.isSafeInteger(threadId) && threadId > 0) await this.onStopped(session, threadId, client);
    } else if ((event === "terminated" || event === "exited") && isChild) {
      this.retireChild(session, client);
    } else if (event === "terminated" || event === "exited") {
      extensionUiStore.appendDebugOutput("Debug session ended.");
      await this.stop();
    }
  }
  private async onStopped(session: Session, threadId: number, client: DebugClient = session.client): Promise<void> {
    session.focus = client;
    const pause = ++session.pause;
    session.stepPause = undefined;
    const current = () => this.owns(session) && session.pause === pause;
    debugStore.setStopped(threadId, []);
    try {
      const stack = await client.request<{
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
      const top = projectRelative(frames[0].source, session.project);
      if (top) notify("agentz-debug-stopped", { path: top, line: frames[0].line });
      const scopes = await client.request<{ scopes?: { variablesReference: number }[] }>(
        "scopes",
        { frameId: frames[0].id },
      );
      if (!current()) return;
      const reference = scopes.scopes?.[0]?.variablesReference;
      if (!reference) return;
      const variables = await client.request<{ variables?: Variable[] }>("variables", {
        variablesReference: reference,
      });
      if (current()) debugStore.setVariables(variables.variables ?? []);
    } catch (error) {
      if (current())
        extensionUiStore.appendDebugOutput(`Paused inspection failed: ${String(error)}`);
    }
  }
  /** Re-sends the breakpoints of every file that changed since the adapter last heard of them. */
  private async pushBreakpoints(session: Session): Promise<void> {
    if (!this.owns(session) || !session.ready) return;
    const current = new Map<string, number[]>();
    for (const [path, lines] of breakpointStore.all()) {
      if (withinRoot(path, session.project)) current.set(path, [...lines].sort((a, b) => a - b));
    }
    const sent = session.sent ?? (session.sent = new Map());
    for (const path of new Set([...sent.keys(), ...current.keys()])) {
      const lines = current.get(path) ?? [];
      const key = JSON.stringify(lines);
      if ((sent.get(path) ?? "[]") === key) continue;
      try {
        for (const target of [session.client, ...session.children]) {
          await target.request("setBreakpoints", {
            source: { path, name: path.split(/[\\/]/).pop() },
            breakpoints: lines.map((line) => ({ line })),
          });
        }
        if (lines.length > 0) sent.set(path, key);
        else sent.delete(path);
      } catch (error) {
        if (this.owns(session))
          extensionUiStore.appendDebugOutput(`Updating breakpoints failed: ${String(error)}`);
      }
      if (!this.owns(session)) return;
    }
  }
  /** Children of a structured variable (only meaningful while paused). */
  async getChildren(variablesReference: number): Promise<Variable[]> {
    const session = this.session;
    if (!session || debugStore.getSnapshot().running || variablesReference <= 0) return [];
    const stop = debugStore.getSnapshot().stopSeq;
    const res = await this.active(session).request<{ variables?: Variable[] }>("variables", { variablesReference });
    if (!this.owns(session) || debugStore.getSnapshot().stopSeq !== stop) return [];
    return res.variables ?? [];
  }
  /** Evaluates an expression in the top frame; "repl" also echoes into the debug console. */
  async evaluate(
    expression: string,
    context: "watch" | "repl" = "watch",
  ): Promise<{ result: string; type?: string; variablesReference: number }> {
    const session = this.session;
    const snapshot = debugStore.getSnapshot();
    if (!session || snapshot.running || !snapshot.frames[0]) throw new Error("Not paused");
    const res = await this.active(session).request<{ result?: string; type?: string; variablesReference?: number }>(
      "evaluate",
      { expression, frameId: snapshot.frames[0].id, context },
    );
    return { result: res.result ?? "", type: res.type, variablesReference: res.variablesReference ?? 0 };
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
      await this.active(session).request(command, { threadId: snapshot.threadId });
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
