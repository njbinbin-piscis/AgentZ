import { describe, expect, it } from "vitest";
import { applyToolEnd, applyToolStart, cancelRunningTools, finalizeTools } from "./agentTools";
import type { AgentToolEvent } from "./agentArtifacts";

const start = (id: string) =>
  ({ type: "tool_start", id, name: "file_read", input: { path: `docs/${id}.md` } }) as const;
const end = (id: string, result: string, is_error: boolean) =>
  ({ type: "tool_end", id, name: "file_read", result, is_error }) as const;

describe("applyToolEnd", () => {
  it("settles a normal call as done or error", () => {
    let tools: AgentToolEvent[] = applyToolStart(applyToolStart([], start("a")), start("b"));
    tools = applyToolEnd(tools, end("a", "contents", false));
    tools = applyToolEnd(tools, end("b", "no such file", true));
    expect(tools.map((t) => t.status)).toEqual(["done", "error"]);
  });

  it("shows a call cut short by Stop as cancelled, not failed", () => {
    let tools: AgentToolEvent[] = applyToolStart([], start("a"));
    tools = applyToolEnd(tools, end("a", "[trace_id:1] 已被用户取消", true));
    expect(tools).toHaveLength(1);
    expect(tools[0].status).toBe("cancelled");
    expect(tools[0].skipped).toBeUndefined();
  });

  it("marks a call that was never dispatched as skipped", () => {
    const tools = applyToolEnd(
      [],
      end("a", "Tool execution was cancelled or interrupted before completion.", true),
    );
    expect(tools).toHaveLength(1);
    expect(tools[0]).toMatchObject({ status: "cancelled", skipped: true });
  });
});

describe("end-of-turn cleanup", () => {
  it("finalizeTools closes calls left running by a finished turn", () => {
    const tools = finalizeTools(applyToolStart([], start("a")));
    expect(tools[0].status).toBe("done");
  });

  it("cancelRunningTools does not claim success for calls of an unconfirmed stop", () => {
    let tools: AgentToolEvent[] = applyToolStart(applyToolStart([], start("a")), start("b"));
    tools = applyToolEnd(tools, end("a", "contents", false));
    expect(cancelRunningTools(tools).map((t) => t.status)).toEqual(["done", "cancelled"]);
  });
});
