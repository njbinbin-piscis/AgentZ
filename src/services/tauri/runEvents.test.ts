import { beforeEach, describe, expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import {
  readRunEvents,
  RunEventCollector,
  type RunEventStream,
  type DurableRunEvent,
  type RunEventPage,
} from "./runEvents";
const stream: RunEventStream = {
  task_id: "task",
  run_id: "run",
  attempt_id: "attempt",
  environment_id: "env",
};
const event = (sequence: number): DurableRunEvent => ({
  cursor: { stream, sequence },
  event_id: `event-${sequence}`,
  kind: "run.execution",
  payload: { execution: "running" },
  created_ms: sequence,
});
const page = (events: DurableRunEvent[], has_more = false): RunEventPage => ({
  stream,
  events,
  next_cursor: events[events.length - 1]?.cursor ?? null,
  has_more,
});
beforeEach(() => invoke.mockReset());
describe("scoped passive Run lifecycle replay", () => {
  it("forks an isolated verified prefix with a fresh page budget and atomic failed drafts", () => {
    const original = new RunEventCollector(stream, null, 1);
    original.append(page([event(1)]));
    const draft = original.fork();
    const external = draft.snapshot();
    external[0].payload = { execution: "corrupt" };
    expect(original.snapshot()[0].payload).toEqual({ execution: "running" });
    expect(draft.snapshot()[0].payload).toEqual({ execution: "running" });
    expect(() => draft.append(page([event(2), { ...event(3), event_id: "event-1" }]))).toThrow(
      "identity",
    );
    expect(draft.cursor()?.sequence).toBe(1);
    expect(original.cursor()?.sequence).toBe(1);
    draft.append(page([event(2)]));
    expect(draft.cursor()?.sequence).toBe(2);
    expect(original.cursor()?.sequence).toBe(1);
    expect(() => original.append(page([event(2)]))).toThrow("budget");
  });
  it("invokes only the fixed authorized query and bounds requested page size", async () => {
    invoke.mockResolvedValue(page([event(1)]));
    await readRunEvents("opaque", "task", "run", "attempt", null, 1);
    expect(invoke).toHaveBeenCalledWith("chat_run_events", {
      scopeHandle: "opaque",
      sessionId: "task",
      runId: "run",
      attemptId: "attempt",
      cursor: null,
      limit: 1,
    });
    invoke.mockClear();
    await expect(readRunEvents("opaque", "task", "run", "attempt", null, 201)).rejects.toThrow(
      "limit",
    );
    await expect(
      readRunEvents("opaque", "task", "run", "different", event(1).cursor),
    ).rejects.toThrow("requested");
    expect(invoke).not.toHaveBeenCalled();
  });
  it("rejects a host response from another attempt or environment without fallback", async () => {
    invoke.mockResolvedValue({ ...page([]), stream: { ...stream, attempt_id: "different" } });
    await expect(readRunEvents("opaque", "task", "run", "attempt")).rejects.toThrow("another");
    invoke.mockResolvedValue({ ...page([]), stream: { ...stream, environment_id: "new-env" } });
    await expect(
      readRunEvents("opaque", "task", "run", "attempt", event(1).cursor),
    ).rejects.toThrow("environment");
    expect(invoke).toHaveBeenCalledTimes(2);
  });
  it("deduplicates exact overlapping pages and preserves stable scoped cursors", () => {
    const collector = new RunEventCollector(stream);
    collector.append(page([event(1), event(2)], true));
    const result = collector.append(page([event(2), event(3)]));
    expect(result.map((record) => record.event_id)).toEqual(["event-1", "event-2", "event-3"]);
    expect(collector.cursor()).toEqual(event(3).cursor);
    result[0].event_id = "caller-mutated";
    expect(collector.snapshot()[0].event_id).toBe("event-1");
  });
  it("rejects gaps, order regressions and sequence/id conflicts atomically", () => {
    const collector = new RunEventCollector(stream);
    collector.append(page([event(1)]));
    for (const invalid of [
      page([event(3)]),
      page([event(2), event(1)]),
      page([{ ...event(1), event_id: "different" }]),
      page([{ ...event(2), event_id: "event-1" }]),
      page([event(2), { ...event(3), kind: "" }]),
      page([{ ...event(1), payload: { execution: "failed" } }]),
    ]) {
      expect(() => collector.append(invalid)).toThrow();
      expect(collector.snapshot()).toEqual([event(1)]);
      expect(collector.cursor()).toEqual(event(1).cursor);
    }
  });
  it("rejects cross-stream pages, cursors, oversized pages and invalid continuation", () => {
    const collector = new RunEventCollector(stream);
    expect(() =>
      collector.append({ ...page([]), stream: { ...stream, task_id: "other" } }),
    ).toThrow("Cross-stream");
    expect(() =>
      collector.append(
        page([
          { ...event(1), cursor: { stream: { ...stream, environment_id: "other" }, sequence: 1 } },
        ]),
      ),
    ).toThrow("Cross-stream");
    expect(() =>
      collector.append(page(Array.from({ length: 201 }, (_, i) => event(i + 1)))),
    ).toThrow("bounded");
    expect(() => collector.append({ ...page([event(1)]), next_cursor: event(2).cursor })).toThrow(
      "next",
    );
    expect(() => collector.append(page([], true))).toThrow("did not advance");
    expect(collector.snapshot()).toEqual([]);
  });
  it("bounds total page replay and accepts empty legacy history without inventing events", () => {
    const collector = new RunEventCollector(stream, null, 1);
    expect(collector.append(page([]))).toEqual([]);
    expect(collector.cursor()).toBeNull();
    expect(() => collector.append(page([]))).toThrow("budget");
    const resumed = new RunEventCollector(stream, event(4).cursor);
    expect(resumed.append(page([event(5)]))).toEqual([event(5)]);
    expect(() => resumed.append(page([event(5)], true))).toThrow("did not advance");
  });
});
