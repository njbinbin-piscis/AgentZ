import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
const { api } = vi.hoisted(() => ({
  api: {
    changeSet: vi.fn(),
    fileView: vi.fn(),
    merge: vi.fn(),
    mergePreview: vi.fn(),
    discard: vi.fn(),
    openPr: vi.fn(),
  },
}));
vi.mock("../../services/tauri/agentTask", () => ({ agentTaskApi: api }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, args?: { msg?: string }) => (args?.msg ? `${key}: ${args.msg}` : key),
  }),
}));
import AgentTaskReview from "./AgentTaskReview";

const task = {
  id: "task-a",
  branch: "workz/task-a",
  worktree_path: "/task-a",
  base: "main",
  base_sha: "frozen-sha",
  environment_id: "env-a",
};
const change = (path: string, source = "committed", binary = false) => ({
  path,
  status: "M",
  old_path: null,
  binary,
  sources: [source],
});
const view = (path: string, binary = false) => ({
  path,
  original: "before",
  modified: "after",
  binary,
  review_version: "base:original:modified",
});
const props = { projectDir: "/project", task, onClose: vi.fn(), onResolved: vi.fn() };
let renderer: ReactTestRenderer | undefined;
const text = () => JSON.stringify(renderer!.toJSON());
const button = (label: string) =>
  renderer!.root.findAllByType("button").find((node) => node.children.includes(label))!;
async function mount() {
  await act(async () => {
    renderer = create(<AgentTaskReview {...props} />);
  });
}
beforeEach(() => {
  Object.values(api).forEach((mock) => mock.mockReset());
  props.onResolved.mockReset();
  props.onClose.mockReset();
  vi.stubGlobal("window", { confirm: vi.fn(() => true) });
  api.changeSet.mockResolvedValue({
    head_sha: "review-head",
    base_sha: "frozen-sha",
    changes: [],
    excluded_runtime_paths: [],
  });
  api.fileView.mockImplementation(async (_project, _task, path) => view(path));
  api.mergePreview.mockResolvedValue({
    task_head: "review-head",
    target_head: "target-head",
    target_branch: "main",
  });
  api.merge.mockResolvedValue({ state: "merged", recovery_ref: "refs/agentz/merges/receipt" });
});
afterEach(() => {
  if (renderer) act(() => renderer!.unmount());
  renderer = undefined;
  vi.unstubAllGlobals();
});

