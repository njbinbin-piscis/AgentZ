// Debug Adapter Protocol client. Speaks DAP over the Tauri DAP broker: requests
// go out via the `dap_send` command; responses + events arrive on `agentz:dap`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface DapEvent {
  session_id: string;
  channel: "message" | "log" | "exit";
  data: string;
}

export interface DapMessage {
  seq: number;
  type: "request" | "response" | "event";
  [key: string]: unknown;
}

type EventHandler = (event: string, body: unknown) => void;
type LogHandler = (line: string) => void;

/**
 * Remote sessions: the adapter sees plain POSIX paths while the IDE uses
 * `agentz-remote://<authority>/…`. Outgoing strings lose the prefix; incoming
 * `path` fields (sources, stack frames, breakpoints) gain it.
 */
export function toAdapterPaths(v: unknown, prefix: string): unknown {
  if (typeof v === "string") {
    return v.toLowerCase().startsWith(prefix.toLowerCase()) ? v.slice(prefix.length) || "/" : v;
  }
  if (Array.isArray(v)) return v.map((x) => toAdapterPaths(x, prefix));
  if (v && typeof v === "object") {
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, toAdapterPaths(x, prefix)]));
  }
  return v;
}

export function fromAdapterPaths(v: unknown, prefix: string): unknown {
  if (Array.isArray(v)) return v.map((x) => fromAdapterPaths(x, prefix));
  if (v && typeof v === "object") {
    return Object.fromEntries(
      Object.entries(v).map(([k, x]) => [
        k,
        k === "path" && typeof x === "string" && x.startsWith("/") ? prefix + x : fromAdapterPaths(x, prefix),
      ]),
    );
  }
  return v;
}

export class DapClient {
  readonly sessionId = crypto.randomUUID();
  private seq = 1;
  private unlisten: UnlistenFn | undefined;
  private pending = new Map<number, { resolve: (b: unknown) => void; reject: (e: Error) => void; timer: ReturnType<typeof setTimeout> }>();
  private state: "new" | "connecting" | "running" | "terminal" = "new";
  private readonly requestTimeoutMs: number;
  private readonly stopTimeoutMs: number;
  private readonly connectTimeoutMs: number;
  private eventHandler: EventHandler | undefined;
  private logHandler: LogHandler | undefined;
  /** `agentz-remote://<authority>` when the adapter runs on a remote. */
  private remotePrefix: string | null = null;

  constructor(options: { requestTimeoutMs?: number; stopTimeoutMs?: number; connectTimeoutMs?: number } = {}) {
    this.requestTimeoutMs = options.requestTimeoutMs ?? 10_000;
    this.stopTimeoutMs = options.stopTimeoutMs ?? 2_000;
    this.connectTimeoutMs = options.connectTimeoutMs ?? 10_000;
    if (![this.requestTimeoutMs, this.stopTimeoutMs, this.connectTimeoutMs].every(n => Number.isSafeInteger(n) && n > 0 && n <= 60_000)) throw new Error("Invalid DAP deadline");
  }

