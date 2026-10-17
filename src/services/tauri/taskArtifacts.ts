import { invoke } from "@tauri-apps/api/core";
import type { DurablePlanBinding } from "./chatPlan";
import type { TaskViewObservation } from "./taskView";
export type ArtifactKind = "change_set" | "document" | "report" | "webpage" | "validation";
export type ArtifactCursor = [string, number];
export interface ArtifactMetadata {
  schema_version: 1; binding: DurablePlanBinding; artifact_id: string; version: number;
  kind: ArtifactKind; title: string; source_relative_path: string | null; media_type: string;
  payload_sha256: string; bytes: number; created_ms: number;
}
export interface ArtifactList { scope: DurablePlanBinding; store_missing: boolean; page: { versions: ArtifactMetadata[]; next_cursor: ArtifactCursor | null; has_more: boolean } }
export interface ArtifactText { scope: DurablePlanBinding; metadata: ArtifactMetadata; content: string }
export type DeleteResult = "deleted" | "already_deleted" | "not_found";
const kinds: ArtifactKind[] = ["change_set", "document", "report", "webpage", "validation"];
const bytes = (s: string) => new TextEncoder().encode(s);
function fail(): never { throw new Error("Invalid task artifact scope, version or response"); }
function object(v: unknown, keys: string[]): Record<string, unknown> {
  if (!v || typeof v !== "object" || Array.isArray(v) || Object.getPrototypeOf(v) !== Object.prototype) fail();
  const r = v as Record<string, unknown>;
  if (Object.keys(r).length !== keys.length || keys.some(k => !Object.prototype.hasOwnProperty.call(r, k))) fail();
  return r;
}
function text(v: unknown, max: number): asserts v is string { if (typeof v !== "string" || !v || v.trim() !== v || bytes(v).length > max || /\p{Cc}/u.test(v)) fail(); }
function integer(v: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): asserts v is number { if (!Number.isSafeInteger(v) || (v as number) < min || (v as number) > max) fail(); }
function id(v: unknown): asserts v is string { if (typeof v !== "string" || !/^[A-Za-z0-9._-]{1,256}$/.test(v)) fail(); }
function path(v: unknown): asserts v is string {
  text(v, 4096); if (v.includes(":") || v.includes("\\") || v.startsWith("/") || v.split("/").some(p => !p || p === "." || p === ".." || p.trim() !== p || p.endsWith("."))) fail();
}
function scope(v: unknown, expected: DurablePlanBinding, historical = false): DurablePlanBinding {
  const s = object(v, ["project_id", "task_id", "environment_id", "binding_version"]);
  text(s.project_id, 65536); text(s.task_id, 256); text(s.environment_id, 256);
  if (typeof s.binding_version !== "string" || !/^[a-fA-F0-9]{64}$/.test(s.binding_version) || s.project_id !== expected.project_id || s.task_id !== expected.task_id || s.environment_id !== expected.environment_id || (!historical && s.binding_version !== expected.binding_version)) fail();
  return s as unknown as DurablePlanBinding;
}
function request(handle: string, view: TaskViewObservation) {
  text(handle, 256); scope(view.scope, view.scope);
  if (view.schema_version !== 1 || view.observation_only !== true || view.session_id !== view.scope.task_id || view.authority_project !== view.scope.project_id) fail();
  return { scopeHandle: handle, sessionId: view.session_id, expectedBinding: { ...view.scope } };
}
function cursor(v: unknown): ArtifactCursor {
  if (!Array.isArray(v) || v.length !== 2) fail(); id(v[0]); integer(v[1], 1); return [v[0], v[1]];
}
function compare(a: ArtifactCursor, b: ArtifactCursor) { return a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : Math.sign(a[1] - b[1]); }
function metadata(value: unknown, expected: DurablePlanBinding): ArtifactMetadata {
  const m = object(value, ["schema_version", "binding", "artifact_id", "version", "kind", "title", "source_relative_path", "media_type", "payload_sha256", "bytes", "created_ms"]);
  if (m.schema_version !== 1 || !kinds.includes(m.kind as ArtifactKind)) fail(); scope(m.binding, expected, true); id(m.artifact_id); integer(m.version, 1); text(m.title, 1024); text(m.media_type, 128);
  if (m.source_relative_path !== null) path(m.source_relative_path);
  if (typeof m.payload_sha256 !== "string" || !/^[a-f0-9]{64}$/.test(m.payload_sha256)) fail(); integer(m.bytes, 0, 1048576); integer(m.created_ms);
  return m as unknown as ArtifactMetadata;
}
async function sha(content: string): Promise<string> {
  const hash = await globalThis.crypto.subtle.digest("SHA-256", bytes(content)); return Array.from(new Uint8Array(hash), n => n.toString(16).padStart(2, "0")).join("");
}
export async function listTaskArtifacts(handle: string, view: TaskViewObservation, after: ArtifactCursor | null = null, limit = 200): Promise<ArtifactList> {
  const args = request(handle, view); integer(limit, 1, 200); const capturedAfter = after === null ? null : cursor(after);
  const raw = object(await invoke("task_artifact_list", { ...args, after: capturedAfter, limit }), ["scope", "store_missing", "page"]);
  const fresh = scope(raw.scope, args.expectedBinding); if (typeof raw.store_missing !== "boolean") fail();
  const p = object(raw.page, ["versions", "next_cursor", "has_more"]); if (!Array.isArray(p.versions) || p.versions.length > limit || typeof p.has_more !== "boolean") fail();
  const versions = p.versions.map(m => metadata(m, fresh)); let previous = capturedAfter;
  for (const m of versions) { const key: ArtifactCursor = [m.artifact_id, m.version]; if (previous && compare(key, previous) <= 0) fail(); previous = key; }
  const next = p.next_cursor === null ? null : cursor(p.next_cursor);
  const last = versions.length ? [versions[versions.length - 1].artifact_id, versions[versions.length - 1].version] as ArtifactCursor : null;
  if ((last === null) !== (next === null) || (last && next && compare(last, next) !== 0) || (p.has_more && versions.length !== limit) || (raw.store_missing && (versions.length || next || p.has_more || capturedAfter))) fail();
  return { scope: fresh, store_missing: raw.store_missing, page: { versions, next_cursor: next, has_more: p.has_more } };
}
export async function captureTaskArtifact(handle: string, view: TaskViewObservation, input: { relativePath: string; kind: ArtifactKind; title: string }): Promise<ArtifactMetadata> {
  input = { ...input };
  const args = request(handle, view); path(input.relativePath); text(input.title, 1024); if (!kinds.includes(input.kind)) fail();
  const m = metadata(await invoke("task_artifact_capture", { ...args, relativePath: input.relativePath, kind: input.kind, title: input.title }), args.expectedBinding);
  scope(m.binding, args.expectedBinding); if (m.kind !== input.kind || m.title !== input.title || m.source_relative_path !== input.relativePath || m.artifact_id !== `path.${await sha(input.relativePath)}`) fail(); return m;
}
export async function readTaskArtifact(handle: string, view: TaskViewObservation, artifactId: string, version: number): Promise<ArtifactText | null> {
  const args = request(handle, view); id(artifactId); integer(version, 1);
  const value = await invoke("task_artifact_read", { ...args, artifactId, version }); if (value === null) return null;
  const r = object(value, ["scope", "metadata", "content"]); const fresh = scope(r.scope, args.expectedBinding); const m = metadata(r.metadata, fresh);
  if (m.artifact_id !== artifactId || m.version !== version || typeof r.content !== "string" || r.content.includes("\0") || new TextDecoder("utf-8", { fatal: true }).decode(bytes(r.content)) !== r.content || bytes(r.content).length !== m.bytes || bytes(r.content).length > 1048576 || await sha(r.content) !== m.payload_sha256) fail();
  return { scope: fresh, metadata: m, content: r.content };
}
export async function deleteTaskArtifact(handle: string, view: TaskViewObservation, artifactId: string, version: number): Promise<DeleteResult> {
  const args = request(handle, view); id(artifactId); integer(version, 1);
  const result = await invoke("task_artifact_delete", { ...args, artifactId, version }); if (result !== "deleted" && result !== "already_deleted" && result !== "not_found") fail(); return result;
}

export async function saveTaskArtifactText(handle: string, view: TaskViewObservation, latest: ArtifactMetadata, content: string): Promise<ArtifactMetadata> {
  const args = request(handle, view);
  id(latest.artifact_id); integer(latest.version, 1);
  if (latest.kind !== "document" && latest.kind !== "report") fail();
  if (!content || content.includes("\0") || bytes(content).length > 1048576) fail();
  const m = metadata(await invoke("task_artifact_save_text", { ...args, artifactId: latest.artifact_id, expectedLatest: latest.version, content }), args.expectedBinding);
  scope(m.binding, args.expectedBinding);
  if (m.artifact_id !== latest.artifact_id || m.kind !== latest.kind || m.title !== latest.title || (m.version !== latest.version && m.version !== latest.version + 1) || m.bytes !== bytes(content).length || m.payload_sha256 !== await sha(content)) fail();
  return m;
}
