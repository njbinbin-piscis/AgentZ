import { beforeAll, describe, expect, it, vi } from "vitest";
import { loadScopedModelId, saveScopedModelId, subscribeScopedModelId } from "./modelPrefs";

beforeAll(() => {
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, v),
  });
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal(
    "CustomEvent",
    class extends Event {
      detail: unknown;
      constructor(type: string, init?: { detail?: unknown }) {
        super(type);
        this.detail = init?.detail;
      }
    },
  );
});

describe("scoped model sync", () => {
  it("notifies other surfaces when the model changes, once per change", () => {
    const seen: string[] = [];
    const off = subscribeScopedModelId("codez", (id) => seen.push(id));
    saveScopedModelId("codez", "a");
    saveScopedModelId("codez", "a");
    saveScopedModelId("workz", "w");
    saveScopedModelId("codez", "b");
    off();
    saveScopedModelId("codez", "c");
    expect(seen).toEqual(["a", "b"]);
    expect(loadScopedModelId("codez")).toBe("c");
  });
});
