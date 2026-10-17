import { expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { observeTaskView, validateTaskView } from "./taskView";
function dto() { return { schema_version: 1, scope: { project_id: "original", task_id: "task", environment_id: "environment", binding_version: "a".repeat(64) }, session_id: "task", authority_project: "original", workspace_root: "managed-worktree", observation_only: true }; }
it("preserves original task authority when workspace differs; sends no renderer path", async () => {
  invoke.mockResolvedValue(dto());
  expect(await observeTaskView("opaque", "task", "original")).toEqual(dto());
  expect(invoke).toHaveBeenCalledWith("task_view_resolve", { scopeHandle: "opaque", sessionId: "task" });
});
it("rejects identity substitution, unbounded paths and operational claims", () => {
  const original = dto();
  for (const value of [{ ...original, session_id: "other" }, { ...original, authority_project: "worktree" }, { ...original, observation_only: false }, { ...original, workspace_root: "x".repeat(65537) }, { ...original, scope: { ...original.scope, task_id: "other" } }, { ...original, scope: { ...original.scope, binding_version: "bad" } }, { ...original, capability: "pretend" }]) {
    expect(() => validateTaskView(value, "task", "original")).toThrow();
  }
});
it("rejects malformed request before invoking host", async () => {
  invoke.mockClear();
  await expect(observeTaskView("opaque", " task", "original")).rejects.toThrow();
  expect(invoke).not.toHaveBeenCalled();
});
