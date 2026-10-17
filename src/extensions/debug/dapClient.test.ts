import { afterEach, beforeEach, expect, it, vi } from "vitest";
const { invoke, listen } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
import { DapClient } from "./dapClient";
let receive: (event: { payload: unknown }) => void;
let unlisten: ReturnType<typeof vi.fn>;
beforeEach(() => {
  vi.useFakeTimers(); invoke.mockReset(); listen.mockReset(); unlisten=vi.fn();
  listen.mockImplementation(async (_name, callback) => { receive=callback; return unlisten; });
  invoke.mockImplementation(async (command, args) => command === "dap_start" ? {running:true,session_id:args.sessionId} : undefined);
});
afterEach(() => { vi.useRealTimers(); });
function event(client:DapClient,channel:string,data:string,id=client.sessionId) { receive({payload:{session_id:id,channel,data}}); }
it("filters old session response and exit, and times out unanswered requests", async () => {
  const client=new DapClient({requestTimeoutMs:20});await client.connect("adapter",[]);
  const request=client.request("stackTrace");const rejected=expect(request).rejects.toThrow("timed out");
  event(client,"message",JSON.stringify({seq:1,type:"response",request_seq:1,success:true,body:"wrong"}),crypto.randomUUID());
  event(client,"exit","old",crypto.randomUUID());await vi.advanceTimersByTimeAsync(20);await rejected;
  expect(invoke).toHaveBeenCalledWith("dap_send",expect.objectContaining({sessionId:client.sessionId}));await client.dispose();
});
it("send failure clears pending timer and a late response cannot settle it", async () => {
  const client=new DapClient({requestTimeoutMs:20});await client.connect("adapter",[]);
  invoke.mockRejectedValueOnce(new Error("send failed"));await expect(client.request("threads")).rejects.toThrow("send failed");
  event(client,"message",JSON.stringify({seq:2,type:"response",request_seq:1,success:true,body:{}}));expect(vi.getTimerCount()).toBe(0);await client.dispose();
});
it("owned exit rejects all pending once and terminal requests never send", async () => {
  const client=new DapClient();const exited=vi.fn();client.onEvent(exited);await client.connect("adapter",[]);
  const pending=expect(client.request("threads")).rejects.toThrow("exited");event(client,"exit","eof");event(client,"exit","again");await pending;
  expect(exited).toHaveBeenCalledTimes(1);expect(unlisten).toHaveBeenCalledTimes(1);expect(vi.getTimerCount()).toBe(0);
  const count=invoke.mock.calls.length;await expect(client.request("threads")).rejects.toThrow("not running");expect(invoke.mock.calls.length).toBe(count);
});
it("dispose while listen is pending unlistens late registration without starting adapter", async () => {
  let finish!: (value:()=>void)=>void;listen.mockImplementation(() => new Promise(resolve => {finish=resolve;}));
  const client=new DapClient();const connecting=expect(client.connect("adapter",[])).rejects.toThrow("disposed");await client.dispose();finish(unlisten);await connecting;
  expect(unlisten).toHaveBeenCalledTimes(1);expect(invoke.mock.calls.every(([name,args])=>name==="dap_stop"&&args.sessionId===client.sessionId)).toBe(true);
});
it("dispose while start is pending cleans the same ID after late completion", async () => {
  let finish!: (value:unknown)=>void;invoke.mockImplementation((name) => name === "dap_start" ? new Promise(resolve=>{finish=resolve;}) : Promise.resolve());
  const client=new DapClient();const connecting=expect(client.connect("adapter",[])).rejects.toThrow("disposed");await vi.advanceTimersByTimeAsync(0);await client.dispose();finish({running:true,session_id:client.sessionId});await connecting;
  expect(invoke.mock.calls.filter(([name])=>name==="dap_stop").every(([,args])=>args.sessionId===client.sessionId)).toBe(true);expect(unlisten).toHaveBeenCalledTimes(1);
});
it("dispose revokes pending synchronously even when owned stop never answers", async () => {
  const client=new DapClient({stopTimeoutMs:30});await client.connect("adapter",[]);
  const pending=expect(client.request("threads")).rejects.toThrow("disposed");invoke.mockImplementation(()=>new Promise(()=>{}));const disposed=client.dispose();
  expect(unlisten).toHaveBeenCalledTimes(1);await pending;await vi.advanceTimersByTimeAsync(30);await disposed;expect(vi.getTimerCount()).toBe(0);
});
it("rejects a mismatched start reply and does not stop its foreign identity", async () => {
  invoke.mockResolvedValueOnce({running:true,session_id:crypto.randomUUID()});const client=new DapClient();
  await expect(client.connect("adapter",[])).rejects.toThrow("identity mismatch");expect(invoke).toHaveBeenLastCalledWith("dap_stop",{sessionId:client.sessionId});expect(unlisten).toHaveBeenCalledTimes(1);
});
it("unanswered listen settles by deadline and a late registration is unlistened", async () => {
  let finish!: (value:()=>void)=>void;listen.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  const client=new DapClient({connectTimeoutMs:20});const failure=expect(client.connect("adapter",[])).rejects.toThrow("connect timed out");
  await vi.advanceTimersByTimeAsync(20);await failure;
  expect(invoke).toHaveBeenCalledWith("dap_stop",{sessionId:client.sessionId});
  finish(unlisten);await Promise.resolve();expect(unlisten).toHaveBeenCalledTimes(1);
  expect(invoke.mock.calls.some(([name])=>name==="dap_start")).toBe(false);expect(vi.getTimerCount()).toBe(0);
});
it("unanswered start settles by deadline and late completion stops only its owned ID", async () => {
  let finish!: (value:unknown)=>void;invoke.mockImplementation((name)=>name==="dap_start"?new Promise(resolve=>{finish=resolve;}):Promise.resolve());
  const client=new DapClient({connectTimeoutMs:20});const failure=expect(client.connect("adapter",[])).rejects.toThrow("connect timed out");
  await vi.advanceTimersByTimeAsync(20);await failure;expect(unlisten).toHaveBeenCalledTimes(1);
  expect(invoke.mock.calls.filter(([name])=>name==="dap_stop")).toHaveLength(1);
  finish({running:true,session_id:client.sessionId});await Promise.resolve();await Promise.resolve();
  expect(invoke.mock.calls.filter(([name])=>name==="dap_stop")).toHaveLength(2);
  expect(invoke.mock.calls.filter(([name])=>name==="dap_stop").every(([,args])=>args.sessionId===client.sessionId)).toBe(true);
  await expect(client.request("threads")).rejects.toThrow("not running");
});
