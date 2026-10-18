import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FOCUS_REFRESH_GAP_MS } from "../../utils/coalesce";
const { discovery, readPage, scopes, language, subscribe, notices, dispose } = vi.hoisted(() => ({
  discovery: vi.fn(),
  subscribe: vi.fn(),
  notices: { handler: null as ((event: unknown) => void) | null },
  dispose: vi.fn(),
  readPage: vi.fn(),
  scopes: new Map<string, { handle: string; project_dir: string }>(),
  language: { value: "en" },
}));
vi.mock("../../services/tauri/projectScope", () => ({
  useProjectScope: (project: string | null) => (project ? (scopes.get(project) ?? null) : null),
}));
vi.mock("../../services/tauri/runDiscovery", () => ({ readCurrentTaskRun: discovery }));
vi.mock("../../services/tauri/runEvents", async () => ({
  ...(await vi.importActual<typeof import("../../services/tauri/runEvents")>(
    "../../services/tauri/runEvents",
  )),
  readRunEvents: readPage,
}));
vi.mock("../../services/tauri/chat", () => ({ onChatEvent: subscribe }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ i18n: { resolvedLanguage: language.value } }),
}));
import RunHistoryPanel from "./RunHistoryPanel";
import type { RunEventCursor } from "../../services/tauri/runEvents";
let renderer: ReactTestRenderer | undefined;
const props = { projectDir: "project", sessionId: "task", busy: false };
const current = (task = "task") => ({
  run_id: `private-run-${task}`,
  task_id: task,
  environment_id: "private-env",
  attempt_id: "private-attempt",
  execution: "running",
  delivery: "not_ready",
  updated_ms: 1,
});
const stream = (task = "task") => ({
  task_id: task,
  run_id: `private-run-${task}`,
  attempt_id: "private-attempt",
  environment_id: "private-env",
});
const event = (sequence: number, task = "task") => ({
  cursor: { stream: stream(task), sequence },
  event_id: `private-event-${sequence}`,
  kind: "run.execution",
  payload: { execution: "running", secret: "do-not-display" },
  created_ms: sequence,
});
const page = (sequence: number, has_more = false, task = "task") => ({
  stream: stream(task),
  events: [event(sequence, task)],
  next_cursor: event(sequence, task).cursor,
  has_more,
});
const text = () => JSON.stringify(renderer?.toJSON());
async function mount() {
  await act(async () => {
    renderer = create(<RunHistoryPanel {...props} />);
  });
}
beforeEach(() => {
  discovery.mockReset();
  subscribe.mockReset();
  dispose.mockReset();
  notices.handler = null;
  subscribe.mockImplementation(async (handler) => {
    notices.handler = handler;
    return dispose;
  });
  readPage.mockReset();
  scopes.clear();
  language.value = "en";
  scopes.set("project", { handle: "opaque", project_dir: "project" });
  scopes.set("other-project", { handle: "other-scope", project_dir: "other-project" });
  discovery.mockImplementation(async (_scope, task) => current(task));
  readPage.mockImplementation(async (_scope, task) => page(1, false, task));
  vi.stubGlobal("window", new EventTarget());
  vi.stubGlobal("document", Object.assign(new EventTarget(), { visibilityState: "visible" }));
});
afterEach(() => {
  act(() => {
    renderer?.unmount();
  });
  renderer = undefined;
  vi.unstubAllGlobals();
});
describe("passive scoped execution history", () => {
  it("enforces the byte budget across the verified prefix and an incremental batch", async () => {
    const chunk = (sequence: number) => ({
      ...event(sequence),
      payload: { execution: "running", secret: "x".repeat(900000) },
    });
    const first = [1, 2, 3, 4, 5].map(chunk);
    readPage.mockResolvedValueOnce({
      stream: stream(),
      events: first,
      next_cursor: first[4].cursor,
      has_more: false,
    });
    await mount();
    const next = [6, 7, 8, 9, 10].map(chunk);
    readPage.mockResolvedValueOnce({
      stream: stream(),
      events: next,
      next_cursor: next[4].cursor,
      has_more: false,
    });
    await act(async () => {
      renderer!.root.findByType("button").props.onClick();
    });
    expect(readPage.mock.calls[1][4]).toEqual(event(5).cursor);
    expect(text()).toContain("Execution history exceeds the display budget");
    expect(renderer!.root.findAllByType("ol")).toHaveLength(0);
  });
  it("rolls back a failed multi-page draft and retries from the last verified cursor", async () => {
    await mount();
    readPage.mockResolvedValueOnce(page(2, true)).mockResolvedValueOnce(page(4));
    await act(async () => {
      renderer!.root.findByType("button").props.onClick();
    });
    expect(text()).toContain("sequence gap");
    expect(renderer!.root.findAllByType("ol")).toHaveLength(0);
    expect(readPage.mock.calls[1][4]).toEqual(event(1).cursor);
    expect(readPage.mock.calls[2][4]).toEqual(event(2).cursor);
    readPage.mockResolvedValueOnce(page(2));
    await act(async () => {
      renderer!.root.findByType("button").props.onClick();
    });
    expect(readPage.mock.calls[3][4]).toEqual(event(1).cursor);
    expect(renderer!.root.findAllByProps({ role: "alert" })).toHaveLength(0);
    expect(renderer!.root.findByType("ol").findAllByType("li")).toHaveLength(2);
  });

  it("rediscovers an explicit refresh and starts a changed attempt without its old prefix", async () => {
    await mount();
    const retry = { ...current(), attempt_id: "new-attempt" };
    const retryStream = { ...stream(), attempt_id: "new-attempt" };
    const first = { ...event(1), cursor: { stream: retryStream, sequence: 1 }, kind: "run.retry" };
    discovery.mockResolvedValueOnce(retry);
    readPage.mockResolvedValueOnce({
      stream: retryStream,
      events: [first],
      next_cursor: first.cursor,
      has_more: false,
    });
    await act(async () => {
      renderer!.root.findByType("button").props.onClick();
    });
    expect(discovery).toHaveBeenCalledTimes(2);
    expect(readPage.mock.calls[1][3]).toBe("new-attempt");
    expect(readPage.mock.calls[1][4]).toBeNull();
    expect(text()).toContain("Retry queued");
    expect(renderer!.root.findByType("ol").findAllByType("li")).toHaveLength(1);
  });
  it("accepts an initial one MiB payload notice without projecting its raw content", async () => {
    const payload = { type: "text_delta", delta: "" };
    const overhead = new TextEncoder().encode(JSON.stringify(payload)).byteLength;
    const remaining = 1024 * 1024 - overhead;
    payload.delta = "汉".repeat(Math.floor(remaining / 3)) + "x".repeat(remaining % 3);
    subscribe.mockImplementationOnce(async (handler) => {
      notices.handler = handler;
      handler({
        sessionId: "task",
        channel: "agent_event_committed",
        payload: { ...event(1), event_id: "agent.initial", kind: "agent.event", payload },
      });
      return dispose;
    });
    await mount();
    expect(discovery).toHaveBeenCalledTimes(1);
    expect(renderer!.root.findAllByProps({ role: "alert" })).toHaveLength(0);
    expect(text()).not.toContain("汉");
  });

  it("keeps malformed initial notices as visible failures without starting discovery", async () => {
    subscribe.mockImplementationOnce(async (handler) => {
      notices.handler = handler;
      handler({
        sessionId: "task",
        channel: "agent_event_committed",
        payload: { ...event(1), event_id: "agent.initial", kind: "agent.event", extra: true },
      });
      return dispose;
    });
    await mount();
    expect(discovery).not.toHaveBeenCalled();
    expect(text()).toContain("Invalid committed event schema");
    expect(renderer!.root.findByProps({ role: "alert" })).toBeTruthy();
  });
  it("keeps one replay in flight and follows a notice arriving after its snapshot", async () => {
    vi.useFakeTimers();
    try {
      let finish!: (value: ReturnType<typeof page>) => void;
      readPage.mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            finish = resolve;
          }),
      );
      await mount();
      await act(async () => {
        notices.handler!({
          sessionId: "task",
          channel: "agent_event_committed",
          payload: {
            ...event(2),
            event_id: "agent.second",
            kind: "agent.event",
            payload: { type: "text_delta", delta: "notification-only" },
          },
        });
        await vi.advanceTimersByTimeAsync(400);
      });
      expect(discovery).toHaveBeenCalledTimes(1);
      readPage.mockResolvedValue({
        stream: stream(),
        events: [event(1), event(2)],
        next_cursor: event(2).cursor,
        has_more: false,
      });
      await act(async () => {
        finish(page(1));
      });
      expect(renderer!.root.findAllByProps({ role: "alert" })).toHaveLength(0);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(200);
      });
      expect(discovery).toHaveBeenCalledTimes(1);
      expect(readPage).toHaveBeenCalledTimes(2);
      expect(readPage.mock.calls[1][4]).toEqual(event(1).cursor);
      expect(text()).not.toContain("notification-only");
    } finally {
      vi.useRealTimers();
    }
  });
  it("registers before discovery and disposes a listener that resolves after unmount", async () => {
    let registered!: (dispose: () => void) => void;
    subscribe.mockImplementationOnce((handler) => {
      notices.handler = handler;
      return new Promise((resolve) => {
        registered = resolve;
      });
    });
    await mount();
    expect(discovery).not.toHaveBeenCalled();
    act(() => {
      renderer!.unmount();
    });
    renderer = undefined;
    await act(async () => {
      registered(dispose);
    });
    expect(dispose).toHaveBeenCalledTimes(1);
    expect(discovery).not.toHaveBeenCalled();
  });

  it("coalesces committed notices and ignores duplicates and old attempts", async () => {
    vi.useFakeTimers();
    try {
      await mount();
      const notice = (sequence: number, attempt = "private-attempt") => ({
        sessionId: "task",
        channel: "agent_event_committed",
        payload: {
          ...event(sequence),
          event_id: `agent.notice-${sequence}`,
          kind: "agent.event",
          cursor: { stream: { ...stream(), attempt_id: attempt }, sequence },
          payload: { type: "text_delta", delta: "notice-must-not-render" },
        },
      });
      readPage.mockResolvedValue({
        stream: stream(),
        events: [event(1), event(2)],
        next_cursor: event(2).cursor,
        has_more: false,
      });
      await act(async () => {
        notices.handler!(notice(20, "old-attempt"));
        notices.handler!(notice(2));
        notices.handler!(notice(2));
        await vi.advanceTimersByTimeAsync(199);
      });
      expect(discovery).toHaveBeenCalledTimes(1);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1);
      });
      expect(discovery).toHaveBeenCalledTimes(1);
      expect(readPage).toHaveBeenCalledTimes(2);
      expect(readPage.mock.calls[1][4]).toEqual(event(1).cursor);
      expect(text()).not.toContain("notice-must-not-render");
      await act(async () => {
        notices.handler!(notice(2));
        await vi.advanceTimersByTimeAsync(400);
      });
      expect(discovery).toHaveBeenCalledTimes(1);
      expect(readPage).toHaveBeenCalledTimes(2);
      expect(readPage.mock.calls[1][4]).toEqual(event(1).cursor);
    } finally {
      vi.useRealTimers();
    }
  });

  it("discards a delayed page and old listener after a project switch", async () => {
    let finishPage!: (value: ReturnType<typeof page>) => void;
    readPage.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishPage = resolve;
        }),
    );
    await mount();
    const oldListener = notices.handler!;
    await act(async () => {
      renderer!.update(
        <RunHistoryPanel projectDir="other-project" sessionId="other-task" busy={false} />,
      );
    });
    expect(dispose).toHaveBeenCalledTimes(1);
    await act(async () => {
      finishPage(page(1));
      oldListener({ sessionId: "task", channel: "agent_event_committed", payload: event(10) });
    });
    expect(discovery).toHaveBeenCalledTimes(2);
    expect(text()).not.toContain("private-run-task");
  });
  it("stops oversized history before reading another page or showing partial content", async () => {
    readPage.mockImplementation(async (_scope, task, _run, _attempt, cursor) => {
      const sequence = (cursor?.sequence ?? 0) + 1;
      const result = page(sequence, true, task);
      result.events[0].payload.secret = "x".repeat(3 * 1024 * 1024);
      return result;
    });
    await mount();
    expect(readPage).toHaveBeenCalledTimes(3);
    expect(text()).toContain("Execution history exceeds the display budget");
    expect(text()).not.toContain("Current execution event preview");
  });
  it("previews text and tool status without executing or exposing raw tool data", async () => {
    const payloads = [
      { type: "text_delta", delta: "Recorded reply <script>do not execute</script>" },
      { type: "tool_start", id: "tool-one", name: "read_file", input: { secret: "private-input" } },
      {
        type: "tool_end",
        id: "tool-one",
        name: "read_file",
        result: "private-result",
        is_error: false,
      },
      { type: "done", total_input_tokens: 2, total_output_tokens: 3 },
    ];
    const events = payloads.map((payload, index) => ({
      ...event(index + 1),
      kind: "agent.event",
      payload,
    }));
    readPage.mockResolvedValue({
      stream: stream(),
      events,
      next_cursor: events[3].cursor,
      has_more: false,
    });
    await mount();
    expect(text()).toContain("Recorded reply");
    expect(text()).toContain("read_file");
    expect(text()).not.toContain("private-input");
    expect(text()).not.toContain("private-result");
    expect(renderer!.root.findAllByType("script")).toHaveLength(0);
    expect(renderer!.root.findAllByType("button")).toHaveLength(1);
    expect(renderer!.root.findAllByType("ol")[0].findAllByType("li")).toHaveLength(0);
    discovery.mockImplementationOnce(() => new Promise(() => {}));
    renderer!.update(
      <RunHistoryPanel projectDir="other-project" sessionId="other-task" busy={false} />,
    );
    expect(text()).not.toContain("Recorded reply");
    await act(async () => {});
  });

  it("shows projection validation failures without partial chat content", async () => {
    const first = {
      ...event(1),
      kind: "agent.event",
      payload: { type: "text_delta", delta: "partial-private-reply" },
    };
    const second = { ...event(2), kind: "agent.event", payload: { type: "unknown-action" } };
    readPage.mockResolvedValue({
      stream: stream(),
      events: [first, second],
      next_cursor: second.cursor,
      has_more: false,
    });
    await mount();
    expect(text()).toContain("Unknown agent event type");
    expect(text()).not.toContain("partial-private-reply");
    expect(renderer!.root.findByProps({ role: "alert" })).toBeTruthy();
  });
  it("masks old state in the first render of a changed task, before effect clearing", async () => {
    discovery.mockResolvedValueOnce({ ...current(), execution: "succeeded", delivery: "accepted" });
    await mount();
    expect(text()).toContain("Accepted");
    discovery.mockImplementationOnce(() => new Promise(() => {}));
    // Deliberately inspect the committed render without act flushing passive effects.
    renderer!.update(
      <RunHistoryPanel projectDir="other-project" sessionId="other-task" busy={false} />,
    );
    expect(text()).not.toContain("Accepted");
    expect(text()).not.toContain("Delivery not ready");
    expect(text()).toContain("Reading recorded history");
    await act(async () => {});
  });

  it("renders unknown prototype-named payload states as safe fallback labels", async () => {
    const first = { ...event(1), payload: { execution: "__proto__" } };
    const second = { ...event(2), kind: "run.delivery", payload: { delivery: "constructor" } };
    readPage.mockResolvedValue({
      stream: stream(),
      events: [first, second],
      next_cursor: second.cursor,
      has_more: false,
    });
    await mount();
    expect(text()).toContain("Execution state recorded");
    expect(text()).toContain("Delivery decision recorded");
    expect(text()).not.toContain("__proto__");
    expect(renderer!.root.findAllByProps({ role: "alert" })).toHaveLength(0);
  });
  it("loads bounded pages and displays lifecycle labels without payloads or identities", async () => {
    readPage.mockImplementation(
      async (_scope, _task, _run, _attempt, cursor: RunEventCursor | null) =>
        cursor ? page(2) : page(1, true),
    );
    await mount();
    expect(readPage).toHaveBeenCalledTimes(2);
    expect(readPage.mock.calls[1][4]).toEqual(event(1).cursor);
    expect(text()).toContain("Running");
    expect(text()).not.toContain("private-");
    expect(text()).not.toContain("do-not-display");
    expect(
      renderer!.root.findAllByType("button").map((button) => button.children.join("")),
    ).toEqual(["Refresh history"]);
  });
  it("discards delayed discovery after switching project/task, including its page fetch", async () => {
    let resolveOld!: (value: ReturnType<typeof current>) => void;
    discovery.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveOld = resolve;
        }),
    );
    await mount();
    await act(async () => {
      renderer!.update(
        <RunHistoryPanel projectDir="other-project" sessionId="other-task" busy={false} />,
      );
    });
    expect(discovery).toHaveBeenLastCalledWith("other-scope", "other-task");
    await act(async () => {
      resolveOld(current());
    });
    expect(readPage).toHaveBeenCalledTimes(1);
    expect(readPage.mock.calls[0][1]).toBe("other-task");
    expect(text()).not.toContain("private-run-task");
  });
  it("refreshes on busy transition and manual request, coalescing focus/online/visible wakeups", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    try {
      await mount();
      await act(async () => {
        renderer!.update(<RunHistoryPanel {...props} busy />);
      });
      await act(async () => {
        renderer!.root.findByType("button").props.onClick();
      });
      const before = discovery.mock.calls.length;
      await act(async () => {
        window.dispatchEvent(new Event("focus"));
      });
      await act(async () => {
        window.dispatchEvent(new Event("online"));
      });
      await act(async () => {
        document.dispatchEvent(new Event("visibilitychange"));
      });
      // A burst of wakeups costs one immediate reload; the rest merge into one trailing reload.
      expect(discovery).toHaveBeenCalledTimes(before + 1);
      await act(async () => {
        vi.advanceTimersByTime(FOCUS_REFRESH_GAP_MS);
      });
      expect(discovery).toHaveBeenCalledTimes(before + 2);
      Object.assign(document, { visibilityState: "hidden" });
      await act(async () => {
        document.dispatchEvent(new Event("visibilitychange"));
      });
      expect(discovery).toHaveBeenCalledTimes(before + 2);
      act(() => {
        renderer!.unmount();
      });
      renderer = undefined;
      window.dispatchEvent(new Event("focus"));
      expect(discovery).toHaveBeenCalledTimes(before + 2);
    } finally {
      vi.useRealTimers();
    }
  });
  it("shows real errors and rejects corrupt pages without continuing or displaying partial history", async () => {
    readPage.mockResolvedValue(page(2, true));
    await mount();
    expect(text()).toContain("sequence gap");
    expect(renderer!.root.findByProps({ role: "alert" })).toBeTruthy();
    expect(readPage).toHaveBeenCalledTimes(1);
    expect(text()).not.toContain("do-not-display");
    discovery.mockRejectedValueOnce(new Error("scope revoked"));
    await act(async () => {
      renderer!.root.findByType("button").props.onClick();
    });
    expect(text()).toContain("scope revoked");
    expect(readPage).toHaveBeenCalledTimes(1);
  });
  it("clears history on scope removal and supports Chinese empty-history text", async () => {
    language.value = "zh-CN";
    readPage.mockResolvedValue({
      stream: stream(),
      events: [],
      next_cursor: null,
      has_more: false,
    });
    await mount();
    expect(text()).toContain("暂无事件记录");
    scopes.delete("project");
    await act(async () => {
      renderer!.update(<RunHistoryPanel {...props} busy />);
    });
    expect(renderer!.toJSON()).toBeNull();
  });
});
