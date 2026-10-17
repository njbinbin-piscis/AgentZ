import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { listNotifications, markNotificationRead } from "./notificationInbox";

beforeEach(() => invoke.mockReset());

describe("project notification inbox client", () => {
  it("validates a scoped page and marks an event read through the host", async () => {
    invoke.mockResolvedValueOnce({ notices: [{ event_key: "run:1", task_id: "task", kind: "completed", summary: "Task completed", created_ms: 10, read_ms: null }], unread_count: 1 });
    const page = await listNotifications("scope", 10);
    expect(page.notices[0].task_id).toBe("task");
    expect(invoke).toHaveBeenCalledWith("notification_inbox_list", { scopeHandle: "scope", limit: 10 });
    invoke.mockResolvedValueOnce(true);
    expect(await markNotificationRead("scope", "run:1")).toBe(true);
    expect(invoke).toHaveBeenLastCalledWith("notification_inbox_mark_read", { scopeHandle: "scope", eventKey: "run:1" });
  });
  it("rejects malformed pages and identities without accepting a fallback", async () => {
    invoke.mockResolvedValueOnce({ notices: [{ event_key: "run:1", task_id: "task", kind: "unknown", summary: "Task", created_ms: 1, read_ms: null }], unread_count: 1 });
    await expect(listNotifications("scope")).rejects.toThrow();
    await expect(markNotificationRead("scope", "\u0000bad")).rejects.toThrow();
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
