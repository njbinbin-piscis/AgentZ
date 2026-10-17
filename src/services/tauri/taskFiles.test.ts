import { beforeEach, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { listTaskFiles, readTaskFile } from "./taskFiles";
import type { TaskViewObservation } from "./taskView";
const view: TaskViewObservation = { schema_version: 1, session_id: "task", authority_project: "original", workspace_root: "checkout", observation_only: true, scope: { project_id: "original", task_id: "task", environment_id: "env", binding_version: "a".repeat(64) } };
beforeEach(() => invoke.mockReset());
it("requests existing task scope and relative paths without renderer workspace authority", async () => {
  invoke.mockResolvedValue({ scope: view.scope, workspace_root: "checkout", entries: [{ relative_path: "src/a.ts", is_dir: false, size: 3 }], truncated: false });
  expect((await listTaskFiles("opaque", view)).entries).toHaveLength(1);
  expect(invoke).toHaveBeenCalledWith("task_view_list", { scopeHandle: "opaque", sessionId: "task", expectedBinding: view.scope, relativePath: "" });
});
it("rejects absolute, parent, drive, scheme and malformed paths before host read", async () => {
  for (const path of ["", "../a", "a/../b", "/a", "C:/a", "a\\b", "file:a", "a//b", "a\u0000"]) await expect(readTaskFile("opaque", view, path)).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
it("rejects stale binding, root substitution and duplicate entries", async () => {
  for (const value of [ { scope: { ...view.scope, environment_id: "other" }, workspace_root: "checkout", entries: [], truncated: false }, { scope: view.scope, workspace_root: "other", entries: [], truncated: false }, { scope: view.scope, workspace_root: "checkout", entries: [{ relative_path: "a", is_dir: false, size: 0 }, { relative_path: "a", is_dir: false, size: 0 }], truncated: false } ]) {
    invoke.mockResolvedValue(value); await expect(listTaskFiles("opaque", view)).rejects.toThrow();
  }
});
it("accepts bounded UTF-8 text and rejects scope/path/size substitution", async () => {
  const valid = { scope: view.scope, relative_path: "src/a.ts", content: "中", bytes: 3 };
  invoke.mockResolvedValue(valid); expect(await readTaskFile("opaque", view, "src/a.ts")).toEqual(valid);
  for (const value of [{ ...valid, bytes: 1 }, { ...valid, relative_path: "other" }, { ...valid, bytes: 1048577 }]) {
    invoke.mockResolvedValue(value); await expect(readTaskFile("opaque", view, "src/a.ts")).rejects.toThrow();
  }
});
