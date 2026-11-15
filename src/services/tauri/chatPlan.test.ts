import { beforeEach, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { applyTaskPlanIntent, cancelOpenPlanItems, queryTaskPlan } from "./chatPlan";
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
const planWith = (states: string[], status = "active") => ({
  current: {
    id: "p1",
    revision: 4,
    status,
    items: states.map((state, i) => ({ id: `i${i}`, description: "d", state })),
  },
});

it("cancelOpenPlanItems sends one cancel_open_items against the queried revision", async () => {
  invoke.mockResolvedValueOnce(planWith(["done", "in_progress", "pending"]));
  invoke.mockResolvedValueOnce({ id: "p1", revision: 5 });
  await cancelOpenPlanItems("opaque", "task");
  expect(invoke).toHaveBeenNthCalledWith(2, "chat_plan_apply", {
    scopeHandle: "opaque",
    sessionId: "task",
    expected: { id: "p1", revision: 4 },
    intent: { kind: "cancel_open_items" },
  });
});

it("cancelOpenPlanItems changes nothing when there is no plan, it is closed, or nothing is open", async () => {
  invoke.mockResolvedValueOnce({ current: null });
  expect(await cancelOpenPlanItems("o", "t")).toBeNull();
  invoke.mockResolvedValueOnce(planWith(["pending"], "cancelled"));
  expect(await cancelOpenPlanItems("o", "t")).toBeNull();
  invoke.mockResolvedValueOnce(planWith(["done", "cancelled"]));
  expect(await cancelOpenPlanItems("o", "t")).toBeNull();
  invoke.mockRejectedValueOnce(new Error("unsupported session"));
  expect(await cancelOpenPlanItems("o", "t")).toBeNull();
  expect(invoke).toHaveBeenCalledTimes(4);
});

it("cancelOpenPlanItems surfaces a refused change", async () => {
  invoke.mockResolvedValueOnce(planWith(["pending"]));
  invoke.mockRejectedValueOnce(new Error("task is running"));
  await expect(cancelOpenPlanItems("o", "t")).rejects.toThrow("running");
});

it("rejects missing revision or empty explicit goal before invoking", async () => {
  await expect(applyTaskPlanIntent("opaque", "task", { id: "p", revision: 0 }, { kind: "cancel" })).rejects.toThrow("revision");
  await expect(applyTaskPlanIntent("opaque", "task", { id: "p", revision: 2 }, { kind: "replace", objective: " " })).rejects.toThrow("objective");
  expect(invoke).not.toHaveBeenCalled();
});
