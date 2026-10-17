import { isRecoveryPath } from "./editorRecovery";

/** Why Ctrl+S cannot write the active tab, or null when a save is allowed. */
export type SaveBlockReason = "diff" | "recovery" | "readOnly";

export function saveBlockReason(
  path: string,
  tab: { isReadOnly?: boolean; isDiff?: boolean } | undefined,
): SaveBlockReason | null {
  if (path.startsWith("diff:") || tab?.isDiff) return "diff";
  if (isRecoveryPath(path)) return "recovery";
  if (tab?.isReadOnly) return "readOnly";
  return null;
}
