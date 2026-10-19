import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { readFileMock, writeFileMock, gitDiffMock, invokeMock } = vi.hoisted(() => ({
  readFileMock: vi.fn(),
  writeFileMock: vi.fn(),
  gitDiffMock: vi.fn(),
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

vi.mock("../../services/tauri/ide", () => ({
  ideApi: {
    readFile: readFileMock,
    writeFile: writeFileMock,
    gitDiff: gitDiffMock,
  },
}));

const tFn = (key: string) => key;
const translationHook = { t: tFn };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

vi.mock("../codez/Markdown", () => ({
  default: ({ content }: { content: string }) => <div data-testid="markdown-mock">{content}</div>,
}));

import { consumeUserRecentEdits, peekUserRecentEdits } from "../../services/tauri/chat";
import AgentFilePreview from "./AgentFilePreview";

describe("AgentFilePreview editable canvas (R10-09 & R10-07)", () => {
  let root: ReactTestRenderer | null = null;

  beforeEach(() => {
    readFileMock.mockReset();
    writeFileMock.mockReset();
    gitDiffMock.mockReset();
    invokeMock.mockReset();
    consumeUserRecentEdits("/repo");
    consumeUserRecentEdits("/repo/.worktrees/wt-1");

    readFileMock.mockResolvedValue({
      path: "/repo/.worktrees/wt-1/docs/plan.md",
      content: "# Draft Plan\nStep 1: init\n",
      language: "markdown",
      is_binary: false,
      size: 26,
      sha256: "sha-initial",
    });
    writeFileMock.mockResolvedValue("sha-updated");
    gitDiffMock.mockResolvedValue({
      original: "# Draft Plan\n",
      modified: "# Draft Plan\nStep 1: init\n",
      hunks: [{ old_start: 1, old_lines: 1, new_start: 1, new_lines: 2, header: "@@" }],
    });
  });

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("switches to Edit mode, saves via ideApi.writeFile, records userRecentEdit, and triggers onContinueWithEdits", async () => {
    const onContinueWithEdits = vi.fn();
    await act(async () => {
      root = create(
        <AgentFilePreview
          projectDir="/repo"
          workspaceDir="/repo/.worktrees/wt-1"
          path="docs/plan.md"
          onClose={() => {}}
          onContinueWithEdits={onContinueWithEdits}
        />,
      );
      await Promise.resolve();
      await Promise.resolve();
    });

    const editBtn = root!.root.findByProps({ "data-testid": "preview-mode-edit" });
    await act(async () => {
      editBtn.props.onClick();
    });

    const textarea = root!.root.findByProps({ "data-testid": "preview-edit-textarea" });
    expect(textarea.props.value).toBe("# Draft Plan\nStep 1: init\n");

    await act(async () => {
      textarea.props.onChange({
        target: { value: "# Draft Plan\nStep 1: init\nStep 2: verify\n" },
      });
    });

    const saveBtn = root!.root.findByProps({ "data-testid": "preview-save-btn" });
    await act(async () => {
      await saveBtn.props.onClick();
    });

    expect(writeFileMock).toHaveBeenCalledWith(
      "/repo/.worktrees/wt-1/docs/plan.md",
      "# Draft Plan\nStep 1: init\nStep 2: verify\n",
      "sha-initial",
    );

    const edits = peekUserRecentEdits("/repo/.worktrees/wt-1");
    expect(edits).toHaveLength(1);
    expect(edits[0].path).toBe("docs/plan.md");
    expect(edits[0].diff_summary).toContain("+ Step 2: verify");

    const continueBtn = root!.root.findByProps({ "data-testid": "preview-continue-with-edits" });
    await act(async () => {
      await continueBtn.props.onClick();
    });

    expect(onContinueWithEdits).toHaveBeenCalledWith({
      path: "docs/plan.md",
      content: "# Draft Plan\nStep 1: init\nStep 2: verify\n",
    });
  });
});
