import { beforeEach, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { readTaskDetails } from "./taskDetails";
const scope = { project_id: "host-project", task_id: "task", environment_id: "env", binding_version: "a".repeat(64) };
function dto() { return { scope: { ...scope }, atomic_snapshot: false, sources: {
  environment: {status:"missing"},run:{status:"missing"},plan:{status:"stale",version:"plan-v1:7"},
  events:{status:"unknown"},budget:{status:"unknown"},artifacts:{status:"unsupported"},
} }; }
function fullPlan() { return { id:"plan", binding:{...scope}, revision:2, objective:"keep goal", items:[{id:"step",description:"unfinished",state:"pending"}],status:"cancelled",created_ms:1,updated_ms:2 }; }
beforeEach(() => { invoke.mockReset(); });
it("sends only opaque scope and existing task; preserves unknown/missing/stale statuses",async()=>{
  invoke.mockResolvedValue(dto());const result=await readTaskDetails("opaque","task");
  expect(invoke).toHaveBeenCalledWith("chat_task_details",{scopeHandle:"opaque",sessionId:"task"});
  expect(result.atomic_snapshot).toBe(false);expect(result.sources.budget).toEqual({status:"unknown"});expect(result.sources.plan).toEqual({status:"stale",version:"plan-v1:7"});
});
it("accepts host versioned unavailable/stale sources and scoped artifact references",async()=>{
  const artifacts={status:"available",version:"artifacts-v1:abc",value:{task_id:"task",environment_id:"env",ids:["research-report"]}};
  invoke.mockResolvedValue({...dto(),sources:{...dto().sources,environment:{status:"unavailable",version:"environment-unregistered:v1"},artifacts}});
  const result=await readTaskDetails("opaque","task");
  expect(result.sources.environment).toEqual({status:"unavailable",version:"environment-unregistered:v1"});
  expect(result.sources.artifacts).toEqual(artifacts);
  invoke.mockResolvedValue({...dto(),sources:{...dto().sources,artifacts:{...artifacts,value:{...artifacts.value,task_id:"other"}}}});
  await expect(readTaskDetails("opaque","task")).rejects.toThrow();
});
it("preserves complete Cancelled plan without clearing its unfinished items",async()=>{
  const value={...dto(),sources:{...dto().sources,plan:{status:"available",value:fullPlan(),version:"plan-v1:2"}}};
  invoke.mockResolvedValue(value);const result=await readTaskDetails("opaque","task");expect(result.sources.plan).toEqual(value.sources.plan);
});
it("rejects other task/environment/binding and unsupported atomicity",async()=>{
  for (const value of [ {...dto(),scope:{...scope,task_id:"other"}}, {...dto(),atomic_snapshot:true}, {...dto(),sources:{...dto().sources,plan:{status:"available",value:{...fullPlan(),binding:{...scope,environment_id:"old"}},version:"v1"}}} ]) {
    invoke.mockResolvedValue(value);await expect(readTaskDetails("opaque","task")).rejects.toThrow("scope");
  }
});
it("rejects malformed or unsafe numbers and unknown source schema without zero fallback",async()=>{
  for (const plan of [{...fullPlan(),revision:Number.MAX_SAFE_INTEGER+1},{...fullPlan(),items:[{id:"step",description:"bad",state:"mystery"}]}]) {
    invoke.mockResolvedValue({...dto(),sources:{...dto().sources,plan:{status:"available",value:plan,version:"v1"}}});await expect(readTaskDetails("opaque","task")).rejects.toThrow();
  }
  invoke.mockResolvedValue({...dto(),sources:{...dto().sources,budget:{status:"unknown",value:0}}});await expect(readTaskDetails("opaque","task")).rejects.toThrow();
});
it("rejects event references without current Run or against an older Attempt",async()=>{
  const events={status:"available",version:"event-v1",value:{stream:{task_id:"task",environment_id:"env",run_id:"r",attempt_id:"old"},sequence:1}};
  invoke.mockResolvedValue({...dto(),sources:{...dto().sources,events}});await expect(readTaskDetails("opaque","task")).rejects.toThrow();
  const run={status:"available",version:"run-v1",value:{task_id:"task",environment_id:"env",run_id:"r",attempt_id:"current",execution:"running",delivery:"not_ready",updated_ms:1}};
  invoke.mockResolvedValue({...dto(),sources:{...dto().sources,events,run}});await expect(readTaskDetails("opaque","task")).rejects.toThrow();
});
it("rejects bad request before IPC and propagates host error once",async()=>{
  await expect(readTaskDetails(" ","task")).rejects.toThrow();expect(invoke).not.toHaveBeenCalled();
  const hostError = new Error("unsupported team task");
  invoke.mockRejectedValue(hostError);
  let received: unknown;
  try { await readTaskDetails("opaque", "task"); } catch (error) { received = error; }
  expect(received).toBe(hostError);
  expect(invoke.mock.calls.length).toBe(1);
});
