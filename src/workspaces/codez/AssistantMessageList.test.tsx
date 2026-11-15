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
  });
});

describe("checkpoint row", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  const cancelledRead = {
    id: "t1",
    name: "file_read",
    input: { path: "docs/a.md" },
    result: "已被用户取消",
    status: "cancelled" as const,
  };

  const renderList = async (
    messages: Array<Record<string, unknown>>,
    extra: { busy?: boolean; stopping?: boolean } = {},
  ) => {
    await act(async () => {
      root = create(
        <AssistantMessageList
          messages={messages as never}
          toolSteps={[]}
          turnDiffsByTurnId={{}}
          busy={extra.busy ?? false}
          stopping={extra.stopping}
          queuedView={[]}
          pendingCards={[]}
          scrollRef={createRef<HTMLDivElement>()}
          onForkCheckpoint={() => {}}
          onRestoreCheckpoint={() => {}}
          onCardSubmitted={() => {}}
          onCardActionSent={() => {}}
          permissionRequest={null}
          onPermissionResolved={() => {}}
        />,
      );
    });
    return root!.root.findAll((n) => n.props.className === "agentz-checkpoint");
  };

  it("is shown for a stopped turn that only made tool calls", async () => {
    const rows = await renderList([
      { id: "u1", role: "user", text: "read the docs" },
      { id: "a1", role: "assistant", text: "", tools: [cancelledRead] },
    ]);
    expect(rows).toHaveLength(1);
  });

  it("is shown for a turn with text, as before", async () => {
    const rows = await renderList([
      { id: "u1", role: "user", text: "hi" },
      { id: "a1", role: "assistant", text: "hello" },
    ]);
    expect(rows).toHaveLength(1);
  });

  it("is not shown for an empty row, an unsaved row, or the turn still running", async () => {
    expect(
      await renderList([
        { id: "u1", role: "user", text: "hi" },
        { id: "a1", role: "assistant", text: "" },
      ]),
    ).toHaveLength(0);
    await act(async () => root?.unmount());
    expect(
      await renderList([
        { id: "u1", role: "user", text: "hi" },
        { role: "assistant", text: "", tools: [cancelledRead] },
      ]),
    ).toHaveLength(0);
    await act(async () => root?.unmount());
    expect(
      await renderList(
        [
          { id: "u1", role: "user", text: "hi" },
          { id: "a1", role: "assistant", text: "", tools: [cancelledRead] },
        ],
        { busy: true, stopping: true },
      ),
    ).toHaveLength(0);
  });

  it("says the turn is stopping instead of thinking once Stop was clicked", async () => {
    const texts = async (stopping: boolean) => {
      await renderList(
        [
          { id: "u1", role: "user", text: "hi" },
          { role: "assistant", text: "" },
        ],
        { busy: true, stopping },
      );
      const found = root!.root
        .findAll((n) => n.props.className === "agentz-msg-text agentz-thinking")
        .map((n) => n.children.join(""));
      await act(async () => root?.unmount());
      root = null;
      return found;
    };
    expect(await texts(false)).toEqual(["chat.thinking"]);
    expect(await texts(true)).toEqual(["chat.stopping"]);
  });
});

describe("queued follow-ups", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("renders interrupt, steer, and remove actions", async () => {
    const onInterruptQueued = vi.fn();
    const onSteerQueued = vi.fn();
    const onEditQueued = vi.fn();
    const onRemoveQueued = vi.fn();
    const scrollRef = createRef<HTMLDivElement>();

    await act(async () => {
      root = create(
        <AssistantMessageList
          messages={[]}
          toolSteps={[]}
          turnDiffsByTurnId={{}}
          busy
          queuedView={["watch the failing test"]}
          onInterruptQueued={onInterruptQueued}
          onSteerQueued={onSteerQueued}
          onEditQueued={onEditQueued}
          onRemoveQueued={onRemoveQueued}
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

    const interrupt = root!.root.findByProps({ "aria-label": "chat.queueInterruptSend" });
    const steer = root!.root.findByProps({ "aria-label": "chat.queueSteerNext" });
    const edit = root!.root.findByProps({ "aria-label": "chat.queueEditReturn" });
    const remove = root!.root.findByProps({ "aria-label": "chat.removeFromQueue" });
    await act(async () => {
      interrupt.props.onClick();
      steer.props.onClick();
      edit.props.onClick();
      remove.props.onClick();
    });
    expect(onInterruptQueued).toHaveBeenCalledWith(0);
    expect(onSteerQueued).toHaveBeenCalledWith(0);
    expect(onEditQueued).toHaveBeenCalledWith(0);
    expect(onRemoveQueued).toHaveBeenCalledWith(0);
  });
});
