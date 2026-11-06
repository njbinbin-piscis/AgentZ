/** Set before reload when a restored editor tab crashed the window. */
export const SKIP_EDITOR_RESTORE_KEY = "agentz.skipEditorRestore";

export function requestSkipEditorRestore(): void {
  try {
    sessionStorage.setItem(SKIP_EDITOR_RESTORE_KEY, "1");
  } catch {
    /* private mode */
  }
}

export function consumeSkipEditorRestore(): boolean {
  try {
    if (sessionStorage.getItem(SKIP_EDITOR_RESTORE_KEY) !== "1") return false;
    sessionStorage.removeItem(SKIP_EDITOR_RESTORE_KEY);
    return true;
  } catch {
    return false;
  }
}
