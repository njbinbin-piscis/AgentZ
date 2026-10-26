import { createRef } from "react";
import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import AssistantMessageList from "./AssistantMessageList";

const tFn = (key: string) => key;
const translationHook = { t: tFn };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

vi.mock("./Markdown", () => ({
  default: ({ content }: { content: string }) => <div>{content}</div>,
}));

describe("AssistantMessageList per-turn undo (R10-11)", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("renders per-turn undo button when turnDiffs exist and calls onUndoTurn with turnId", async () => {
    const onUndoTurn = vi.fn();
    const scrollRef = createRef<HTMLDivElement>();

    await act(async () => {
      root = create(
        <AssistantMessageList
          messages={[
            { id: "m1", role: "user", text: "Fix bug" },
            { id: "m2", role: "assistant", text: "Fixed in app.ts", turnId: "turn-42" },
          ]}
          toolSteps={[]}
          turnDiffsByTurnId={{
            "turn-42": [
              {
                id: 1,
                rel_path: "src/app.ts",
                existed: true,
                before: "const a = 1;\n",
                after: "const a = 2;\n",
              },
            ],
          }}
          busy={false}
          queuedView={[]}
          pendingCards={[]}
          scrollRef={scrollRef}
          onForkCheckpoint={() => {}}
          onRestoreCheckpoint={() => {}}
          onUndoTurn={onUndoTurn}
          onCardSubmitted={() => {}}
          onCardActionSent={() => {}}
          permissionRequest={null}
          onPermissionResolved={() => {}}
        />,
      );
    });

    const undoBtn = root!.root.findByProps({ "data-testid": "undo-turn-turn-42" });
    await act(async () => {
      undoBtn.props.onClick();
    });

    expect(onUndoTurn).toHaveBeenCalledWith("turn-42");
  });

  it("shows edit cards only for the newest turn of this run, not older turns or restored history", async () => {
    const scrollRef = createRef<HTMLDivElement>();
    const edit = (id: string) => ({
      id,
      name: "file_edit",
      input: { path: `src/${id}.ts`, old_string: "a", new_string: "b" },
      status: "done" as const,
    });
    const render = async (messages: Array<Record<string, unknown>>) => {
      await act(async () => {
        root = create(
          <AssistantMessageList
            messages={messages as never}
            toolSteps={[]}
            turnDiffsByTurnId={{}}
            busy={false}
            queuedView={[]}
            pendingCards={[]}
            scrollRef={scrollRef}
            onForkCheckpoint={() => {}}
            onRestoreCheckpoint={() => {}}
            onCardSubmitted={() => {}}
            onCardActionSent={() => {}}
            permissionRequest={null}
            onPermissionResolved={() => {}}
          />,
        );
      });
      return root!.root.findAll((n) => n.props.className === "agentz-edit-card-name").map((n) => n.children.join(""));
    };

    // Two turns in this run: only the latest keeps its card.
    expect(
      await render([
        { id: "u1", role: "user", text: "one" },
        { id: "a1", role: "assistant", text: "done 1", turnId: "t1", tools: [edit("old")] },
        { id: "u2", role: "user", text: "two" },
        { id: "a2", role: "assistant", text: "done 2", turnId: "t2", tools: [edit("new")] },
      ]),
    ).toEqual(["new.ts"]);

    await act(async () => root?.unmount());
    // Restored history (no turnId): nothing is shown.
    expect(
      await render([
        { id: "u1", role: "user", text: "one" },
        { id: "a1", role: "assistant", text: "done 1", tools: [edit("restored")] },
      ]),
    ).toEqual([]);
  });});
