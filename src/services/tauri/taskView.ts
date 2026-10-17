import { invoke } from "@tauri-apps/api/core";
import type { DurablePlanBinding } from "./chatPlan";

/** Observation only. Never use workspace_root as authorization for IDE actions. */
export interface TaskViewObservation {
  schema_version: 1;
  scope: DurablePlanBinding;
  session_id: string;
  authority_project: string;
  workspace_root: string;
  observation_only: true;
}
function record(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.getPrototypeOf(value) !== Object.prototype) throw new Error("Invalid task view");
  const object = value as Record<string, unknown>;
  if (Object.keys(object).length !== keys.length || keys.some(key => !Object.prototype.hasOwnProperty.call(object, key))) throw new Error("Invalid task view shape");
  return object;
}
function text(value: unknown, max: number): value is string {
  return typeof value === "string" && value.trim() === value && value.length > 0 && new TextEncoder().encode(value).length <= max && !/\p{Cc}/u.test(value);
}
export function validateTaskView(value: unknown, sessionId: string, authorityProject: string): TaskViewObservation {
  const view = record(value, ["schema_version", "scope", "session_id", "authority_project", "workspace_root", "observation_only"]);
  const scope = record(view.scope, ["project_id", "task_id", "environment_id", "binding_version"]);
  if (view.schema_version !== 1 || view.observation_only !== true || view.session_id !== sessionId || scope.task_id !== sessionId || view.authority_project !== authorityProject || scope.project_id !== authorityProject
    || !text(view.workspace_root, 65536) || !text(scope.environment_id, 256) || typeof scope.binding_version !== "string" || !/^[a-fA-F0-9]{64}$/.test(scope.binding_version)) throw new Error("Task view identity mismatch");
  return value as TaskViewObservation;
}
export async function observeTaskView(scopeHandle: string, sessionId: string, authorityProject: string): Promise<TaskViewObservation> {
  if (!text(scopeHandle, 256) || !text(sessionId, 256) || !text(authorityProject, 65536)) throw new Error("Invalid task view request");
  return validateTaskView(await invoke("task_view_resolve", { scopeHandle, sessionId }), sessionId, authorityProject);
}
