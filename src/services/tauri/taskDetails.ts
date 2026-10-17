import { invoke } from "@tauri-apps/api/core";
import type { DurablePlan, DurablePlanBinding } from "./chatPlan";
import type { CurrentTaskRun } from "./runDiscovery";
import type { RunEventCursor } from "./runEvents";

export type TaskDetailSource<T> =
  | { status: "available"; value: T; version: string }
  | { status: "missing" | "unsupported" | "unavailable" | "stale" | "unknown"; version?: string };
export interface TaskDetailEnvironment {
  schema_version: number; task_id: string; environment_id: string;
  project_root: string; workspace_root: string; kind: string; branch: string | null;
  base_ref: string; base_sha: string; created_at: string;
}
export interface ObserverBudgetSnapshot {
  root_task_id: string;
  policy: { revision: number; max_total_tokens: number | null; max_active_ms: number | null;
    max_cost: { currency: string; micro_units: number } | null };
  usage_count: number; known_input_tokens: number; known_output_tokens: number;
  unknown_token_records: number; known_active_ms: number; unknown_duration_records: number;
  known_cost_micro_units: Record<string, number>; unknown_price_records: number;
  observed_limits_exceeded: string[];
}
export interface TaskDetailArtifacts { task_id: string; environment_id: string; ids: string[] }
export interface TaskDetails {
  scope: DurablePlanBinding; atomic_snapshot: false;
  sources: {
    environment: TaskDetailSource<TaskDetailEnvironment>; run: TaskDetailSource<CurrentTaskRun>;
    plan: TaskDetailSource<DurablePlan>; events: TaskDetailSource<RunEventCursor>;
    budget: TaskDetailSource<ObserverBudgetSnapshot>; artifacts: TaskDetailSource<TaskDetailArtifacts>;
  };
}
function fail(): never { throw new Error("Invalid task detail response or scope"); }
function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.getPrototypeOf(value) !== Object.prototype) fail();
  return value as Record<string, unknown>;
}
function exact(value: Record<string, unknown>, keys: string[]): void {
  if (Object.keys(value).length !== keys.length || keys.some(key => !Object.prototype.hasOwnProperty.call(value, key))) fail();
}
function text(value: unknown, max = 65536, allowEmpty = false): asserts value is string {
  if (typeof value !== "string" || (!allowEmpty && !value.trim()) || new TextEncoder().encode(value).length > max) fail();
}
function identity(value: unknown): asserts value is string {
  text(value, 256);
  if (value.trim() !== value || /\p{Cc}/u.test(value)) fail();
}
function integer(value: unknown, min = 0): asserts value is number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < min) fail();
}
function binding(value: unknown): DurablePlanBinding {
  const v = object(value); exact(v, ["project_id", "task_id", "environment_id", "binding_version"]);
  text(v.project_id); identity(v.task_id); identity(v.environment_id);
  if (typeof v.binding_version !== "string" || !/^[a-fA-F0-9]{64}$/.test(v.binding_version)) fail();
  return v as unknown as DurablePlanBinding;
}
function scopeMatch(v: Record<string, unknown>, s: DurablePlanBinding): void {
  if (v.task_id !== s.task_id || v.environment_id !== s.environment_id) fail();
}
function source<T>(value: unknown, validate: (value: unknown) => T): TaskDetailSource<T> {
  const v = object(value);
  if (v.status === "available") {
    exact(v, ["status", "value", "version"]); identity(v.version);
    return { status: "available", value: validate(v.value), version: v.version };
  }
  if (v.status === "unavailable" || v.status === "stale") {
    exact(v, ["status", "version"]); identity(v.version);
    return v as unknown as TaskDetailSource<T>;
  }
  if (!["missing", "unsupported", "unknown"].includes(String(v.status))) fail();
  exact(v, ["status"]);
  return v as unknown as TaskDetailSource<T>;
}
function environment(value: unknown, s: DurablePlanBinding): TaskDetailEnvironment {
  const v = object(value); exact(v, ["schema_version", "task_id", "environment_id", "project_root", "workspace_root", "kind", "branch", "base_ref", "base_sha", "created_at"]);
  scopeMatch(v,s); if (v.schema_version !== 1 || v.project_root !== s.project_id) fail();
  text(v.workspace_root); text(v.kind); text(v.base_ref,65536,true);text(v.base_sha,65536,true);text(v.created_at);
  if (v.branch !== null) text(v.branch);
  return v as unknown as TaskDetailEnvironment;
}
function run(value: unknown, s: DurablePlanBinding): CurrentTaskRun {
  const v = object(value);exact(v,["run_id","task_id","environment_id","attempt_id","execution","delivery","updated_ms"]);
  scopeMatch(v,s);identity(v.run_id);identity(v.attempt_id);integer(v.updated_ms);
  if (!["queued","running","cancelling","succeeded","failed","cancelled","interrupted"].includes(String(v.execution)) || !["not_ready","awaiting_review","accepted","rejected"].includes(String(v.delivery))) fail();
  if ((v.execution === "succeeded") === (v.delivery === "not_ready")) fail();
  return v as unknown as CurrentTaskRun;
}
function plan(value: unknown, s: DurablePlanBinding): DurablePlan {
  const v=object(value);exact(v,["id","binding","revision","objective","items","status","created_ms","updated_ms"]);
  const b=binding(v.binding);if (Object.keys(s).some(key=>b[key as keyof DurablePlanBinding] !== s[key as keyof DurablePlanBinding])) fail();
  text(v.id);text(v.objective);integer(v.revision,1);integer(v.created_ms);integer(v.updated_ms);if (v.updated_ms<v.created_ms) fail();
  if (!Array.isArray(v.items) || v.items.length>500) fail();
  const ids=new Set<string>();let open=false;
  for (const entry of v.items) {const i=object(entry);exact(i,["id","description","state"]);text(i.id);text(i.description);if (ids.has(i.id)) fail();ids.add(i.id);if (!["pending","in_progress","done","cancelled"].includes(String(i.state))) fail();open ||= i.state==="pending" || i.state==="in_progress";}
  if (!["active","completed","cancelled","replaced"].includes(String(v.status))) fail();
  if (v.status==="active" && v.items.length>0 && !open || v.status==="completed" && (!v.items.length || open)) fail();
  return v as unknown as DurablePlan;
}
function budget(value:unknown,s:DurablePlanBinding):ObserverBudgetSnapshot {
  const v=object(value);exact(v,["root_task_id","policy","usage_count","known_input_tokens","known_output_tokens","unknown_token_records","known_active_ms","unknown_duration_records","known_cost_micro_units","unknown_price_records","observed_limits_exceeded"]);
  if (v.root_task_id!==s.task_id) fail();
  const p=object(v.policy);exact(p,["revision","max_total_tokens","max_active_ms","max_cost"]);integer(p.revision,1);
  for (const k of ["max_total_tokens","max_active_ms"]) if (p[k]!==null) integer(p[k]);
  if (p.max_cost!==null) {const m=object(p.max_cost);exact(m,["currency","micro_units"]);if (typeof m.currency!=="string" || !/^[A-Z]{3}$/.test(m.currency)) fail();integer(m.micro_units);}
  for (const k of ["usage_count","known_input_tokens","known_output_tokens","unknown_token_records","known_active_ms","unknown_duration_records","unknown_price_records"]) integer(v[k]);
  for (const k of ["unknown_token_records","unknown_duration_records","unknown_price_records"]) if ((v[k] as number)>(v.usage_count as number)) fail();
  const costs=object(v.known_cost_micro_units);if (Object.keys(costs).length>200) fail();for (const [currency,amount] of Object.entries(costs)) {if (!/^[A-Z]{3}$/.test(currency)) fail();integer(amount);}
  if (!Array.isArray(v.observed_limits_exceeded) || v.observed_limits_exceeded.length>10) fail();v.observed_limits_exceeded.forEach(x=>text(x,256));
  return v as unknown as ObserverBudgetSnapshot;
}
/** Passive host-authorized query only. Unknown amounts/statuses never become zero. */
export async function readTaskDetails(scopeHandle:string,sessionId:string):Promise<TaskDetails> {
  identity(scopeHandle);identity(sessionId);
  const value=object(await invoke<unknown>("chat_task_details",{scopeHandle,sessionId}));
  exact(value,["scope","atomic_snapshot","sources"]);if (value.atomic_snapshot!==false) fail();
  const s=binding(value.scope);if (s.task_id!==sessionId) fail();
  const inputs=object(value.sources);exact(inputs,["environment","run","plan","events","budget","artifacts"]);
  const environmentValue=source(inputs.environment,v=>environment(v,s));
  const runValue=source(inputs.run,v=>run(v,s));
  const planValue=source(inputs.plan,v=>plan(v,s));
  const eventsValue=source<RunEventCursor>(inputs.events,v=>{
    const c=object(v);exact(c,["stream","sequence"]);integer(c.sequence);
    const stream=object(c.stream);exact(stream,["task_id","environment_id","run_id","attempt_id"]);scopeMatch(stream,s);identity(stream.run_id);identity(stream.attempt_id);
    if (runValue.status!=="available" || stream.run_id!==runValue.value.run_id || stream.attempt_id!==runValue.value.attempt_id) fail();
    return c as unknown as RunEventCursor;
  });
  const budgetValue=source(inputs.budget,v=>budget(v,s));
  const artifactsValue=source<TaskDetailArtifacts>(inputs.artifacts,v=>{
    const a=object(v);exact(a,["task_id","environment_id","ids"]);scopeMatch(a,s);
    if (!Array.isArray(a.ids) || a.ids.length>200) fail();a.ids.forEach(identity);if (new Set(a.ids).size!==a.ids.length) fail();return a as unknown as TaskDetailArtifacts;
  });
  return {scope:s,atomic_snapshot:false,sources:{environment:environmentValue,run:runValue,plan:planValue,events:eventsValue,budget:budgetValue,artifacts:artifactsValue}};
}
