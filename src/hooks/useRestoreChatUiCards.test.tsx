import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ToolTraceItem } from "../components/ToolTrace";

const pending = vi.hoisted(() => ({ ids: [] as string[] }));
vi.mock("../services/tauri/interactive", () => ({
  pendingInteractiveIds: () => Promise.resolve(pending.ids),
}));

import { useInteractiveCards, useRestoreChatUiCards } from "./useInteractiveCards";

const item = (id: string): ToolTraceItem => ({
  id,
  name: "chat_ui",
  status: "error",
  input: { ui_definition: { title: id, blocks: [] } },
});

describe("useRestoreChatUiCards", () => {
  let root: ReactTestRenderer | null = null;
  let cards: ReturnType<typeof useInteractiveCards>["cards"];
  let settled: Set<string>;
  const g = globalThis as { window?: unknown; IS_REACT_ACT_ENVIRONMENT?: boolean };
  const origWindow = g.window;

  function Harness({ items }: { items: ToolTraceItem[] }) {
    const api = useInteractiveCards();
    cards = api.cards;
    settled = useRestoreChatUiCards(items, api.cards, api.restoreCard);
    return null;
  }

  beforeEach(() => {
    g.window = globalThis;
    g.IS_REACT_ACT_ENVIRONMENT = true;
  });
  afterEach(async () => {
    await act(async () => root?.unmount());
    root = null;
    if (origWindow === undefined) delete g.window;
    else g.window = origWindow;
  });

  it("revives the form the backend is still waiting on, and settles the rest", async () => {
    pending.ids = ["live"];
    await act(async () => {
      root = create(<Harness items={[item("live"), item("gone")]} />);
    });
    expect(Object.keys(cards)).toEqual(["live"]);
    expect(cards.live.submitted).toBeUndefined();
    expect([...settled]).toEqual(["gone"]);
  });
});
