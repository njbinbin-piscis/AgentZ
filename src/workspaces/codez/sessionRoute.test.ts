import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { OPEN_CODEZ_SESSION_EVENT, requestOpenCodezSession, sameProjectPath } from "./sessionRoute";
import { isReadOnlyInCodeZ, SESSION_SOURCES_PROJECT_SHARED } from "../../services/tauri/chat";

describe("sameProjectPath", () => {
  it("ignores separator style and trailing slashes", () => {
    expect(sameProjectPath("C:\\Projects\\AgentZ\\", "C:/Projects/AgentZ")).toBe(true);
  });

  it("does not match different projects or missing paths", () => {
    expect(sameProjectPath("C:/Projects/A", "C:/Projects/AB")).toBe(false);
    expect(sameProjectPath(null, "C:/Projects/A")).toBe(false);
    expect(sameProjectPath("", "")).toBe(false);
  });
});

describe("requestOpenCodezSession", () => {
  // The suite runs in node (no DOM); an EventTarget stands in for window.
  beforeEach(() => {
    vi.stubGlobal("window", new EventTarget());
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("dispatches the open event with the session detail", () => {
    const listener = vi.fn();
    window.addEventListener(OPEN_CODEZ_SESSION_EVENT, listener);
    requestOpenCodezSession({ projectDir: "C:/p", sessionId: "s1", source: "workz" });
    expect(listener).toHaveBeenCalledTimes(1);
    const event = listener.mock.calls[0][0] as CustomEvent;
    expect(event.detail).toEqual({ projectDir: "C:/p", sessionId: "s1", source: "workz" });
  });
});

describe("shared project sessions", () => {
  it("list CodeZ, WorkZ and team sessions for one project", () => {
    expect(SESSION_SOURCES_PROJECT_SHARED).toEqual(["codez", "workz", "workz-team"]);
  });

  it("only team sessions are read-only in CodeZ", () => {
    expect(isReadOnlyInCodeZ("workz-team")).toBe(true);
    expect(isReadOnlyInCodeZ("workz")).toBe(false);
    expect(isReadOnlyInCodeZ("codez")).toBe(false);
    expect(isReadOnlyInCodeZ(undefined)).toBe(false);
  });
});
