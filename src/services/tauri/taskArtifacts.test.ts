import { beforeEach, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { captureTaskArtifact, deleteTaskArtifact, listTaskArtifacts, readTaskArtifact, saveTaskArtifactText, type ArtifactMetadata } from "./taskArtifacts";
import type { TaskViewObservation } from "./taskView";
const binding = { project_id: "project", task_id: "task", environment_id: "env", binding_version: "a".repeat(64) };
const view: TaskViewObservation = { schema_version: 1, scope: binding, session_id: "task", authority_project: "project", workspace_root: "workspace", observation_only: true };
// Independent fixed SHA256 fixture vectors; production uses the browser WebCrypto API.
const hash = (s: string) => {
  if(s==="out.txt")return "c07be25a843b0d0a2fea5a887e22c8bc381592ea5cddb120d4c979c6dc1d0f41";
  if(s==="你好")return "670d9743542cae3ea7ebe36af56bd53648b0a1126162e78d81a32934a711302e";
  throw new Error("Undefined fixture hash vector");
};
function meta(version = 1, content = "你好"): ArtifactMetadata { return { schema_version: 1, binding: { ...binding, binding_version: "b".repeat(64) }, artifact_id: `path.${hash("out.txt")}`, version, kind: "document", title: "Saved document", source_relative_path: "out.txt", media_type: "text/plain; charset=utf-8", payload_sha256: hash(content), bytes: new TextEncoder().encode(content).length, created_ms: 1 }; }
beforeEach(() => { invoke.mockReset(); });
it("reads old immutable content with historical binding but exact fresh reply scope and UTF8 digest", async () => {
  const metadata = meta(); invoke.mockResolvedValue({ scope: binding, metadata, content: "你好" });
  expect((await readTaskArtifact("opaque", view, metadata.artifact_id, 1))?.metadata.binding.binding_version).toBe("b".repeat(64));
  expect(invoke).toHaveBeenCalledWith("task_artifact_read", { scopeHandle: "opaque", sessionId: "task", expectedBinding: binding, artifactId: metadata.artifact_id, version: 1 });
});
it("rejects fresh scope mismatch, historical wrong environment, byte/hash corruption and unsafe version", async () => {
  const metadata = meta();
  for (const dto of [
    { scope: { ...binding, binding_version: "c".repeat(64) }, metadata, content: "你好" },
    { scope: binding, metadata: { ...metadata, binding: { ...metadata.binding, environment_id: "other" } }, content: "你好" },
    { scope: binding, metadata: { ...metadata, bytes: 2 }, content: "你好" },
    { scope: binding, metadata: { ...metadata, payload_sha256: "0".repeat(64) }, content: "你好" },
    { scope: binding, metadata: { ...metadata, version: Number.MAX_SAFE_INTEGER + 1 }, content: "你好" },
  ]) { invoke.mockResolvedValue(dto); await expect(readTaskArtifact("opaque", view, metadata.artifact_id, 1)).rejects.toThrow(); }
});
it("validates paging order, next cursor, budgets and missing store without fake versions", async () => {
  const a = meta(1); const b = meta(2); invoke.mockResolvedValue({ scope: binding, store_missing: false, page: { versions: [a, b], next_cursor: [b.artifact_id, 2], has_more: true } });
  expect((await listTaskArtifacts("opaque", view, null, 2)).page.has_more).toBe(true);
  for (const page of [ { versions: [b, a], next_cursor: [a.artifact_id, 1], has_more: false }, { versions: [a], next_cursor: [a.artifact_id, 2], has_more: false }, { versions: [], next_cursor: null, has_more: true } ]) { invoke.mockResolvedValue({ scope: binding, store_missing: false, page }); await expect(listTaskArtifacts("opaque", view)).rejects.toThrow(); }
  invoke.mockResolvedValue({ scope: binding, store_missing: true, page: { versions: [], next_cursor: null, has_more: false } }); expect((await listTaskArtifacts("opaque", view)).store_missing).toBe(true);
});
it("capture sends only host-owned snapshot inputs and checks captured identity", async () => {
  const m = meta(); m.binding = binding; invoke.mockResolvedValue(m);
  await captureTaskArtifact("opaque", view, { relativePath: "out.txt", kind: "document", title: "Saved document" });
  expect(invoke).toHaveBeenCalledWith("task_artifact_capture", { scopeHandle: "opaque", sessionId: "task", expectedBinding: binding, relativePath: "out.txt", kind: "document", title: "Saved document" });
  invoke.mockResolvedValue({ ...m, source_relative_path: "other.txt" }); await expect(captureTaskArtifact("opaque", view, { relativePath: "out.txt", kind: "document", title: "Saved document" })).rejects.toThrow();
});
it("accepts empty terminal page after a supplied cursor while rejecting fabricated continuation",async()=>{
  const a=meta();const after:[string,number]=[a.artifact_id,1];
  invoke.mockResolvedValue({scope:binding,store_missing:false,page:{versions:[],next_cursor:null,has_more:false}});
  expect((await listTaskArtifacts("opaque",view,after)).page).toEqual({versions:[],next_cursor:null,has_more:false});
  invoke.mockResolvedValue({scope:binding,store_missing:false,page:{versions:[],next_cursor:after,has_more:false}});
  await expect(listTaskArtifacts("opaque",view,after)).rejects.toThrow();
});
it("rejects invalid request paths ids limits before IPC", async () => {
  for (const p of ["../secret", "C:/secret", "ssh://host/a", "a\\b", "/abs", "a/..", "a/trailing."]) await expect(captureTaskArtifact("opaque", view, { relativePath: p, kind: "document", title: "Title" })).rejects.toThrow();
  await expect(readTaskArtifact("opaque", view, "../bad", 1)).rejects.toThrow(); await expect(listTaskArtifacts("opaque", view, null, 201)).rejects.toThrow(); await expect(deleteTaskArtifact("opaque", view, "valid", 0)).rejects.toThrow(); expect(invoke).not.toHaveBeenCalled();
});
it("keeps nullable absent read and three distinct deletion results, propagates host failure", async () => {
  invoke.mockResolvedValue(null); expect(await readTaskArtifact("opaque", view, "valid", 1)).toBeNull();
  for (const state of ["deleted", "already_deleted", "not_found"] as const) { invoke.mockResolvedValue(state); expect(await deleteTaskArtifact("opaque", view, "valid", 1)).toBe(state); }
  invoke.mockResolvedValue("success"); await expect(deleteTaskArtifact("opaque", view, "valid", 1)).rejects.toThrow();
  invoke.mockRejectedValue(new Error("host stale scope")); await expect(deleteTaskArtifact("opaque", view, "valid", 1)).rejects.toThrow("host stale scope");
});
it("saves a text revision only against the expected head and validates the returned digest", async () => {
  const latest = meta();
  const revised = { ...meta(2), binding, source_relative_path: null, payload_sha256: hash("你好") };
  invoke.mockResolvedValue(revised);
  expect((await saveTaskArtifactText("opaque", view, latest, "你好")).version).toBe(2);
  expect(invoke).toHaveBeenCalledWith("task_artifact_save_text", { scopeHandle: "opaque", sessionId: "task", expectedBinding: binding, artifactId: latest.artifact_id, expectedLatest: 1, content: "你好" });
  invoke.mockResolvedValue({ ...revised, version: 3 });
  await expect(saveTaskArtifactText("opaque", view, latest, "你好")).rejects.toThrow();
  invoke.mockReset();
  await expect(saveTaskArtifactText("opaque", view, { ...latest, kind: "webpage" }, "你好")).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
