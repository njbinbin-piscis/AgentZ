// AgentZ extension host entrypoint. Launched by the Tauri host as a Node
// sidecar; speaks line-delimited-JSON RPC over stdin/stdout.

import { RPCProtocol } from "./common/rpcProtocol";
import { ExtHostContext } from "./common/protocol";
import { StdioTransport } from "./host/stdioTransport";
import { Services } from "./host/services";
import { ExtHostExtensionService } from "./host/extensionService";
import { AgentzServer } from "./server/agentzServer";

function log(msg: string): void {
  process.stderr.write(msg + "\n");
}

const redirectConsole = (...args: unknown[]): void => {
  const text = args
    .map((a) => {
      if (typeof a === "string") return a;
      if (a instanceof Error) return a.stack ?? a.message;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(" ");
  log(`[console] ${text}`);
};
console.log = redirectConsole;
console.info = redirectConsole;
console.debug = redirectConsole;
console.warn = redirectConsole;

function main(): void {
  log("[host] AgentZ extension host starting");

  const transport = new StdioTransport(process.stdin, process.stdout);
  const server = new AgentzServer((frame) => transport.writeFrame(frame));
  transport.onControl((msg) => {
    if (!AgentzServer.isControl(msg)) return false;
    void server.handle(msg);
    return true;
  });
  const rpc = new RPCProtocol(transport);

  const services = new Services(rpc);
  const extensionService = new ExtHostExtensionService(services, log);
  rpc.set(ExtHostContext.ExtHostExtensionService, extensionService);

  // Surface uncaught extension errors without killing the host.
  process.on("uncaughtException", (err) => log(`[host] uncaughtException: ${err?.stack ?? err}`));
  process.on("unhandledRejection", (reason) => log(`[host] unhandledRejection: ${String(reason)}`));

  log("[host] ready, awaiting $initialize");
}

main();
