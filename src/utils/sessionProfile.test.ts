import { beforeEach, describe, expect, it } from "vitest";
import { loadSessionProfile, removeSessionProfile, saveSessionProfile } from "./sessionProfile";

const store = new Map<string, string>();
globalThis.localStorage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
  clear: () => store.clear(),
  key: () => null,
  length: 0,
} as Storage;

describe("sessionProfile", () => {
  beforeEach(() => localStorage.clear());

  it("keeps each session's selection separate", () => {
    saveSessionProfile("a", { modelId: "mimo", agentId: "", enabledSkills: [] });
    saveSessionProfile("b", { modelId: "mimopro", agentId: "reviewer", enabledSkills: ["tdd"] });
    expect(loadSessionProfile("a")?.modelId).toBe("mimo");
    expect(loadSessionProfile("b")).toEqual({ modelId: "mimopro", agentId: "reviewer", enabledSkills: ["tdd"] });
  });

  it("returns null for unknown or removed sessions", () => {
    expect(loadSessionProfile("x")).toBeNull();
    saveSessionProfile("x", { modelId: "m", agentId: "", enabledSkills: [] });
    removeSessionProfile("x");
    expect(loadSessionProfile("x")).toBeNull();
  });

  it("tolerates malformed stored values", () => {
    localStorage.setItem("agentz-session-profile:bad", "{not json");
    expect(loadSessionProfile("bad")).toBeNull();
  });
});
