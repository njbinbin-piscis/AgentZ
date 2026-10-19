/** Per-workspace model picker persistence (CodeZ vs WorkZ are independent). */

export type ModelScope = "codez" | "workz";

const STORAGE_KEYS: Record<ModelScope, string> = {
  codez: "agentz-codez-model-id",
  workz: "agentz-workz-model-id",
};

const LEGACY_KEY = "agentz-model-id";

export function loadScopedModelId(scope: ModelScope): string {
  try {
    if (typeof localStorage === "undefined") return "";
    const saved = localStorage.getItem(STORAGE_KEYS[scope]);
    if (saved != null && saved !== "") return saved;
    return localStorage.getItem(LEGACY_KEY) ?? "";
  } catch {
    return "";
  }
}

export function saveScopedModelId(scope: ModelScope, modelId: string): void {
  try {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(STORAGE_KEYS[scope], modelId);
  } catch {
    /* ignore in headless test environments */
  }
}
