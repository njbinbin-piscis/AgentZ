import { describe, expect, it, vi } from "vitest";
import type { WorkspaceSnapshot } from "./tauri/workspace";
import { preflightWorkspaceNavigation, type NavigationChecks } from "./workspaceNavigation";
function snapshot(): WorkspaceSnapshot {
  return { version: 1, project_dir: "/original", editor: { open_paths: ["a.txt"], active_path: "a.txt", dirty_buffers: {} },
    layout: { chat_open: true, chat_width: 300, browser_open: false, mode: "codez", sidebar_tab: "files", sidebar_collapsed: false, sidebar_width: 200, bottom_open: true, bottom_tab: "terminal", bottom_height: 200 } };
}
function setup() {
  const live = snapshot();
  const checks: NavigationChecks = { readSnapshot: () => live, isCurrent: () => true, confirmTerminals: vi.fn(async () => true), persist: vi.fn(async () => {}) };
  return { live, checks };
}
describe("workspace navigation non-destructive preflight", () => {
  it("dirty live buffer including empty content blocks without terminal or snapshot side effects", async () => {
    const { live, checks } = setup(); live.editor.dirty_buffers["a.txt"] = "";
    const before = structuredClone(live);
    expect(await preflightWorkspaceNavigation(checks)).toEqual({ status: "blocked", reason: "dirty_buffers" });
    expect(checks.confirmTerminals).not.toHaveBeenCalled(); expect(checks.persist).not.toHaveBeenCalled(); expect(live).toEqual(before);
  });
  it("cancellation retains original snapshot and does not save", async () => {
    const { live, checks } = setup(); checks.confirmTerminals = vi.fn(async () => false); const before = structuredClone(live);
    expect(await preflightWorkspaceNavigation(checks)).toEqual({ status: "blocked", reason: "cancelled" });
    expect(checks.persist).not.toHaveBeenCalled(); expect(live).toEqual(before);
  });
  it("strict save failure cannot return ready or modify live snapshot", async () => {
    const { live, checks } = setup(); checks.persist = vi.fn(async () => { throw new Error("disk full"); }); const before = structuredClone(live);
    expect(await preflightWorkspaceNavigation(checks)).toEqual({ status: "blocked", reason: "save_failed" }); expect(live).toEqual(before);
  });
  it("clean successful preflight passes only after confirmed and persisted and returns independent copy", async () => {
    const { live, checks } = setup(); const order: string[] = [];
    checks.confirmTerminals = async () => { order.push("confirm"); return true; };
    checks.persist = async value => { order.push("save"); value.editor.open_paths.push("mutated-copy"); };
    const result = await preflightWorkspaceNavigation(checks); expect(order).toEqual(["confirm", "save"]); expect(result.status).toBe("ready");
    if (result.status === "ready") result.snapshot.editor.open_paths.push("returned-copy"); expect(live.editor.open_paths).toEqual(["a.txt"]);
  });
  it("typing during confirmation rejects stale snapshot before save", async () => {
    const { live, checks } = setup(); checks.confirmTerminals = async () => { live.editor.dirty_buffers["a.txt"] = "late edit"; return true; };
    expect(await preflightWorkspaceNavigation(checks)).toEqual({ status: "blocked", reason: "stale" }); expect(checks.persist).not.toHaveBeenCalled(); expect(live.editor.dirty_buffers["a.txt"]).toBe("late edit");
  });
  it("route changes during persistence reject commit readiness", async () => {
    const { live, checks } = setup(); checks.persist = async () => { live.project_dir = "/new-route"; };
    expect(await preflightWorkspaceNavigation(checks)).toEqual({ status: "blocked", reason: "stale" });
  });
});
