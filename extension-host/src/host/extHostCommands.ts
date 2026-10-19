// ExtHostCommands: registers extension commands and dispatches both
// extension-defined and built-in command executions across the RPC boundary.

import { IRPCProtocol } from "../common/proxyIdentifier";
import { ExtHostCommandsShape, MainContext, MainThreadCommandsShape } from "../common/protocol";
import { Disposable } from "./types-impl";

type CommandCallback = (...args: unknown[]) => unknown;

export class ExtHostCommands implements ExtHostCommandsShape {
  private readonly proxy: MainThreadCommandsShape;
  private readonly commands = new Map<string, CommandCallback[]>();
  private readonly argumentConverters: ((arg: unknown) => unknown)[] = [];

  constructor(rpc: IRPCProtocol) {
    this.proxy = rpc.getProxy(MainContext.MainThreadCommands);
  }

  registerArgumentConverter(fn: (arg: unknown) => unknown): void {
    this.argumentConverters.push(fn);
  }

  registerCommand(id: string, callback: CommandCallback, thisArg?: unknown): Disposable {
    if (!id.trim()) throw new Error("invalid command id");
    const bound: CommandCallback = thisArg ? callback.bind(thisArg) : callback;
    const stack = this.commands.get(id);
    if (!stack) {
      this.commands.set(id, [bound]);
      this.proxy.$registerCommand(id);
    } else {
      stack.push(bound);
    }
    return new Disposable(() => {
      const cur = this.commands.get(id);
      if (!cur) return;
      const idx = cur.lastIndexOf(bound);
      if (idx >= 0) cur.splice(idx, 1);
      if (cur.length === 0) {
        this.commands.delete(id);
        this.proxy.$unregisterCommand(id);
      }
    });
  }

  async executeCommand<T>(id: string, ...args: unknown[]): Promise<T> {
    if (this.commands.has(id)) {
      return (await this.$executeContributedCommand(id, args)) as T;
    }
    // Fall through to the main thread (built-in command, or one registered by
    // another extension / the workbench).
    return (await this.proxy.$executeCommand(id, args)) as T;
  }

  getCommands(): Promise<string[]> {
    return Promise.resolve([...this.commands.keys()]);
  }

  async $executeContributedCommand(id: string, args: unknown[]): Promise<unknown> {
    const stack = this.commands.get(id);
    const callback = stack?.[stack.length - 1];
    if (!callback) {
      throw new Error(`command '${id}' not found`);
    }
    const converted = args.map((a) => this.applyConverters(a));
    return await callback(...converted);
  }

  private applyConverters(arg: unknown): unknown {
    let result = arg;
    for (const conv of this.argumentConverters) {
      result = conv(result);
    }
    return result;
  }
}
