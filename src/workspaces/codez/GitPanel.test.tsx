import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import GitPanel from "./GitPanel";
import { ideApi } from "../../services/tauri/ide";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts && "path" in opts ? `${key}:${opts.path}` : key,
  }),
}));

vi.mock("../../services/tauri/ide", () => ({
  ideApi: {
    gitWorkspaceStatus: vi.fn(async () => []),
    gitInit: vi.fn(async () => {}),
  },
}));

describe("GitPanel empty state", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => root?.unmount());
      root = null;
    }
    vi.clearAllMocks();
  });

  async function render() {
    await act(async () => {
      root = create(
        <GitPanel
          projectDir="/plain-folder"
          onDiffClick={() => {}}
          onOpenFile={() => {}}
          onRefresh={async () => {}}
        />,
      );
    });
    await act(async () => {});
    return root!;
  }

  it("offers to initialize a repository when the folder has none", async () => {
    const r = await render();
    const button = r.root.findByProps({ className: "git-init-btn" });
    expect(button.children).toContain("ide.initRepo");
    await act(async () => button.props.onClick());
    expect(ideApi.gitInit).toHaveBeenCalledWith("/plain-folder");
  });

  it("explains when the folder is already inside a repository", async () => {
    vi.mocked(ideApi.gitInit).mockRejectedValueOnce(
      "GIT_ALREADY_INSIDE:C:/parent",
    );
    const r = await render();
    const button = r.root.findByProps({ className: "git-init-btn" });
    await act(async () => button.props.onClick());
    const error = r.root.findByProps({ className: "git-init-error" });
    expect(error.children.join("")).toBe("ide.initRepoInside:C:/parent");
  });
});
