import { describe, expect, it } from "vitest";
import { projectRunAgentEvents } from "./runEventProjection";
import type { DurableRunEvent, RunEventStream } from "./runEvents";
const stream: RunEventStream = {
  task_id: "task",
  run_id: "run",
  attempt_id: "attempt",
  environment_id: "environment",
};
function events(payloads: unknown[]): DurableRunEvent[] {
  return payloads.map((payload, index) => ({
    cursor: { stream, sequence: index + 1 },
    event_id: `agent.${index}`,
    kind: "agent.event",
    payload,
    created_ms: index,
  }));
}
describe("passive Attempt display projection", () => {
  it("projects deltas and tool state without duplicate committed text or secret tool payloads", () => {
    const input = events([
      { type: "text_segment_start", iteration: 1 },
      { type: "text_delta", delta: "hello " },
      { type: "tool_start", id: "call", name: "file_read", input: { api_key: "TOP_SECRET" } },
      {
        type: "tool_end",
        id: "call",
        name: "file_read",
        result: "TOP_SECRET tool output",
        is_error: false,
      },
      { type: "text_delta", delta: "world" },
      { type: "message_commit", message: { text: "hello world TOP_SECRET" } },
      { type: "done", total_input_tokens: 1, total_output_tokens: 2 },
    ]);
    const before = JSON.stringify(input);
    const result = projectRunAgentEvents(stream, input);
    expect(result.text).toBe("hello world");
    expect(result.tools).toEqual([{ id: "call", name: "file_read", status: "completed" }]);
    expect(result.outcome).toBe("done");
    expect(JSON.stringify(result)).not.toContain("TOP_SECRET");
    expect(JSON.stringify(input)).toBe(before);
  });
  it("redacts familiar credentials across deltas and keeps error details out of display", () => {
    const result = projectRunAgentEvents(
      stream,
      events([
        { type: "text_delta", delta: "Bearer sec" },
        { type: "text_delta", delta: "ret api_key=private sk-abcdefghijklmnop" },
        {
          type: "error",
          message: "SECRET_ERROR",
          code: "model_error",
          details: { authorization: "SECRET_ERROR" },
        },
        { type: "done", total_input_tokens: 1, total_output_tokens: 2 },
      ]),
    );
    expect(result.text).toContain("[redacted]");
    expect(result.text).not.toMatch(/private|abcdefghijklmnop|secret/);
    expect(JSON.stringify(result)).not.toContain("SECRET_ERROR");
    expect(result.outcome).toBe("error");
    expect(result.errorCount).toBe(1);
  });
  it("ignores permissions and UI without creating actions and reflects failed tool/cancellation", () => {
    const result = projectRunAgentEvents(
      stream,
      events([
        {
          type: "permission_request",
          request_id: "request",
          tool_name: "shell",
          tool_input: { command: "delete secrets" },
          description: "danger",
        },
        { type: "interactive_ui", request_id: "ui", ui_definition: { action: "execute" } },
        { type: "tool_end", id: "call", name: "shell", result: "secret", is_error: true },
        { type: "cancelled" },
      ]),
    );
    expect(result.tools[0].status).toBe("failed");
    expect(result.outcome).toBe("cancelled");
    expect(result.ignoredCount).toBe(2);
    expect(JSON.stringify(result)).not.toMatch(/command|execute|danger|secret/);
  });
  it("rejects malformed, unknown fields/types, bad tool status and cross Attempt/order", () => {
    for (const payload of [
      { type: "future_event" },
      { type: "text_delta", delta: 1 },
      { type: "text_delta", delta: "ok", unknown: "secret" },
      { type: "tool_end", id: "i", name: "shell", result: "ok", is_error: "false" },
      { type: "text_segment_start", iteration: 0 },
      { type: "context_usage", estimated_input_tokens: "bad" },
    ]) {
      expect(() => projectRunAgentEvents(stream, events([payload]))).toThrow();
    }
    const scoped = events([{ type: "cancelled" }]);
    scoped[0].cursor.stream = { ...stream, attempt_id: "old" };
    expect(() => projectRunAgentEvents(stream, scoped)).toThrow();
    const gap = events([
      { type: "text_delta", delta: "a" },
      { type: "text_delta", delta: "b" },
    ]);
    gap[1].cursor.sequence = 3;
    expect(() => projectRunAgentEvents(stream, gap)).toThrow();
  });
  it("bounds display text/tool counts and input event/shape work", () => {
    const result = projectRunAgentEvents(
      stream,
      events([
        { type: "text_delta", delta: "a".repeat(70000) },
        ...Array.from({ length: 101 }, (_, i) => ({
          type: "tool_start",
          id: `call-${i}`,
          name: "shell",
          input: {},
        })),
      ]),
    );
    expect(result.text.length).toBe(65536);
    expect(result.tools).toHaveLength(100);
    expect(result.truncated).toBe(true);
    expect(() =>
      projectRunAgentEvents(
        stream,
        events(Array.from({ length: 5001 }, () => ({ type: "cancelled" }))),
      ),
    ).toThrow();
    expect(() =>
      projectRunAgentEvents(
        stream,
        events([{ type: "tool_start", id: "call", name: "shell", input: undefined }]),
      ),
    ).toThrow();
  });
  it("accepts actual optional serde shapes without projecting background or context data", () => {
    const result = projectRunAgentEvents(
      stream,
      events([
        {
          type: "fish_progress",
          fish_id: "fish",
          fish_name: "helper",
          iteration: 1,
          tool_name: null,
          status: "thinking",
          text_delta: "PRIVATE_BACKGROUND",
        },
        {
          type: "notice",
          message: "PRIVATE_NOTICE",
          code: null,
          details: { secret: "PRIVATE_NOTICE" },
        },
        {
          type: "context_usage",
          estimated_input_tokens: 1,
          total_input_budget: 2,
          trigger_threshold: 1,
          cumulative_input_tokens: 1,
          cumulative_output_tokens: 1,
          rolling_summary_version: 0,
          auto_compact_threshold: 0,
        },
        {
          type: "plan_update",
          items: [{ id: "step", content: "PRIVATE_PLAN", status: "cancelled" }],
        },
      ]),
    );
    expect(result.ignoredCount).toBe(4);
    expect(JSON.stringify(result)).not.toContain("PRIVATE_");
    expect(() =>
      projectRunAgentEvents(
        stream,
        events([{ type: "plan_update", items: [{ id: "step", content: 1, status: "pending" }] }]),
      ),
    ).toThrow();
  });
});
