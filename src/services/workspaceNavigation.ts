import type { WorkspaceSnapshot } from "./tauri/workspace";

export type NavigationPreflight =
  | { status: "ready"; snapshot: WorkspaceSnapshot }
  | { status: "blocked"; reason: "dirty_buffers" | "cancelled" | "snapshot_failed" | "save_failed" | "stale" };
export interface NavigationChecks {
  /** Must read live editor refs, not the debounced workspace persistence patch. */
  readSnapshot(): WorkspaceSnapshot;
  /** Captured route/navigation generation remains current. */
  isCurrent(): boolean;
  /** Confirmation only: must not destroy terminals or mutate the route. */
  confirmTerminals(): Promise<boolean>;
  /** Must reject on failure; a logging-and-swallowing save is not sufficient. */
  persist(snapshot: WorkspaceSnapshot): Promise<void>;
}
function capture(checks: NavigationChecks): WorkspaceSnapshot {
  const snapshot = structuredClone(checks.readSnapshot());
  if (!snapshot.editor || typeof snapshot.editor.dirty_buffers !== "object"
      || snapshot.editor.dirty_buffers === null || Array.isArray(snapshot.editor.dirty_buffers)) {
    throw new Error("Live editor snapshot required");
  }
  return snapshot;
}
/**
 * Non-destructive preflight only. No implicit discard or snapshot-as-dirty-backup.
 * Current workspace_save has one overwriteable global file; dirty navigation is
 * refused until actual editor save succeeds. Caller owns terminal cleanup/commit.
 */
export async function preflightWorkspaceNavigation(checks: NavigationChecks): Promise<NavigationPreflight> {
  if (!checks.isCurrent()) return { status: "blocked", reason: "stale" };
  let snapshot: WorkspaceSnapshot;
  try { snapshot = capture(checks); } catch { return { status: "blocked", reason: "snapshot_failed" }; }
  if (Object.keys(snapshot.editor.dirty_buffers).length) return { status: "blocked", reason: "dirty_buffers" };
  const encoded = JSON.stringify(snapshot);
  const unchanged = () => {
    try { return checks.isCurrent() && JSON.stringify(capture(checks)) === encoded; } catch { return false; }
  };
  let confirmed: boolean;
  try { confirmed = await checks.confirmTerminals(); } catch { return { status: "blocked", reason: "cancelled" }; }
  if (!unchanged()) return { status: "blocked", reason: "stale" };
  if (!confirmed) return { status: "blocked", reason: "cancelled" };
  try { await checks.persist(structuredClone(snapshot)); } catch { return { status: "blocked", reason: "save_failed" }; }
  if (!unchanged()) return { status: "blocked", reason: "stale" };
  return { status: "ready", snapshot };
}
