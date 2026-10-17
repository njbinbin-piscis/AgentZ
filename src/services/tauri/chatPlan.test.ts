import { beforeEach, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { applyTaskPlanIntent, queryTaskPlan } from "./chatPlan";
beforeEach(() => { invoke.mockReset(); });

it("queries by opaque scope and existing session without renderer paths", async () => {
  invoke.mockResolvedValue({ current: null });
  await queryTaskPlan("opaque", "task");
  expect(invoke).toHaveBeenCalledWith("chat_plan_query", { scopeHandle: "opaque", sessionId: "task" });
});
it("propagates stale CAS rejection without resubmitting against a newer plan", async () => {
  invoke.mockRejectedValue(new Error("stale explicit plan identity/revision"));
  await expect(applyTaskPlanIntent("opaque", "task", { id: "old", revision: 3 }, { kind: "cancel" })).rejects.toThrow("stale");
  expect(invoke).toHaveBeenCalledTimes(1);
});
it("rejects missing revision or empty explicit goal before invoking", async () => {
  await expect(applyTaskPlanIntent("opaque", "task", { id: "p", revision: 0 }, { kind: "cancel" })).rejects.toThrow("revision");
  await expect(applyTaskPlanIntent("opaque", "task", { id: "p", revision: 2 }, { kind: "replace", objective: " " })).rejects.toThrow("objective");
  expect(invoke).not.toHaveBeenCalled();
});
