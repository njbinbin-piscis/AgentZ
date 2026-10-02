// Debug Adapter Protocol client. Speaks DAP over the Tauri DAP broker: requests
// go out via the `dap_send` command; responses + events arrive on `agentz:dap`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface DapEvent {
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
  private seq = 1;
  private unlisten: UnlistenFn | undefined;
  private pending = new Map<number, { resolve: (b: unknown) => void; reject: (e: Error) => void }>();
  private eventHandler: EventHandler | undefined;
  private logHandler: LogHandler | undefined;
  /** `agentz-remote://<authority>` when the adapter runs on a remote. */
  private remotePrefix: string | null = null;

  async connect(command: string, args: string[], cwd?: string, remoteDir?: string): Promise<void> {
    this.unlisten = await listen<DapEvent>("agentz:dap", (e) => {
      const { channel, data } = e.payload;
      if (channel === "message") this.onMessage(data);
      else if (channel === "log") this.logHandler?.(data);
      else if (channel === "exit") this.eventHandler?.("exited", { reason: data });
    });
    if (remoteDir) {
      const m = /^agentz-remote:\/\/[^/]+/.exec(remoteDir);
      this.remotePrefix = m ? m[0] : null;
    }
    await invoke("dap_start", { command, args, cwd, remoteDir: remoteDir ?? null });
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
    } catch {
      return;
    }
    if (this.remotePrefix) msg = fromAdapterPaths(msg, this.remotePrefix) as DapMessage;
    if (msg.type === "response") {
      const reqSeq = msg.request_seq as number;
      const pending = this.pending.get(reqSeq);
      if (pending) {
        this.pending.delete(reqSeq);
        if (msg.success) pending.resolve(msg.body);
        else pending.reject(new Error((msg.message as string) ?? "DAP request failed"));
      }
    } else if (msg.type === "event") {
      this.eventHandler?.(msg.event as string, msg.body);
    }
  }

  request<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
    const seq = this.seq++;
    const payload = this.remotePrefix ? toAdapterPaths(args ?? {}, this.remotePrefix) : (args ?? {});
    const message = JSON.stringify({ seq, type: "request", command, arguments: payload });
    return new Promise<T>((resolve, reject) => {
      this.pending.set(seq, { resolve: resolve as (b: unknown) => void, reject });
      void invoke("dap_send", { message }).catch(reject);
    });
  }

  async dispose(): Promise<void> {
    this.unlisten?.();
    this.unlisten = undefined;
    for (const p of this.pending.values()) p.reject(new Error("DAP disposed"));
    this.pending.clear();
    try {
      await invoke("dap_stop");
    } catch {
      /* ignore */
    }
  }
}
