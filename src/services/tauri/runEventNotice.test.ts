import { describe, expect, it } from "vitest";
import { committedHighWatermark, committedNoticeBytes } from "./runEventNotice";

const stream = { task_id: "task", run_id: "run", attempt_id: "attempt", environment_id: "env" };
const notice = (seq = 3) => ({
  sessionId: "task",
  channel: "agent_event_committed",
  payload: {
    cursor: { stream: { ...stream }, sequence: seq },
    event_id: `agent.event-${seq}`,
    kind: "agent.event",
    payload: { type: "text_delta", delta: "hello" },
    created_ms: 1,
  },
});

describe("committedHighWatermark", () => {
  it("accepts exactly one MiB of UTF-8 payload plus bounded envelope metadata", () => {
    const input = notice();
    const emptyBytes =
      new TextEncoder().encode(JSON.stringify(input.payload.payload)).byteLength - 5;
    const contentBytes = 1024 * 1024 - emptyBytes;
    input.payload.payload.delta =
      "汉".repeat(Math.floor(contentBytes / 3)) + "x".repeat(contentBytes % 3);
    expect(new TextEncoder().encode(JSON.stringify(input.payload.payload)).byteLength).toBe(
      1024 * 1024,
    );
    expect(committedNoticeBytes(input)).toBeGreaterThan(1024 * 1024);
    expect(committedHighWatermark(stream, input, null)).toBe(3);
    input.payload.payload.delta += "x";
    expect(() => committedNoticeBytes(input)).toThrow("payload exceeds budget");
  });

  it("validates initial schema and UTF-8 identity boundaries before caching", () => {
    const task = "汉".repeat(85) + "x";
    const input = notice();
    input.sessionId = task;
    input.payload.cursor.stream.task_id = task;
    expect(committedNoticeBytes(input)).toBeGreaterThan(0);
    input.sessionId += "x";
    input.payload.cursor.stream.task_id = input.sessionId;
    expect(() => committedNoticeBytes(input)).toThrow("identity");
    expect(() => committedNoticeBytes({ ...notice(), extra: true })).toThrow("schema");
    expect(() => committedNoticeBytes({ ...notice(), sessionId: "other" })).toThrow("Cross-task");
    expect(() => committedNoticeBytes({ ...notice(), channel: "agent_event" })).toThrow("channel");
  });
  it("coalesces duplicates and out of order notices without changing verified cursor", () => {
    const verified = { stream: { ...stream }, sequence: 2 };
    const input = notice();
    const before = JSON.stringify({ verified, input });
    expect(committedHighWatermark(stream, input, verified)).toBe(3);
    expect(committedHighWatermark(stream, input, verified, 8)).toBe(8);
    expect(committedHighWatermark(stream, notice(1), verified)).toBe(2);
    expect(JSON.stringify({ verified, input })).toBe(before);
  });
  it("ignores other sessions, channels and every old stream identity", () => {
    expect(committedHighWatermark(stream, { ...notice(), sessionId: null }, null)).toBeNull();
    expect(committedHighWatermark(stream, { ...notice(), sessionId: "other" }, null)).toBeNull();
    expect(
      committedHighWatermark(stream, { ...notice(), channel: "agent_event", payload: {} }, null),
    ).toBeNull();
    for (const field of Object.keys(stream) as (keyof typeof stream)[]) {
      const input = notice();
      input.payload.cursor.stream[field] = "old";
      expect(committedHighWatermark(stream, input, null)).toBeNull();
    }
  });
  it("fails closed for malformed matching notices", () => {
    const cases = [
      { ...notice(), extra: true },
      { ...notice(), payload: { ...notice().payload, kind: "run.created" } },
      { ...notice(), payload: { ...notice().payload, event_id: "agent." } },
      { ...notice(), payload: { ...notice().payload, created_ms: -1 } },
      { ...notice(), payload: { ...notice().payload, payload: undefined } },
      notice(0),
      notice(1.5),
      notice(Number.MAX_SAFE_INTEGER + 1),
    ];
    for (const input of cases) expect(() => committedHighWatermark(stream, input, null)).toThrow();
  });
  it("rejects cross stream cursors and unsafe existing high watermarks", () => {
    expect(() =>
      committedHighWatermark(stream, notice(), {
        stream: { ...stream, run_id: "old" },
        sequence: 1,
      }),
    ).toThrow();
    expect(() => committedHighWatermark(stream, notice(), null, NaN)).toThrow();
    expect(() => committedHighWatermark(stream, notice(), null, -1)).toThrow();
  });
  it("bounds opaque JSON and treats future sequence as replay hint only", () => {
    expect(committedHighWatermark(stream, notice(100), null)).toBe(100);
    const input = notice();
    input.payload.payload.delta = "x".repeat(1048577);
    expect(() => committedHighWatermark(stream, input, null)).toThrow();
    let nested: unknown = null;
    for (let i = 0; i < 18; i++) nested = { nested };
    expect(() =>
      committedHighWatermark(
        stream,
        { ...notice(), payload: { ...notice().payload, payload: nested } },
        null,
      ),
    ).toThrow();
  });
});
