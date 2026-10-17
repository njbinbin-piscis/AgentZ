import { invoke } from "@tauri-apps/api/core";
import type { DurablePlanBinding } from "./chatPlan";
import type { TaskViewObservation } from "./taskView";

export interface TaskFileEntry { relative_path: string; is_dir: boolean; size: number }
export interface TaskFileList { scope: DurablePlanBinding; workspace_root: string; entries: TaskFileEntry[]; truncated: boolean }
export interface TaskFileText { scope: DurablePlanBinding; relative_path: string; content: string; bytes: number }
function object(value: unknown, keys: string[]): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value) || Object.getPrototypeOf(value) !== Object.prototype) throw new Error("Invalid task file response");
  const result = value as Record<string, unknown>;
  if (Object.keys(result).length !== keys.length || keys.some(key => !Object.prototype.hasOwnProperty.call(result, key))) throw new Error("Invalid task file shape");
  return result;
}
function relative(value: unknown, root = false): value is string {
  return typeof value === "string" && (root && value === "" || value.length > 0 && new TextEncoder().encode(value).length <= 4096 && !/[\\:\p{Cc}]/u.test(value) && !value.startsWith("/") && value.split("/").every(part => part !== "" && part !== "." && part !== ".."));
}
function scope(value: unknown, expected: DurablePlanBinding): void {
  const actual = object(value, ["project_id", "task_id", "environment_id", "binding_version"]);
  if (Object.keys(actual).some(key => actual[key] !== expected[key as keyof DurablePlanBinding])) throw new Error("Task file scope changed");
}
function request(handle: string, path: string, root = false): void {
  if (!handle.trim() || handle.trim() !== handle || handle.length > 256 || /\p{Cc}/u.test(handle) || !relative(path, root)) throw new Error("Invalid task file request");
}
export async function listTaskFiles(scopeHandle: string, view: TaskViewObservation, relativePath = ""): Promise<TaskFileList> {
  request(scopeHandle, relativePath, true);
  const result = object(await invoke("task_view_list", { scopeHandle, sessionId: view.session_id, expectedBinding: view.scope, relativePath }), ["scope", "workspace_root", "entries", "truncated"]);
  scope(result.scope, view.scope);
  if (result.workspace_root !== view.workspace_root || typeof result.truncated !== "boolean" || !Array.isArray(result.entries) || result.entries.length > 1000) throw new Error("Invalid task file list");
  const seen = new Set<string>();
  for (const entry of result.entries) {
    const value = object(entry, ["relative_path", "is_dir", "size"]);
    if (!relative(value.relative_path) || seen.has(value.relative_path) || typeof value.is_dir !== "boolean" || !Number.isSafeInteger(value.size) || Number(value.size) < 0) throw new Error("Invalid task file entry");
    seen.add(value.relative_path);
  }
  return result as unknown as TaskFileList;
}
export async function readTaskFile(scopeHandle: string, view: TaskViewObservation, relativePath: string): Promise<TaskFileText> {
  request(scopeHandle, relativePath);
  const result = object(await invoke("task_view_read", { scopeHandle, sessionId: view.session_id, expectedBinding: view.scope, relativePath }), ["scope", "relative_path", "content", "bytes"]);
  scope(result.scope, view.scope);
  if (result.relative_path !== relativePath || typeof result.content !== "string" || !Number.isSafeInteger(result.bytes) || Number(result.bytes) < 0 || Number(result.bytes) > 1024 * 1024 || new TextEncoder().encode(result.content).length !== result.bytes) throw new Error("Invalid task file content");
  return result as unknown as TaskFileText;
}
