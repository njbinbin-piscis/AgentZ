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
    const prev = localStorage.getItem(STORAGE_KEYS[scope]);
    localStorage.setItem(STORAGE_KEYS[scope], modelId);
    if (prev !== modelId && typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent(MODEL_CHANGE_EVENT, { detail: { scope, modelId } }));
    }
  } catch {
    /* ignore in headless test environments */
  }
}

export const MODEL_CHANGE_EVENT = "agentz-model-change";

/** Call `cb` whenever another surface (status bar, composer) changes the scope's model. */
export function subscribeScopedModelId(scope: ModelScope, cb: (modelId: string) => void): () => void {
  if (typeof window === "undefined") return () => {};
  const onChange = (e: Event) => {
    const d = (e as CustomEvent<{ scope: ModelScope; modelId: string }>).detail;
    if (d && d.scope === scope) cb(d.modelId);
  };
  window.addEventListener(MODEL_CHANGE_EVENT, onChange);
  return () => window.removeEventListener(MODEL_CHANGE_EVENT, onChange);
}
