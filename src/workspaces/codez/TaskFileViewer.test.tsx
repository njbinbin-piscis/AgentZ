import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
const { observe, list, read, scopes, language } = vi.hoisted(() => ({
  observe: vi.fn(),
  list: vi.fn(),
  read: vi.fn(),
  scopes: new Map<string, { handle: string; project_dir: string }>(),
  language: { value: "en" },
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ i18n: { resolvedLanguage: language.value } }),
}));
vi.mock("../../services/tauri/projectScope", () => ({
  useProjectScope: (p: string) => scopes.get(p) ?? null,
}));
vi.mock("../../services/tauri/taskView", () => ({ observeTaskView: observe }));
vi.mock("../../services/tauri/taskFiles", () => ({ listTaskFiles: list, readTaskFile: read }));
import TaskFileViewer from "./TaskFileViewer";
let renderer: ReactTestRenderer | undefined;
const props = { projectDir: "project", sessionId: "task", onClose: vi.fn() };
const view = (id = "task") => ({
  session_id: id,
  authority_project: "project",
  workspace_root: `workspace-${id}`,
  scope: {},
});
const output = () => JSON.stringify(renderer?.toJSON());
const button = (text: string) =>
  renderer!.root.findAllByType("button").find((b) => b.children.join("").includes(text))!;
beforeEach(() => {
  language.value = "en";
  observe.mockReset();
  list.mockReset();
  read.mockReset();
  scopes.clear();
  scopes.set("project", { handle: "opaque", project_dir: "project" });
  observe.mockResolvedValue(view());
  list.mockResolvedValue({
    entries: [{ relative_path: "README.md", is_dir: false, size: 3 }],
    truncated: false,
  });
});
afterEach(async () => {
  await act(async () => renderer?.unmount());
  renderer = undefined;
});
describe("TaskFileViewer", () => {
  it("renders a Chinese inline region without modal claims", async () => {
    language.value = "zh-CN";
    scopes.clear();
    await act(async () => {
      renderer = create(<TaskFileViewer {...props} />);
    });
    expect(output()).toContain("只读任务文件");
    expect(output()).toContain("未知");
    expect(output()).toContain("授权范围");
    expect(renderer!.root.findByType("section").props.role).toBe("region");
    expect(renderer!.root.findByType("section").props["aria-modal"]).toBeUndefined();
    button("关闭").props.onClick();
  });
  it("observes authority before listing and reads only scoped relative paths", async () => {
    await act(async () => {
      renderer = create(<TaskFileViewer {...props} />);
    });
    expect(observe).toHaveBeenCalledWith("opaque", "task", "project");
    expect(list).toHaveBeenCalledWith("opaque", view());
    read.mockResolvedValue({ relative_path: "README.md", content: "abc", bytes: 3 });
    await act(async () => {
      button("File:").props.onClick();
    });
    expect(read).toHaveBeenCalledWith("opaque", view(), "README.md");
    expect(output()).toContain("abc");
    expect(output()).toContain("workspace-task");
  });
  it("hides old content and discards late file replies after task switch", async () => {
    await act(async () => {
      renderer = create(<TaskFileViewer {...props} />);
    });
    let resolve!: (value: unknown) => void;
    read.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    await act(async () => {
      button("File:").props.onClick();
    });
    observe.mockResolvedValue(view("next"));
    await act(async () => {
      renderer?.update(<TaskFileViewer {...props} sessionId="next" />);
    });
    await act(async () => {
      resolve({ relative_path: "README.md", content: "OLD_PRIVATE_TEXT", bytes: 16 });
    });
    expect(output()).not.toContain("OLD_PRIVATE_TEXT");
    expect(output()).not.toContain("workspace-task");
    expect(output()).toContain("workspace-next");
  });
  it("shows truncation and safe binary error without stale content or raw host paths", async () => {
    list.mockResolvedValue({
      entries: [{ relative_path: "binary.bin", is_dir: false, size: 8 }],
      truncated: true,
    });
    await act(async () => {
      renderer = create(<TaskFileViewer {...props} />);
    });
    expect(output()).toContain("truncated");
    read.mockRejectedValue(new Error("private-host-path"));
    await act(async () => {
      button("File:").props.onClick();
    });
    expect(output()).toContain("Binary");
    expect(output()).not.toContain("private-host-path");
  });
  it("does not observe without a scope and closes without invoking file APIs", async () => {
    scopes.clear();
    await act(async () => {
      renderer = create(<TaskFileViewer {...props} />);
    });
    expect(observe).not.toHaveBeenCalled();
    expect(output()).toContain("Unknown");
    button("Close").props.onClick();
    expect(props.onClose).toHaveBeenCalled();
    expect(read).not.toHaveBeenCalled();
  });
});
