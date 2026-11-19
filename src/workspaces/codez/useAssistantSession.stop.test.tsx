// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { countRunTools } from "../../components/toolCancel";
import { STOP_WATCHDOG_MS } from "./sessionTurnBookkeeping";

/**
 * Stop flow of the CodeZ session hook, driven through the real service layer:
 * only the Tauri bridge (`invoke` / `listen`) is replaced.
 */

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}
function lastMessage<T>(messages: T[]): T {
  return messages[messages.length - 1];
}

interface SendCall {
  args: { prompt: string; sessionId: string; taskKey: string };
  result: Deferred<unknown>;
}

const bridge = vi.hoisted(() => ({
  sends: [] as unknown[],
  cancels: [] as unknown[],
  pages: 0,
  history: [] as unknown[],
  listeners: new Map<string, Set<(event: { payload: unknown }) => void>>(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => {
    switch (command) {
      case "chat_send": {
        let resolve!: (value: unknown) => void;
        let reject!: (reason: unknown) => void;
        const promise = new Promise((res, rej) => {
          resolve = res;
          reject = rej;
        });
        bridge.sends.push({ args: args?.args ?? args, result: { promise, resolve, reject } });
        return promise;
      }
      case "chat_cancel":
        bridge.cancels.push(args);
        return Promise.resolve();
      case "chat_get_messages_page":
        bridge.pages += 1;
        return Promise.resolve({ messages: bridge.history, next_offset: bridge.history.length, has_more: false });
      case "is_configured":
        return Promise.resolve(true);
      default:
        return Promise.resolve([]);
    }
  },
  convertFileSrc: (path: string) => path,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, cb: (event: { payload: unknown }) => void) => {
    let set = bridge.listeners.get(name);
    if (!set) bridge.listeners.set(name, (set = new Set()));
    set.add(cb);
    return Promise.resolve(() => set!.delete(cb));
  },
  emit: () => Promise.resolve(),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));

const translation = { t: (key: string) => key, i18n: { language: "zh" } };
vi.mock("react-i18next", () => ({ useTranslation: () => translation }));

const appSettings = { appSettings: null, llmProviders: [], defaultModelLabel: "", defaultModelHint: "" };
vi.mock("../../hooks/useAppSettings", () => ({
  useAppSettings: () => appSettings,
  pruneModelId: (id: string) => id,
}));

const edgeActions = {
  setPendingReview: vi.fn(),
  setArtifacts: vi.fn(),
  onSelectPath: vi.fn(),
  setAgentTurnBusy: vi.fn(),
  scheduleWorkspaceRefresh: vi.fn(),
};
vi.mock("../../contexts/ProjectEdgeContext", () => ({ useProjectEdgeActions: () => edgeActions }));
vi.mock("../../services/tauri/projectScope", () => ({
  useProjectScope: () => null,
  getProjectScope: () => null,
}));

import { useAssistantSession } from "./useAssistantSession";

const PROJECT = "C:/proj";
const NOT_STARTED = "Tool execution was cancelled or interrupted before completion.";

const sendCalls = () => bridge.sends as SendCall[];

function emit(channel: string, sessionId: string, payload: unknown) {
  for (const cb of bridge.listeners.get("agentz:chat-event") ?? []) {
    cb({ payload: { channel, sessionId, payload } });
  }
}

/** Let pending promise callbacks (and the state updates they make) settle. */
async function settle() {
  await act(async () => {
    for (let i = 0; i < 8; i++) await Promise.resolve();
  });
}

async function mountSession() {
  const view = renderHook(() => useAssistantSession({ projectDir: PROJECT, onOpenFolder: () => {} }));
  await settle();
  return view;
}

type View = Awaited<ReturnType<typeof mountSession>>;

async function send(view: View, text: string) {
  await act(async () => {
    view.result.current.onInputChange(text);
  });
  await act(async () => {
    view.result.current.submit();
  });
  await settle();
}

/** Start a turn and bring one tool call on screen; returns the turn's send call. */
async function startTurnWithRunningTool(view: View): Promise<SendCall> {
  await send(view, "first");
  expect(sendCalls()).toHaveLength(1);
  const call = sendCalls()[0];
  await act(async () => {
    emit("agent_event", call.args.sessionId, {
      type: "tool_start",
      id: "t1",
      name: "file_read",
      input: { path: "docs/a.md" },
    });
  });
  expect(view.result.current.busy).toBe(true);
  expect(view.result.current.toolSteps.map((s) => s.status)).toEqual(["running"]);
  return call;
}

function persistedStoppedTurn(sessionText: string) {
  return [
    { id: "u1", role: "user", content: sessionText, tools: [] },
    {
      id: "a1",
      role: "assistant",
      content: "",
      tools: [
        { id: "t1", name: "file_read", input: { path: "docs/a.md" }, result: "已被用户取消", is_error: true, text_offset: 0 },
        // Requested by the model in the same batch, never dispatched.
        { id: "t2", name: "file_read", input: { path: "docs/b.md" }, result: NOT_STARTED, is_error: true, text_offset: 0 },
      ],
    },
  ];
}

beforeEach(() => {
  bridge.sends.length = 0;
  bridge.cancels.length = 0;
  bridge.pages = 0;
  bridge.history = [];
  bridge.listeners.clear();
  // Some imported modules replace globalThis.localStorage with a partial stub
  // (no clear()); install a complete in-memory Storage so the reset is reliable.
  const store = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => void store.set(k, String(v)),
    removeItem: (k: string) => void store.delete(k),
    clear: () => store.clear(),
    key: (i: number) => [...store.keys()][i] ?? null,
    get length() {
      return store.size;
    },
  } satisfies Storage);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useAssistantSession: stopping a turn", () => {
  it("keeps the session running and applies the tool result that arrives after Stop", async () => {
    const view = await mountSession();
    const call = await startTurnWithRunningTool(view);

    await act(async () => {
      view.result.current.stopCurrent();
    });
    expect(bridge.cancels).toEqual([{ taskKey: call.args.taskKey, sessionId: call.args.sessionId }]);
    // Acknowledged at once, but the turn is not over until the backend says so.
    expect(view.result.current.stopping).toBe(true);
    expect(view.result.current.busy).toBe(true);

    await act(async () => {
      emit("agent_event", call.args.sessionId, {
        type: "tool_end",
        id: "t1",
        name: "file_read",
        result: "[trace_id:abc] 已被用户取消",
        is_error: true,
      });
    });
    // The late result is applied, and a stopped call is not a failed call.
    expect(view.result.current.toolSteps.map((s) => s.status)).toEqual(["cancelled"]);

    // A second click has nothing more to do.
    await act(async () => {
      view.result.current.stopCurrent();
    });
    expect(view.result.current.stopping).toBe(true);
  });

  it("shows the same number of tool calls before and after the turn is reloaded", async () => {
    const view = await mountSession();
    const call = await startTurnWithRunningTool(view);
    await act(async () => {
      view.result.current.stopCurrent();
    });
    await act(async () => {
      emit("agent_event", call.args.sessionId, {
        type: "tool_end",
        id: "t1",
        name: "file_read",
        result: "[trace_id:abc] 已被用户取消",
        is_error: true,
      });
    });
    const liveCount = countRunTools(view.result.current.toolSteps);
    expect(liveCount).toBe(1);

    bridge.history = persistedStoppedTurn("first");
    await act(async () => {
      call.result.resolve({ ok: true, session_id: call.args.sessionId, response_text: "", turn_id: null });
    });
    await settle();

    expect(view.result.current.busy).toBe(false);
    expect(view.result.current.stopping).toBe(false);
    // One reconciliation with the database, at the end.
    expect(bridge.pages).toBe(1);
    const reloaded = lastMessage(view.result.current.messages);
    expect(reloaded.id).toBe("a1");
    expect(reloaded.tools!.map((tool) => tool.status)).toEqual(["cancelled", "cancelled"]);
    expect(countRunTools(reloaded.tools!)).toBe(liveCount);
  });

  it("queues a message sent while stopping and starts it after the turn has finished", async () => {
    const view = await mountSession();
    const call = await startTurnWithRunningTool(view);
    await act(async () => {
      view.result.current.stopCurrent();
    });

    await send(view, "second");
    // Queued behind the turn that is still unwinding, not started beside it.
    expect(sendCalls()).toHaveLength(1);
    expect(view.result.current.queuedView).toEqual(["second"]);

    bridge.history = persistedStoppedTurn("first");
    await act(async () => {
      call.result.resolve({ ok: true, session_id: call.args.sessionId, response_text: "", turn_id: null });
    });
    await settle();

    expect(sendCalls()).toHaveLength(2);
    expect(sendCalls()[1].args.prompt).toBe("second");
    expect(sendCalls()[1].args.sessionId).toBe(call.args.sessionId);
    expect(view.result.current.queuedView).toEqual([]);
    expect(view.result.current.busy).toBe(true);
    expect(view.result.current.stopping).toBe(false);
    // The stopped turn's rows are in place, followed by the new turn's bubbles.
    expect(view.result.current.messages.map((m) => [m.role, m.text])).toEqual([
      ["user", "first"],
      ["assistant", ""],
      ["user", "second"],
      ["assistant", ""],
    ]);
  });

  it("releases the session after the watchdog, and a late return cannot overwrite the next turn", async () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
    const view = await mountSession();
    const call = await startTurnWithRunningTool(view);
    await act(async () => {
      view.result.current.stopCurrent();
    });

    await act(async () => {
      vi.advanceTimersByTime(STOP_WATCHDOG_MS - 1);
    });
    expect(view.result.current.busy).toBe(true);

    bridge.history = persistedStoppedTurn("first");
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    await settle();
    expect(view.result.current.busy).toBe(false);
    expect(view.result.current.stopping).toBe(false);
    const pagesAfterRelease = bridge.pages;
    expect(pagesAfterRelease).toBe(1);

    await send(view, "second");
    expect(sendCalls()).toHaveLength(2);
    expect(view.result.current.busy).toBe(true);
    const before = view.result.current.messages;

    // The stopped turn finally returns, with text the new turn must not receive.
    await act(async () => {
      call.result.resolve({
        ok: true,
        session_id: call.args.sessionId,
        response_text: "late text of the stopped turn",
        turn_id: null,
      });
    });
    await settle();

    expect(view.result.current.busy).toBe(true);
    expect(view.result.current.messages).toEqual(before);
    expect(lastMessage(view.result.current.messages)).toMatchObject({ role: "assistant", text: "" });
    expect(bridge.pages).toBe(pagesAfterRelease);
  });
});
