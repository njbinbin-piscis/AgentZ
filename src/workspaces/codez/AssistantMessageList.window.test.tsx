// @vitest-environment jsdom
import { createRef } from "react";
import { render, cleanup, fireEvent, act } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import AssistantMessageList from "./AssistantMessageList";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("./Markdown", () => ({ default: ({ content }: { content: string }) => <div>{content}</div> }));
vi.mock("../../components/ContextMenu", async () => {
  const actual = await vi.importActual<typeof import("../../components/ContextMenu")>("../../components/ContextMenu");
  return { ...actual, useContextMenu: () => ({ openWith: () => () => {}, element: null, state: null, close: () => {} }) };
});
vi.mock("../../components/ToolTrace", () => ({ ToolTrace: () => null, FileEditCard: () => null, SubagentCard: () => null }));
vi.mock("../../components/TaskCard", () => ({ default: ({ text }: { text: string }) => <div>{text}</div> }));
vi.mock("./chatUiRestore", () => ({ useRestoreChatUiCards: () => new Set() }));

// loadOlder releases its lock inside requestAnimationFrame; run callbacks
// synchronously so consecutive "load older" clicks are observable in jsdom.
vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
  cb(0);
  return 1;
});
vi.stubGlobal("cancelAnimationFrame", () => {});

const msgs = (n: number, prefix: string) =>
  Array.from({ length: n }, (_, i) => ({
    id: `${prefix}-${i}`,
    role: (i % 2 === 0 ? "user" : "assistant") as "user" | "assistant",
    text: `${prefix} message ${i}`,
  }));

function baseProps(scrollRef: { current: HTMLDivElement | null }) {
  return {
    toolSteps: [],
    turnDiffsByTurnId: {},
    busy: false,
    queuedView: [],
    onRemoveQueued: () => {},
    onInterruptQueued: () => {},
    onSteerQueued: () => {},
    onEditQueued: () => {},
    pendingCards: [],
    scrollRef,
    hasMoreOlder: false,
    onSelectPath: () => {},
    onForkCheckpoint: async () => {},
    onRestoreCheckpoint: async () => {},
    onCardSubmitted: () => {},
    onCardActionSent: () => {},
    permissionRequest: null,
    onPermissionResolved: () => {},
  };
}

afterEach(() => cleanup());

describe("AssistantMessageList session window", () => {
  it("resets the lazy-load window to the newest messages when the session changes", async () => {
    const scrollRef = createRef<HTMLDivElement>();
    const sessionA = msgs(80, "A");
    const view = render(
      <AssistantMessageList {...baseProps(scrollRef)} messages={sessionA} sessionId="session-A" />,
    );
    const el = scrollRef.current!;
    expect(el).toBeTruthy();
    // jsdom has no layout: give the container a measurable height so the
    // anchoring math in loadOlder runs against real numbers.
    Object.defineProperty(el, "clientHeight", { value: 200, configurable: true });
    Object.defineProperty(el, "scrollHeight", { value: 2000, configurable: true });

    const text = () => el.textContent ?? "";
    // Window shows the newest 50 of 80 → first mounted row is A-30.
    expect(text()).toContain("A message 79");
    expect(text()).not.toContain("A message 0");

    // User loads older messages until the very first row is mounted.
    for (let i = 0; i < 4; i++) {
      const btn = el.querySelector<HTMLButtonElement>(".agentz-load-more-msgs");
      if (!btn) break;
      await act(async () => {
        fireEvent.click(btn);
      });
    }
    expect(text(), "older loading must reach the first message").toContain("A message 0");

    // Switch to session B (also long): the window must reset — B's first
    // message must NOT be mounted; the newest must be.
    const sessionB = msgs(90, "B");
    await act(async () => {
      view.rerender(
        <AssistantMessageList {...baseProps(scrollRef)} messages={sessionB} sessionId="session-B" />,
      );
    });
    expect(text(), "new session must show its newest message").toContain("B message 89");
    expect(text(), "new session must not start at its first message").not.toContain("B message 0");
  });
});
