import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { beforeEach, afterEach, describe, it, expect, vi } from "vitest";
const { observe, list, read, capture, remove, saveText, scopes } = vi.hoisted(() => ({
  observe: vi.fn(),
  list: vi.fn(),
  read: vi.fn(),
  capture: vi.fn(),
  remove: vi.fn(),
  saveText: vi.fn(),
  scopes: new Map<string, { handle: string; project_dir: string }>(),
}));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { resolvedLanguage: "en" } }) }));
vi.mock("../../services/tauri/projectScope", () => ({
  useProjectScope: (p: string) => scopes.get(p) ?? null,
}));
vi.mock("../../services/tauri/taskView", () => ({ observeTaskView: observe }));
vi.mock("../../services/tauri/taskArtifacts", () => ({
  listTaskArtifacts: list,
  readTaskArtifact: read,
  captureTaskArtifact: capture,
  deleteTaskArtifact: remove,
  saveTaskArtifactText: saveText,
}));
import TaskArtifactViewer from "./TaskArtifactViewer";
let renderer: ReactTestRenderer | undefined;
const props = { projectDir: "project", sessionId: "task", onClose: vi.fn() };
const view = {
  authority_project: "project",
  workspace_root: "workspace",
  session_id: "task",
  scope: {},
};
const metadata = {
  artifact_id: "path.hash",
  version: 1,
  title: "Old report",
  bytes: 3,
  payload_sha256: "digest",
};
const page = {
  store_missing: false,
  page: { versions: [metadata], next_cursor: ["path.hash", 1], has_more: true },
};
const output = () => JSON.stringify(renderer?.toJSON());
const button = (text: string) =>
  renderer!.root.findAllByType("button").find((b) => b.children.join("").includes(text))!;
