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
});
