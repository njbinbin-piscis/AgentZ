import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { chatSend } from "./chat";

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({ ok: true, session_id: "task", response_text: "continued" });
});
describe("ordinary task continuation", () => {
  it("sends continuation by default without inferring replacement", async () => {
    const result = await chatSend({ prompt: "continue", sessionId: "task", projectDir: "fixture-project" });
    expect(result.session_id).toBe("task");
    expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ clearPlan: false, sessionId: "task", prompt: "continue" }));
  });
  it("preserves explicit continuation for a queued or automatic follow-up", async () => {
    await chatSend({ prompt: "next step", sessionId: "task", projectDir: "fixture-project", clearPlan: false, taskKey: "queued-run" });
    expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({ clearPlan: false, taskKey: "queued-run" }));
  });
  it("rejects legacy destructive clear before invoking the host", async () => {
    await expect(chatSend({ prompt: "a different input is not replacement authorization", projectDir: "fixture-project", clearPlan: true })).rejects.toThrow("clearPlan=true is deprecated");
    expect(invoke).not.toHaveBeenCalled();
  });
  it("passes an immutable artifact reference separately from user-visible text", async () => {
    const binding = { project_id: "fixture-project", task_id: "task", environment_id: "env", binding_version: "a".repeat(64) };
    const artifactRef = { artifact_id: "report", version: 2, payload_sha256: "b".repeat(64), expected_binding: binding };
    await chatSend({ prompt: "continue", displayPrompt: "continue", sessionId: "task", projectDir: "fixture-project", artifactRef, artifactScopeHandle: "host-scope" });
    expect(invoke).toHaveBeenCalledWith("chat_send", expect.objectContaining({
      prompt: "continue", displayPrompt: "continue", artifactRef, artifactScopeHandle: "host-scope",
    }));
  });
});