beforeEach(() => {
  observe.mockReset();
  list.mockReset();
  read.mockReset();
  capture.mockReset();
  remove.mockReset();
  saveText.mockReset();
  scopes.clear();
  scopes.set("project", { handle: "opaque", project_dir: "project" });
  observe.mockResolvedValue(view);
  list.mockResolvedValue(page);
});
afterEach(async () => {
  await act(async () => renderer?.unmount());
  renderer = undefined;
});
describe("TaskArtifactViewer", () => {
  it("re-observes and re-reads an immutable version before adding it to a continuation draft", async () => {
    const onContinue = vi.fn(() => true);
    read.mockResolvedValue({ scope: { task_id: "task" }, metadata, content: "original" });
    await act(async () => { renderer = create(<TaskArtifactViewer {...props} onContinue={onContinue} />); });
    await act(async () => { button("View text").props.onClick(); });
    await act(async () => { button("Use this version").props.onClick(); });
    expect(observe).toHaveBeenCalledTimes(2);
    expect(read).toHaveBeenCalledTimes(2);
    expect(onContinue).toHaveBeenCalledWith({ scope: { task_id: "task" }, metadata, content: "original" });
  });
  it("does not attach a changed or missing revision to the next message", async () => {
    const onContinue = vi.fn(() => true);
    read.mockResolvedValueOnce({ metadata, content: "original" });
    read.mockResolvedValueOnce({ metadata: { ...metadata, payload_sha256: "changed" }, content: "changed" });
    await act(async () => { renderer = create(<TaskArtifactViewer {...props} onContinue={onContinue} />); });
    await act(async () => { button("View text").props.onClick(); });
    await act(async () => { button("Use this version").props.onClick(); });
    expect(onContinue).not.toHaveBeenCalled();
    expect(output()).toContain("No success is assumed");
  });
  it("edits and restores text through a versioned save, leaving older content available", async () => {
    const old = { ...metadata, version: 1, kind: "report" };
    const latest = { ...metadata, version: 2, kind: "report" };
    list.mockResolvedValue({ store_missing: false, page: { versions: [old, latest], next_cursor: ["path.hash", 2], has_more: false } });
    read.mockResolvedValue({ metadata: old, content: "original" });
    saveText.mockResolvedValue({ ...latest, version: 3 });
    await act(async () => { renderer = create(<TaskArtifactViewer {...props} />); });
    await act(async () => { renderer!.root.findAllByType("button").filter(b => b.children.join("").includes("View text"))[0].props.onClick(); });
    await act(async () => { renderer!.root.findByType("textarea").props.onChange({ target: { value: "edited" } }); });
    await act(async () => { button("Preview draft").props.onClick(); });
    expect(output()).toContain("edited");
    await act(async () => { button("Save as new version").props.onClick(); });
    expect(saveText).toHaveBeenCalledWith("opaque", view, latest, "edited");
    await act(async () => { button("View text").props.onClick(); });
    await act(async () => { button("Restore this version as new").props.onClick(); });
    expect(saveText).toHaveBeenLastCalledWith("opaque", view, latest, "original");
  });
  it("serializes same-tick captures and requires fresh reconciliation after mutation failure", async () => {
    await act(async () => {
      renderer = create(<TaskArtifactViewer {...props} />);
    });
    const inputs = renderer!.root.findAllByType("input");
    await act(async () => {
      inputs[0].props.onChange({ target: { value: "out.txt" } });
      inputs[1].props.onChange({ target: { value: "Report" } });
    });
    let reject!: (error: Error) => void;
    capture.mockImplementationOnce(
      () =>
        new Promise((_, r) => {
          reject = r;
        }),
    );
    const save = button("Save explicit").props.onClick;
    await act(async () => {
      save();
      save();
    });
    expect(capture).toHaveBeenCalledTimes(1);
    await act(async () => {
      reject(new Error("unknown commit"));
    });
    expect(button("Save explicit").props.disabled).toBe(true);
    expect(button("Delete version").props.disabled).toBe(true);
    await act(async () => {
      save();
    });
    expect(capture).toHaveBeenCalledTimes(1);
    observe.mockRejectedValueOnce(new Error("stale observation"));
    await act(async () => {
      button("Refresh versions").props.onClick();
    });
    expect(button("Save explicit").props.disabled).toBe(true);
    await act(async () => {
      button("Refresh versions").props.onClick();
    });
    expect(observe).toHaveBeenCalledTimes(3);
    expect(button("Save explicit").props.disabled).toBe(false);
  });
  it("serializes same-tick deletion confirmations", async () => {
    await act(async () => {
      renderer = create(<TaskArtifactViewer {...props} />);
    });
    await act(async () => {
      button("Delete version").props.onClick();
    });
    let resolve!: (value: string) => void;
    remove.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const confirm = button("Confirm deletion").props.onClick;
    await act(async () => {
      confirm();
      confirm();
    });
    expect(remove).toHaveBeenCalledTimes(1);
    await act(async () => {
      resolve("deleted");
    });
    expect(output()).toContain("Version payload deleted");
  });
  it("observes first, reads immutable version as plain text and explicitly pages", async () => {
    await act(async () => {
      renderer = create(<TaskArtifactViewer {...props} />);
    });
    expect(observe).toHaveBeenCalledWith("opaque", "task", "project");
    expect(list).toHaveBeenCalledWith("opaque", view, null, 200);
    read.mockResolvedValue({ metadata, content: "<script>not executed</script>" });
    await act(async () => {
      button("View text").props.onClick();
    });
    expect(read).toHaveBeenCalledWith("opaque", view, "path.hash", 1);
    expect(renderer!.root.findByType("pre").children.join("")).toContain("<script>");
    expect(renderer!.root.findAllByType("iframe")).toHaveLength(0);
    await act(async () => {
      button("Next page").props.onClick();
    });
    expect(list).toHaveBeenLastCalledWith("opaque", view, ["path.hash", 1], 200);
  });
  it("requires explicit input to capture and confirmation to delete with distinct result", async () => {
    await act(async () => {
      renderer = create(<TaskArtifactViewer {...props} />);
    });
    const inputs = renderer!.root.findAllByType("input");
    await act(async () => {
      inputs[0].props.onChange({ target: { value: "reports/out.txt" } });
      inputs[1].props.onChange({ target: { value: "Report" } });
    });
    capture.mockResolvedValue(metadata);
    await act(async () => {
      button("Save explicit").props.onClick();
    });
    expect(capture).toHaveBeenCalledWith("opaque", view, {
      relativePath: "reports/out.txt",
      title: "Report",
      kind: "report",
    });
    await act(async () => {
      button("Delete version").props.onClick();
    });
    expect(remove).not.toHaveBeenCalled();
    remove.mockResolvedValue("already_deleted");
    await act(async () => {
      button("Confirm deletion").props.onClick();
    });
    expect(output()).toContain("already deleted");
  });
  it("rejects late version content after a task switch", async () => {
    await act(async () => {
      renderer = create(<TaskArtifactViewer {...props} />);
    });
    let resolve!: (v: unknown) => void;
    read.mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    await act(async () => {
      button("View text").props.onClick();
    });
    await act(async () => {
      renderer?.update(<TaskArtifactViewer {...props} sessionId="next" />);
    });
    await act(async () => {
      resolve({ metadata, content: "STALE_SECRET" });
    });
    expect(output()).not.toContain("STALE_SECRET");
  });
  it("does not capture or list without scope, and failed deletion never claims success", async () => {
    scopes.clear();
    await act(async () => {
      renderer = create(<TaskArtifactViewer {...props} />);
    });
    expect(observe).not.toHaveBeenCalled();
    expect(output()).toContain("Unknown");
    scopes.set("project", { handle: "new", project_dir: "project" });
    await act(async () => {
      renderer?.update(<TaskArtifactViewer {...props} />);
    });
    await act(async () => {
      button("Delete version").props.onClick();
    });
    remove.mockRejectedValue(new Error("private path"));
    await act(async () => {
      button("Confirm deletion").props.onClick();
    });
    expect(output()).toContain("No success is assumed");
    expect(output()).not.toContain("private path");
  });
});
