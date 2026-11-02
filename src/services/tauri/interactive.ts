import { invoke } from "@tauri-apps/api/core";

/** Send the user's structured input back to a blocking `chat_ui` tool. */
export function respondInteractiveUi(
  requestId: string,
  values: Record<string, unknown>,
): Promise<void> {
  return invoke<void>("respond_interactive_ui", { requestId, values });
}

/** Request ids that a blocking `chat_ui` tool is still waiting on. */
export function pendingInteractiveIds(): Promise<string[]> {
  return invoke<string[]>("pending_interactive_ids");
}

/** Resolve a pending shell/file-write confirmation from the Agent harness. */
export function respondPermissionRequest(
  requestId: string,
  approved: boolean,
  expectedDigest?: string,
  alwaysAllow?: boolean,
): Promise<void> {
  return invoke<void>("respond_permission_request", {
    requestId,
    approved,
    expectedDigest,
    ...(alwaysAllow ? { alwaysAllow: true } : {}),
  });
}

export function getPermissionPreview(requestId: string): Promise<import("../../components/chat/PermissionCard").PermissionRequestCard> {
  return invoke("get_permission_preview", { requestId });
}
