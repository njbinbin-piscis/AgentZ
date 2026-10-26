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

import { parentOf, parentRef } from "./activity";

describe("parent todos", () => {
  it("reads the Parent line and resolves by id prefix", () => {
    expect(parentRef("Do it\nParent: abcdef12")).toBe("abcdef12");
    expect(parentRef("Parent: ab")).toBeNull();
    const todos = [
      { id: "abcdef1234", description: "" },
      { id: "c1", description: "Parent: abcdef12" },
    ];
    expect(parentOf(todos[1], todos)?.id).toBe("abcdef1234");
    expect(parentOf(todos[0], todos)).toBeNull();
  });
});

import { mentionsMember, unreadFor } from "./activity";

describe("mailbox", () => {
  it("counts unread mentions only", () => {
    expect(mentionsMember("@!Coder please review", "Coder")).toBe(true);
    expect(mentionsMember("hi @Coders", "Coder")).toBe(false);
    const feed = [
      { id: 1, sender_id: "piscis", content: "@Coder old" },
      { id: 2, sender_id: "piscis", content: "@!Coder new" },
      { id: 3, sender_id: "coder", content: "@Coder self" },
    ];
    expect(unreadFor(feed, "coder", "Coder", 1)).toBe(1);
    expect(unreadFor(feed, "coder", "Coder", 2)).toBe(0);
  });
});
