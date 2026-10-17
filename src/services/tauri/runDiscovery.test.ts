import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { readCurrentTaskRun } from "./runDiscovery";
const run = {
  run_id: "run",
  task_id: "task",
  environment_id: "env",
  attempt_id: "first",
  execution: "queued",
  delivery: "not_ready",
  updated_ms: 1,
};
beforeEach(() => invoke.mockReset());
describe("passive current Run discovery", () => {
  it("queries by scope/task only, accepts no-history null and propagates unknown-task errors", async () => {
    invoke.mockResolvedValueOnce(null);
    expect(await readCurrentTaskRun("opaque", "task")).toBeNull();
    expect(invoke).toHaveBeenCalledWith("chat_current_run", {
      scopeHandle: "opaque",
      sessionId: "task",
    });
    invoke.mockRejectedValueOnce(new Error("session not found"));
    await expect(readCurrentTaskRun("opaque", "unknown")).rejects.toThrow("not found");
    expect(invoke).toHaveBeenCalledTimes(2);
  });
  it("uses the latest retry Attempt rather than a cached previous Run snapshot", async () => {
    invoke.mockResolvedValueOnce({ ...run, execution: "failed" });
    expect((await readCurrentTaskRun("opaque", "task"))?.attempt_id).toBe("first");
    invoke.mockResolvedValueOnce({ ...run, attempt_id: "retry", updated_ms: 2 });
    const retried = await readCurrentTaskRun("opaque", "task");
    expect(retried?.attempt_id).toBe("retry");
    expect(Object.isFrozen(retried)).toBe(true);
    expect(invoke).toHaveBeenCalledTimes(2);
  });
  it("accepts every Rust snake_case state including double-l cancelled", async () => {
    for (const execution of [
      "queued",
      "running",
      "cancelling",
      "succeeded",
      "failed",
      "cancelled",
      "interrupted",
    ]) {
      invoke.mockResolvedValueOnce({
        ...run,
        execution,
        delivery: execution === "succeeded" ? "awaiting_review" : "not_ready",
      });
      expect((await readCurrentTaskRun("opaque", "task"))?.execution).toBe(execution);
    }
    for (const delivery of ["accepted", "rejected"]) {
      invoke.mockResolvedValueOnce({ ...run, execution: "succeeded", delivery });
      expect((await readCurrentTaskRun("opaque", "task"))?.delivery).toBe(delivery);
    }
  });
  it("rejects other tasks, malformed identities and unsafe timestamps", async () => {
    for (const invalid of [
      { ...run, task_id: "other" },
      { ...run, attempt_id: "" },
      { ...run, environment_id: " env" },
      { ...run, run_id: "control\u0085" },
      { ...run, updated_ms: -1 },
      { ...run, updated_ms: 1.5 },
      { ...run, updated_ms: Number.MAX_SAFE_INTEGER + 1 },
    ]) {
      invoke.mockResolvedValueOnce(invalid);
      await expect(readCurrentTaskRun("opaque", "task")).rejects.toThrow();
    }
  });
  it("rejects unknown enums and inconsistent delivery without automatic fallback/resume", async () => {
    for (const invalid of [
      { ...run, execution: "canceled" },
      { ...run, execution: "resuming" },
      { ...run, delivery: "published" },
      { ...run, delivery: "accepted" },
      { ...run, execution: "succeeded" },
    ]) {
      invoke.mockResolvedValueOnce(invalid);
      await expect(readCurrentTaskRun("opaque", "task")).rejects.toThrow();
    }
    expect(invoke).toHaveBeenCalledTimes(5);
    expect(invoke.mock.calls.every(([command]) => command === "chat_current_run")).toBe(true);
  });
  it("rejects invalid request identities before invoking the host", async () => {
    await expect(readCurrentTaskRun("", "task")).rejects.toThrow("identity");
    await expect(readCurrentTaskRun("opaque", " task")).rejects.toThrow("identity");
    expect(invoke).not.toHaveBeenCalled();
  });
});
