import { describe, expect, it } from "vitest";
import { activityForTool, activityFromEvent, deskGrid, koiIdFromSession, koiTaskSessionId } from "./activity";

describe("botz activity", () => {
  it("maps tools to desk activities", () => {
    expect(activityForTool("file_edit")).toBe("typing");
    expect(activityForTool("file_write")).toBe("typing");
    expect(activityForTool("file_read")).toBe("reading");
    expect(activityForTool("grep")).toBe("reading");
    expect(activityForTool("code_run")).toBe("running");
    expect(activityForTool("pool_chat")).toBe("thinking");
  });

  it("follows the event stream", () => {
    expect(activityFromEvent({ type: "tool_start", id: "1", name: "shell", input: {} }, "idle")).toBe("running");
    expect(activityFromEvent({ type: "tool_end", id: "1", name: "shell", result: "", is_error: true }, "running")).toBe("error");
    expect(
      activityFromEvent({ type: "permission_request", request_id: "r", tool_name: "shell", tool_input: {}, description: "" }, "running"),
    ).toBe("waiting");
    expect(activityFromEvent({ type: "text_delta", delta: "x" }, "thinking")).toBeNull();
    expect(activityFromEvent({ type: "done", total_input_tokens: 1, total_output_tokens: 1 }, "typing")).toBe("idle");
  });

  it("resolves member from koi task session ids", () => {
    const sid = koiTaskSessionId("dev_lead", "abcdef1234567");
    expect(sid).toBe("koi_task_dev_lead_abcdef12");
    expect(koiIdFromSession(sid, ["dev", "dev_lead"])).toBe("dev_lead");
    expect(koiIdFromSession("koi_task_qa_12345678", ["dev"])).toBeNull();
    expect(koiIdFromSession("other", ["dev"])).toBeNull();
  });

  it("lays out desks in a compact grid", () => {
    expect(deskGrid(0)).toEqual({ cols: 1, rows: 0 });
    expect(deskGrid(3)).toEqual({ cols: 2, rows: 2 });
    expect(deskGrid(9)).toEqual({ cols: 3, rows: 3 });
    expect(deskGrid(20)).toEqual({ cols: 4, rows: 5 });
  });
});
