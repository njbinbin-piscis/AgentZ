import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { disableReminder, pollReminders, reminderStatus, setReminder } from "./automationReminder";

beforeEach(() => invoke.mockReset());

describe("scoped task reminder client", () => {
  it("uses the verified scope for status, schedule, disable and poll", async () => {
    invoke.mockResolvedValueOnce(null);
    expect(await reminderStatus("scope", "task")).toBeNull();
    invoke.mockResolvedValueOnce({ interval_minutes: 60, next_due_ms: 1000 });
    expect((await setReminder("scope", "task", 60)).interval_minutes).toBe(60);
    invoke.mockResolvedValueOnce(undefined);
    await disableReminder("scope", "task");
    invoke.mockResolvedValueOnce(1);
    expect(await pollReminders("scope")).toBe(1);
    expect(invoke.mock.calls.map(([name]) => name)).toEqual([
      "automation_reminder_status", "automation_reminder_set", "automation_reminder_disable", "automation_reminder_poll",
    ]);
  });
  it("rejects invalid identities, intervals and malformed host output", async () => {
    await expect(setReminder("scope", "task", 1)).rejects.toThrow();
    await expect(reminderStatus("scope", "\u0000task")).rejects.toThrow();
    invoke.mockResolvedValueOnce({ interval_minutes: 1, next_due_ms: 0 });
    await expect(reminderStatus("scope", "task")).rejects.toThrow();
    invoke.mockResolvedValueOnce(26);
    await expect(pollReminders("scope")).rejects.toThrow();
    expect(invoke).toHaveBeenCalledTimes(2);
  });
});
