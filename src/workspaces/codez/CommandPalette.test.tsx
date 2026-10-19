import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import CommandPalette, { flattenFileTreePaths, fuzzyScore } from "./CommandPalette";
import type { FileNode } from "./types";

const tFn = (key: string) => key;
const translationHook = { t: tFn };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

const sampleTree: FileNode[] = [
  {
    name: "src",
    path: "src",
    is_dir: true,
    size: 0,
    modified: null,
    children: [
      { name: "App.tsx", path: "src/App.tsx", is_dir: false, size: 100, modified: null, children: null },
      { name: "main.tsx", path: "src/main.tsx", is_dir: false, size: 100, modified: null, children: null },
    ],
  },
  { name: "package.json", path: "package.json", is_dir: false, size: 100, modified: null, children: null },
];

describe("CommandPalette & Quick Open (R10-10)", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("flattens file tree and scores fuzzy matches accurately", () => {
    const flat = flattenFileTreePaths(sampleTree);
    expect(flat).toEqual(["src/App.tsx", "src/main.tsx", "package.json"]);
    expect(fuzzyScore("src/App.tsx", "app")).toBeGreaterThan(fuzzyScore("package.json", "app"));
    expect(fuzzyScore("src/App.tsx", "zzz")).toBe(-1);
  });

  it("filters files in Quick Open mode and opens the selected file on Enter", async () => {
    const onSelectFile = vi.fn();
    const onClose = vi.fn();

    await act(async () => {
      root = create(
        <CommandPalette
          open={true}
          initialMode="files"
          fileTree={sampleTree}
          commands={[]}
          onSelectFile={onSelectFile}
          onClose={onClose}
        />,
      );
    });

    const input = root!.root.findByProps({ "data-testid": "command-palette-input" });
    await act(async () => {
      input.props.onChange({ target: { value: "main" } });
    });

    await act(async () => {
      input.props.onKeyDown({ key: "Enter", preventDefault: () => {} });
    });

    expect(onSelectFile).toHaveBeenCalledWith("src/main.tsx");
    expect(onClose).toHaveBeenCalled();
  });

  it("switches to command mode with '>' prefix and runs the selected command", async () => {
    const runToggleTerminal = vi.fn();
    const onClose = vi.fn();

    await act(async () => {
      root = create(
        <CommandPalette
          open={true}
          initialMode="commands"
          fileTree={sampleTree}
          commands={[
            {
              id: "toggle-terminal",
              label: "View: Toggle Terminal Panel",
              shortcut: "Ctrl+`",
              run: runToggleTerminal,
            },
          ]}
          onSelectFile={() => {}}
          onClose={onClose}
        />,
      );
    });

    const cmdBtn = root!.root.findByProps({ "data-testid": "palette-cmd-toggle-terminal" });
    await act(async () => {
      cmdBtn.props.onClick();
    });

    expect(runToggleTerminal).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });
});