  private revoke(reason: string): void {
    this.state = "terminal";
    this.unlisten?.(); this.unlisten = undefined;
    for (const p of this.pending.values()) { clearTimeout(p.timer); p.reject(new Error(reason)); }
    this.pending.clear();
  }
  private isTerminal(): boolean { return this.state === "terminal"; }
  private async connectPhase<T>(operation: Promise<T>, late: (value: T) => void): Promise<T> {
    let expired = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    // Keep a continuation on the real IPC promise: a race alone would leak late ownership.
    void operation.then(value => { if (expired) late(value); }, () => {});
    try {
      return await Promise.race([operation, new Promise<never>((_, reject) => {
        timer = setTimeout(() => { expired = true; reject(new Error("DAP connect timed out")); }, this.connectTimeoutMs);
      })]);
    } finally { if (timer !== undefined) clearTimeout(timer); }
  }
  private async stopOwned(): Promise<void> {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      await Promise.race([invoke("dap_stop", { sessionId: this.sessionId }), new Promise<void>(resolve => { timer = setTimeout(resolve, this.stopTimeoutMs); })]);
    } catch { /* Cleanup cannot claim the adapter stopped on an IPC failure. */ }
    finally { if (timer !== undefined) clearTimeout(timer); }
  }

  async connect(command: string, args: string[], cwd?: string, remoteDir?: string): Promise<void> {
    if (this.state !== "new") throw new Error("DAP client already used");
    this.state = "connecting";
    try {
    const unlisten = await this.connectPhase(listen<DapEvent>("agentz:dap", (e) => {
      if (this.state === "terminal" || !e.payload || e.payload.session_id !== this.sessionId || typeof e.payload.data !== "string") return;
      const { channel, data } = e.payload;
      if (channel === "message") this.onMessage(data);
      else if (channel === "log") this.logHandler?.(data);
      else if (channel === "exit") { this.revoke("DAP exited"); this.eventHandler?.("exited", { reason: data }); }
    }), lateUnlisten => { lateUnlisten(); });
    if (this.isTerminal()) { unlisten(); throw new Error("DAP disposed during connect"); }
    this.unlisten = unlisten;
    if (remoteDir) {
      const m = /^agentz-remote:\/\/[^/]+/.exec(remoteDir);
      this.remotePrefix = m ? m[0] : null;
    }
    const started = await this.connectPhase(invoke<{ running: boolean; session_id: string }>("dap_start", { sessionId: this.sessionId, command, args, cwd, remoteDir: remoteDir ?? null }), () => { void this.stopOwned(); });
    if (this.isTerminal()) throw new Error("DAP disposed during start");
    if (!started || started.running !== true || started.session_id !== this.sessionId) throw new Error("DAP start identity mismatch");
    this.state = "running";
    } catch (error) { this.revoke("DAP connect failed"); await this.stopOwned(); throw error; }
  }

  onEvent(handler: EventHandler): void {
    this.eventHandler = handler;
  }
  onLog(handler: LogHandler): void {
    this.logHandler = handler;
  }

  private onMessage(raw: string): void {
    let msg: DapMessage;
    try {
      msg = JSON.parse(raw) as DapMessage;
      if (!msg || typeof msg !== "object" || !Number.isSafeInteger(msg.seq) || msg.seq < 0) return;
    } catch {
      return;
    }
    if (this.remotePrefix) msg = fromAdapterPaths(msg, this.remotePrefix) as DapMessage;
    if (msg.type === "response") {
      const reqSeq = msg.request_seq as number;
      if (!Number.isSafeInteger(reqSeq) || typeof msg.success !== "boolean") return;
      const pending = this.pending.get(reqSeq);
      if (pending) {
        this.pending.delete(reqSeq);
        clearTimeout(pending.timer);
        if (msg.success) pending.resolve(msg.body);
        else pending.reject(new Error((msg.message as string) ?? "DAP request failed"));
      }
    } else if (msg.type === "event") {
      if (typeof msg.event === "string") this.eventHandler?.(msg.event, msg.body);
    }
  }

  request<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
    if (this.state !== "running") return Promise.reject(new Error("DAP is not running"));
    if (!Number.isSafeInteger(this.seq) || this.seq === Number.MAX_SAFE_INTEGER) return Promise.reject(new Error("DAP sequence exhausted"));
    const seq = this.seq++;
    const payload = this.remotePrefix ? toAdapterPaths(args ?? {}, this.remotePrefix) : (args ?? {});
    const message = JSON.stringify({ seq, type: "request", command, arguments: payload });
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => { const p = this.pending.get(seq); if (p) { this.pending.delete(seq); p.reject(new Error("DAP request timed out")); } }, this.requestTimeoutMs);
      this.pending.set(seq, { resolve: resolve as (b: unknown) => void, reject, timer });
      void invoke("dap_send", { sessionId: this.sessionId, message }).catch(error => {
        const p = this.pending.get(seq); if (!p) return;
        this.pending.delete(seq); clearTimeout(p.timer); p.reject(error instanceof Error ? error : new Error(String(error)));
      });
    });
  }

  async dispose(): Promise<void> {
    this.revoke("DAP disposed");
    await this.stopOwned();
  }
}