describe("complete task review", () => {
  it("shows all change sources and the old rename path", async () => {
    api.changeSet.mockResolvedValue({
      changes: [
        {
          ...change("renamed.txt"),
          old_path: "old.txt",
          sources: ["committed", "staged", "unstaged", "untracked"],
        },
      ],
    });
    await mount();
    for (const source of ["committed", "staged", "unstaged", "untracked"])
      expect(text()).toContain(`agent.reviewSource.${source}`);
    expect(text()).toContain("old.txt → ");
    expect(text()).toContain("renamed.txt");
  });

  it("does not submit a merge after its preview outlives the task review", async () => {
    let resolvePreview!: (value: unknown) => void;
    api.changeSet.mockResolvedValue({ head_sha: "review-head", changes: [change("file.txt")] });
    api.mergePreview.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolvePreview = resolve;
        }),
    );
    await mount();
    let operation!: Promise<void>;
    act(() => {
      operation = button("agent.merge").props.onClick();
    });
    await act(async () => {
      renderer!.update(
        <AgentTaskReview {...props} task={{ ...task, id: "task-b", environment_id: "env-b" }} />,
      );
    });
    await act(async () => {
      resolvePreview({ target_head: "target-head" });
      await operation;
    });
    expect(api.merge).not.toHaveBeenCalled();
    expect(props.onResolved).not.toHaveBeenCalled();
    expect(button("agent.merge").props.disabled).toBe(false);
  });

  it("does not show an old archive receipt or notify the new task", async () => {
    let resolveArchive!: (value: unknown) => void;
    api.discard.mockImplementation(
      () =>
        new Promise((resolve) => {
          resolveArchive = resolve;
        }),
    );
    await mount();
    let operation!: Promise<void>;
    act(() => {
      operation = button("agent.discard").props.onClick();
    });
    await act(async () => {
      renderer!.update(<AgentTaskReview {...props} task={{ ...task, id: "task-b" }} />);
    });
    await act(async () => {
      resolveArchive({ retained_root: "/retained/old-task", state: "archived" });
      await operation;
    });
    expect(text()).not.toContain("/retained/old-task");
    expect(text()).not.toContain("agent.archiveOk");
    expect(props.onResolved).not.toHaveBeenCalled();
    expect(button("agent.discard").props.disabled).toBe(false);
  });

  it("clears a prior task result when only its environment binding changes", async () => {
    api.discard.mockResolvedValue({ retained_root: "/retained/old-env", state: "archived" });
    await mount();
    await act(async () => {
      await button("agent.discard").props.onClick();
    });
    expect(text()).toContain("/retained/old-env");
    await act(async () => {
      renderer!.update(
        <AgentTaskReview {...props} task={{ ...task, environment_id: "env-new" }} />,
      );
    });
    expect(text()).not.toContain("/retained/old-env");
    expect(button("agent.discard").props.disabled).toBe(false);
  });

  it("ignores an old file response after switching tasks with the same path", async () => {
    let resolveOldFile!: (value: unknown) => void;
    api.changeSet.mockResolvedValue({ changes: [change("shared.txt")] });
    api.fileView.mockImplementation((_project, id) =>
      id === "task-a"
        ? new Promise((resolve) => {
            resolveOldFile = resolve;
          })
        : Promise.resolve({ ...view("shared.txt"), modified: "new task content" }),
    );
    await mount();
    await act(async () => {
      renderer!.update(<AgentTaskReview {...props} task={{ ...task, id: "task-b" }} />);
    });
    await act(async () => {
      resolveOldFile({ ...view("shared.txt"), modified: "stale task content" });
    });
    expect(text()).toContain("new task content");
    expect(text()).not.toContain("stale task content");
  });

  it("does not merge when preflight detects a stale reviewed version", async () => {
    api.changeSet.mockResolvedValue({ head_sha: "review-head", changes: [change("file.txt")] });
    api.mergePreview.mockRejectedValue({
      stage: "review_changed",
      message: "task changed; refresh",
    });
    await mount();
    await act(async () => {
      await button("agent.merge").props.onClick();
    });
    expect(text()).toContain("task changed; refresh");
    expect(api.merge).not.toHaveBeenCalled();
    expect(props.onResolved).not.toHaveBeenCalled();
  });
  it("retains a failed transaction's recovery reference and prevents a second merge", async () => {
    api.changeSet.mockResolvedValue({ head_sha: "review-head", changes: [change("file.txt")] });
    api.merge.mockRejectedValue({
      message: "abort failed",
      state: "needs_recovery",
      recovery_ref: "refs/agentz/merges/failed",
    });
    await mount();
    await act(async () => {
      await button("agent.merge").props.onClick();
    });
    expect(text()).toContain("abort failed");
    expect(text()).toContain("refs/agentz/merges/failed");
    expect(button("agent.merge").props.disabled).toBe(true);
    expect(props.onResolved).not.toHaveBeenCalled();
  });
  it("requires a reviewed task HEAD before enabling merge", async () => {
    api.changeSet.mockResolvedValue({ changes: [change("file.txt")] });
    await mount();
    expect(button("agent.merge").props.disabled).toBe(true);
  });
  it("archives dirty content and displays the retained path", async () => {
    api.changeSet.mockResolvedValue({ changes: [change("dirty.txt", "unstaged")] });
    api.discard.mockResolvedValue({ retained_root: "/retained/a", state: "archived" });
    await mount();
    await act(async () => {
      await button("agent.discard").props.onClick();
    });
    expect(api.discard).toHaveBeenCalledWith("/project", "/task-a", "workz/task-a", "task-a");
    expect(text()).toContain("/retained/a");
    expect(props.onResolved).toHaveBeenCalledOnce();
    expect(props.onClose).not.toHaveBeenCalled();
    expect(button("agent.discard").props.disabled).toBe(true);
    expect(button("agent.reviewRefresh").props.disabled).toBe(true);
  });
  it("reports partial archive failure with a recovery location", async () => {
    api.discard.mockRejectedValue({
      stage: "receipt",
      message: "receipt write failed",
      retained_root: "/retained/a",
    });
    await mount();
    await act(async () => {
      await button("agent.discard").props.onClick();
    });
    expect(text()).toContain("receipt write failed");
    expect(text()).toContain("/retained/a");
    expect(props.onResolved).not.toHaveBeenCalled();
    expect(button("agent.discard").props.disabled).toBe(true);
  });
  it("shows a read error instead of an empty success and disables actions", async () => {
    api.changeSet.mockRejectedValue({ code: "environment_invalid", message: "wrong branch" });
    await mount();
    expect(text()).toContain("wrong branch");
    expect(text()).not.toContain("agent.reviewEmpty");
    expect(button("agent.merge").props.disabled).toBe(true);
    expect(button("agent.discard").props.disabled).toBe(true);
  });
  it("renders working files and blocks operations that would omit uncommitted content", async () => {
    api.changeSet.mockResolvedValue({ changes: [change("new file.txt", "untracked")] });
    await mount();
    expect(text()).toContain("new file.txt");
    expect(text()).toContain("agent.reviewSource.untracked");
    expect(text()).toContain("after");
    expect(api.fileView).toHaveBeenCalledWith("/project", "task-a", "new file.txt");
    for (const label of ["agent.merge", "agent.openPr"])
      expect(button(label).props.disabled).toBe(true);
    expect(button("agent.discard").props.disabled).toBe(false);
  });
  it("shows binary content explicitly and identifies the captured review version", async () => {
    api.changeSet.mockResolvedValue({ changes: [change("image.bin", "committed", true)] });
    api.fileView.mockResolvedValue(view("image.bin", true));
    await mount();
    expect(text()).toContain("agent.reviewBinaryNoPreview");
    expect(text()).toContain("agent.reviewVersion");
    expect(text()).not.toContain("agentz-task-review-diff-pre");
  });
  it("blocks branch operations when a dirty file has canceled its net base diff", async () => {
    api.changeSet.mockResolvedValue({
      changes: [change("other-committed.txt")],
      has_uncommitted: true,
    });
    await mount();
    expect(button("agent.merge").props.disabled).toBe(true);
    expect(button("agent.openPr").props.disabled).toBe(true);
  });
  it("keeps file read failure distinct from no change", async () => {
    api.changeSet.mockResolvedValue({ changes: [change("file.txt")] });
    api.fileView.mockRejectedValue({ code: "file_read_failed", message: "file is unreadable" });
    await mount();
    expect(text()).toContain("file is unreadable");
    expect(text()).not.toContain("agentz-task-review-diff-pre");
    expect(button("agent.merge").props.disabled).toBe(true);
  });
  it("ignores an older task response after switching tasks", async () => {
    let resolveOld!: (value: unknown) => void;
    api.changeSet.mockImplementation((_root, id) =>
      id === "task-a"
        ? new Promise((resolve) => {
            resolveOld = resolve;
          })
        : Promise.resolve({ changes: [] }),
    );
    await mount();
    expect(text()).toContain("common.loading");
    await act(async () => {
      renderer!.update(<AgentTaskReview {...props} task={{ ...task, id: "task-b" }} />);
    });
    await act(async () => {
      resolveOld({ changes: [change("old-task-file.txt")] });
    });
    expect(text()).not.toContain("old-task-file.txt");
    expect(text()).toContain("agent.reviewEmpty");
  });
  it("merges into the target branch rather than the frozen comparison SHA", async () => {
    api.changeSet.mockResolvedValue({
      head_sha: "review-head",
      changes: [change("committed.txt")],
    });
    await mount();
    expect(button("agent.merge").props.disabled).toBe(false);
    await act(async () => {
      await button("agent.merge").props.onClick();
    });
    expect(api.mergePreview).toHaveBeenCalledWith("/project", "task-a", "main", "review-head");
    expect(api.merge).toHaveBeenCalledWith(
      "/project",
      "task-a",
      "main",
      "review-head",
      "target-head",
    );
    expect(props.onResolved).toHaveBeenCalled();
  });
});
