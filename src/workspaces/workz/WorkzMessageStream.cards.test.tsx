import { createRef } from "react";
import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import WorkzMessageStream from "./WorkzMessageStream";

const translationHook = { t: (key: string) => key };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));
vi.mock("../codez/Markdown", () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));
vi.mock("../../components/chat/InteractiveCard", () => ({
  default: ({ requestId }: { requestId: string }) => <div data-card={requestId} />,
}));
vi.mock("../../services/tauri/chat", () => ({ journalUndoTurn: vi.fn() }));
vi.mock("../../services/tauri/confirm", () => ({ confirmDialog: vi.fn() }));
vi.mock("../../hooks/useInteractiveCards", () => ({
  useRestoreChatUiCards: () => new Set<string>(),
}));

const definition = { title: "form", fields: [] } as never;
const card = (requestId: string) => ({ requestId, uiDefinition: definition, listenOpen: false });

describe("WorkZ chat_ui cards stay in the conversation flow", () => {
  let root: ReactTestRenderer | null = null;
  afterEach(async () => {
    if (root) await act(async () => root?.unmount());
    root = null;
  });

  const render = async (steps: never[], pending: ReturnType<typeof card>[]) => {
    await act(async () => {
      root = create(
        <WorkzMessageStream
          projectDir="/p"
          isFree={false}
          freeTaskWorkspace={null}
          isSwarmTeam={false}
          swarmMainTab="main"
          onChangeSwarmTab={() => {}}
          activePoolId={null}
          scrollRef={createRef<HTMLDivElement>()}
          steps={steps}
          busy={false}
          selectedId="s"
          sessionId="s"
          pendingCards={pending}
          allCards={Object.fromEntries(pending.map((c) => [c.requestId, c]))}
          permissionRequest={null}
          onClearPermission={() => {}}
          onMarkCardSubmitted={() => {}}
          onMarkCardActionSent={() => {}}
          onSelectScenario={() => {}}
          onRefreshGitChanges={() => {}}
          onShowToast={() => {}}
          onSetError={() => {}}
        />,
      );
    });
    return root!.root.findAll((n) => typeof n.props["data-card"] === "string" && typeof n.type === "string");
  };

  const chatUiStep = (id: string) => ({
    role: "assistant",
    text: "",
    tools: [{ id, name: "chat_ui", input: { ui_definition: definition }, status: "running" }],
  });

  it("renders a card once, inside its message, and never again at the bottom", async () => {
    const nodes = await render(
      [{ id: "u1", role: "user", text: "go", tools: [] }, { id: "a1", ...chatUiStep("r1") }] as never[],
      [card("r1")],
    );
    expect(nodes.map((n) => n.props["data-card"])).toEqual(["r1"]);
    // The one copy sits inside the assistant message, not in a trailing block.
    let up = nodes[0].parent;
    while (up && typeof up.props.className !== "string") up = up.parent;
    expect(up?.props.className).toBe("agentz-chat-ui-inline");
  });

  it("older cards in earlier messages do not pile up at the bottom", async () => {
    const nodes = await render(
      [
        { id: "a1", ...chatUiStep("r1") },
        { id: "u2", role: "user", text: "again", tools: [] },
        { id: "a2", ...chatUiStep("r2") },
      ] as never[],
      [card("r1"), card("r2")],
    );
    expect(nodes.map((n) => n.props["data-card"])).toEqual(["r1", "r2"]);
  });

  it("only a card with no message of its own falls back to the bottom", async () => {
    const nodes = await render([{ id: "a1", ...chatUiStep("r1") }] as never[], [card("r1"), card("orphan")]);
    expect(nodes.map((n) => n.props["data-card"])).toEqual(["r1", "orphan"]);
  });
});
