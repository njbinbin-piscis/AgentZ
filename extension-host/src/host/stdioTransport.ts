// Line-delimited-JSON transport over a Node stream pair (stdin/stdout). The
// Tauri host brokers these to the renderer's RPC protocol.

import { ITransport, RpcMessage } from "../common/rpcProtocol";

export class StdioTransport implements ITransport {
  private handler: ((m: RpcMessage) => void) | undefined;
  private controlHandler: ((m: unknown) => boolean) | undefined;
  private buffer = "";

  constructor(
    private readonly input: NodeJS.ReadableStream,
    private readonly output: NodeJS.WritableStream,
  ) {
    this.input.setEncoding?.("utf8");
    this.input.on("data", (chunk: string) => this.onData(chunk));
    // The broker (local Tauri or the ssh session) is gone: an orphaned host
    // would otherwise spin on EPIPE writes forever.
    const shutdown = () => process.exit(0);
    this.input.on("end", shutdown);
    this.input.on("close", shutdown);
    this.output.on("error", shutdown);
  }

  send(message: RpcMessage): void {
    this.writeFrame(message);
  }

  writeFrame(frame: unknown): void {
    this.output.write(JSON.stringify(frame) + "\n");
  }

  onMessage(handler: (m: RpcMessage) => void): void {
    this.handler = handler;
  }

  /** Intercepts out-of-band frames; return true to consume the frame. */
  onControl(handler: (m: unknown) => boolean): void {
    this.controlHandler = handler;
  }

  private onData(chunk: string): void {
    this.buffer += chunk;
    let idx: number;
    while ((idx = this.buffer.indexOf("\n")) >= 0) {
      const line = this.buffer.slice(0, idx).trim();
      this.buffer = this.buffer.slice(idx + 1);
      if (!line) continue;
      try {
        const msg = JSON.parse(line);
        if (this.controlHandler?.(msg)) continue;
        this.handler?.(msg as RpcMessage);
      } catch (err) {
        process.stderr.write(`[host] bad RPC line: ${String(err)}\n`);
      }
    }
  }
}
