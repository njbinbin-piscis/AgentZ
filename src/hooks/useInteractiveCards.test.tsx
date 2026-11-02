import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INTERACTIVE_WAIT_MS, useInteractiveCards } from "./useInteractiveCards";
import type { AgentEvent } from "../services/tauri/chat";

type Api = ReturnType<typeof useInteractiveCards>;

const show = (id: string, kind?: string) =>
  ({ type: "interactive_ui", request_id: id, ui_definition: { kind, blocks: [] } }) as unknown as AgentEvent;

describe("useInteractiveCards wait timeout", () => {
  let root: ReactTestRenderer | null = null;
  let api: Api;
  const g = globalThis as { window?: unknown; IS_REACT_ACT_ENVIRONMENT?: boolean };
  const origWindow = g.window;

  function Harness() {
    api = useInteractiveCards();
    return null;
  }
  const ids = () => api.pendingCards.map((c) => c.requestId).sort();
  const advance = (ms: number) => act(async () => void vi.advanceTimersByTime(ms));

  beforeEach(async () => {
    vi.useFakeTimers();
    g.window = globalThis;
    g.IS_REACT_ACT_ENVIRONMENT = true;
    await act(async () => {
      root = create(<Harness />);
    });
  });
  afterEach(async () => {
    await act(async () => root?.unmount());
    root = null;
    vi.useRealTimers();
    if (origWindow === undefined) delete g.window;
    else g.window = origWindow;
  });

  it("retires a card once the backend tool has given up waiting", async () => {
    await act(async () => api.handleAgentEvent(show("a")));
    expect(ids()).toEqual(["a"]);
    await advance(INTERACTIVE_WAIT_MS - 1000);
    expect(ids()).toEqual(["a"]);
    await advance(1500);
    expect(ids()).toEqual([]);
  });

  it("a listen event restarts the wait; plan cards are untouched", async () => {
    await act(async () => {
      api.handleAgentEvent(show("a"));
      api.handleAgentEvent(show("p", "plan_mode_build"));
    });
    await advance(INTERACTIVE_WAIT_MS - 1000);
    await act(async () =>
      api.handleAgentEvent({ type: "interactive_ui_listen", request_id: "a" } as unknown as AgentEvent),
    );
    await advance(INTERACTIVE_WAIT_MS - 1000);
    expect(ids()).toEqual(["a", "p"]);
    await advance(2000);
    expect(ids()).toEqual(["p"]);
  });

  it("retires the card when its chat_ui call ends in an error", async () => {
    await act(async () => api.handleAgentEvent(show("a")));
    await act(async () =>
      api.handleAgentEvent({ type: "tool_end", id: "a", name: "chat_ui", result: "timed out", is_error: true } as unknown as AgentEvent),
    );
    expect(ids()).toEqual([]);
  });

  it("keeps the card when chat_ui ends normally (action flow)", async () => {
    await act(async () => api.handleAgentEvent(show("a")));
    await act(async () =>
      api.handleAgentEvent({ type: "tool_end", id: "a", name: "chat_ui", result: "ok", is_error: false } as unknown as AgentEvent),
    );
    expect(ids()).toEqual(["a"]);
  });
});