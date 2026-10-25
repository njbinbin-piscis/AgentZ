import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fileEditDiff } from "./fileEditDiff";
import ToolTrace from "./ToolTrace";

describe("fileEditDiff", () => {
  it("counts changed lines of a single edit", () => {
    const d = fileEditDiff("file_edit", {
      path: "src/a.ts",
      old_string: "a\nb\nc\n",
      new_string: "a\nB\nc\nd\n",
    })!;
    expect([d.added, d.removed]).toEqual([2, 1]);
    expect(d.ops.map((o) => o.type)).toEqual(["equal", "remove", "add", "equal", "add"]);
  });

  it("joins batch edits and treats file_write as all added", () => {
    const d = fileEditDiff("file_edit", {
      path: "x.rs",
      edits: [
        { old_string: "1", new_string: "2" },
        { old_string: "3\r\n4", new_string: "3\r\n4\r\n5" },
      ],
    })!;
    expect([d.added, d.removed]).toEqual([2, 1]);
    expect(fileEditDiff("file_write", { path: "n.md", content: "l1\nl2\n" })).toMatchObject({
      added: 2,
      removed: 0,
    });
    expect(fileEditDiff("file_read", { path: "n.md" })).toBeNull();
  });
});

describe("ToolTrace edit card", () => {
  const origWindow = (globalThis as { window?: unknown }).window;
  afterEach(() => {
    (globalThis as { window?: unknown }).window = origWindow;
  });

  it("shows counts, expands the diff and opens the file", async () => {
    const dispatchEvent = vi.fn();
    (globalThis as { window?: unknown }).window = { dispatchEvent };
    let root!: ReactTestRenderer;
    await act(async () => {
      root = create(
        <ToolTrace
          items={[
            { id: "1", name: "shell", input: { command: "ls" }, status: "done" },
            {
              id: "2",
              name: "file_edit",
              input: { path: "src/app/main.ts", old_string: "old()", new_string: "fresh()\nmore()" },
              status: "done",
            },
          ]}
        />,
      );
    });
    const text = (cls: string) =>
      root.root.findAll((n) => n.props.className === cls).map((n) => n.children.join(""));
    expect(text("agentz-edit-card-name")).toEqual(["main.ts"]);
    expect(text("agentz-edit-card-add")).toEqual(["+2"]);
    expect(text("agentz-edit-card-del")).toEqual(["-1"]);
    expect(root.root.findAll((n) => n.props.className === "agentz-edit-card-diff")).toHaveLength(0);

    await act(async () => {
      root.root.findByProps({ className: "agentz-edit-card-toggle" }).props.onClick();
    });
    const lines = root.root.findAll(
      (n) => typeof n.props.className === "string" && n.props.className.startsWith("agentz-edit-line "),
    );
    expect(lines.map((n) => n.props.className)).toEqual([
      "agentz-edit-line is-remove",
      "agentz-edit-line is-add",
      "agentz-edit-line is-add",
    ]);

    root.root.findByProps({ className: "agentz-edit-card-file" }).props.onClick();
    expect(dispatchEvent.mock.calls[0][0].type).toBe("agentz-open-file");
    expect(dispatchEvent.mock.calls[0][0].detail).toEqual({ path: "src/app/main.ts" });
    await act(async () => root.unmount());
  });
});
