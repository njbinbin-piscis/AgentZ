import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import {
  chatSend,
  consumeUserRecentEdits,
  formatUserRecentEditsBlock,
  peekUserRecentEdits,
  recordUserRecentEdit,
} from "./chat";

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({ ok: true, session_id: "task", response_text: "continued" });
  consumeUserRecentEdits("fixture-project");
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
  it("records human file saves and injects userRecentEdits into the next chatSend call", async () => {
    const edit = recordUserRecentEdit({
      projectDir: "fixture-project",
      path: "src/config.ts",
      beforeContent: "export const PORT = 3000;\n",
      afterContent: "export const PORT = 8080;\n",
      timestamp: "2026-10-19T12:00:00.000Z",
    });
    expect(edit).not.toBeNull();
    expect(peekUserRecentEdits("fixture-project")).toHaveLength(1);
    const block = formatUserRecentEditsBlock(peekUserRecentEdits("fixture-project"));
    expect(block).toContain("<user_recent_edits>");
    expect(block).toContain('path="src/config.ts"');
    expect(block).toContain("export const PORT = 8080;");

    await chatSend({
      prompt: "start the server",
      sessionId: "task",
      projectDir: "fixture-project",
    });
    expect(invoke).toHaveBeenCalledWith(
      "chat_send",
      expect.objectContaining({
        userRecentEdits: [
          expect.objectContaining({
            path: "src/config.ts",
            timestamp: "2026-10-19T12:00:00.000Z",
          }),
        ],
      }),
    );
    expect(peekUserRecentEdits("fixture-project")).toHaveLength(0);
  });
});
