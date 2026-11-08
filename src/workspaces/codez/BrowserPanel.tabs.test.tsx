import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";

const translationHook = { t: (key: string) => key };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

const svc = vi.hoisted(() => ({
  browserListTabs: vi.fn(),
  browserCloseTab: vi.fn(),
  browserSwitchTab: vi.fn(),
}));

vi.mock("../../services/tauri/browser", () => {
  const ok = () => vi.fn().mockResolvedValue(undefined);
  return {
    browserListTabs: svc.browserListTabs,
    browserCloseTab: svc.browserCloseTab,
    browserSwitchTab: svc.browserSwitchTab,
    browserSetSurface: ok(),
    browserState: vi.fn().mockResolvedValue({ url: "" }),
    browserScreenshot: vi.fn().mockRejectedValue(new Error("no page")),
    browserCurrentUrl: vi.fn().mockResolvedValue(""),
    browserScrollInfo: vi.fn().mockRejectedValue(new Error("no page")),
    browserSetViewport: ok(),
    onBrowserChanged: vi.fn().mockResolvedValue(() => {}),
    browserClose: ok(),
    browserClickAt: ok(),
    browserDragSelect: ok(),
    browserMouseClick: ok(),
    browserSelectAll: ok(),
    browserSelectionText: vi.fn().mockResolvedValue(""),
    browserHistoryStep: ok(),
    browserInsertText: ok(),
    browserPressKey: ok(),
    browserInspectAt: ok(),
    browserNavigate: ok(),
    browserPickAt: ok(),
    browserScrollBy: ok(),
    browserScrollTo: ok(),
  };
});
vi.mock("../../extensions/remotePorts", () => ({
  rewriteRemoteBrowserUrl: async (_dir: unknown, url: string) => url,
}));

import BrowserPanel from "./BrowserPanel";

const tab = (id: string, active: boolean) => ({
  id,
  session_id: "s",
  surface: "codez",
  url: `https://example.com/${id}`,
  active,
});

describe("browser panel tabs", () => {
  let root: ReactTestRenderer | null = null;
  afterEach(async () => {
    if (root) await act(async () => root?.unmount());
    root = null;
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it("every tab has a close button that closes that tab only", async () => {
    // The vitest environment is plain node: give the panel the few globals it touches.
    vi.stubGlobal("window", {
      addEventListener: () => {},
      removeEventListener: () => {},
    });
    vi.stubGlobal("document", { hidden: true });
    vi.stubGlobal("requestAnimationFrame", () => 0);
    vi.stubGlobal("cancelAnimationFrame", () => {});
    svc.browserListTabs.mockResolvedValue([tab("t1", true), tab("t2", false)]);
    svc.browserCloseTab.mockResolvedValue(undefined);
    await act(async () => {
      root = create(
        <BrowserPanel
          sessionId="s"
          onClose={() => {}}
          onSendElementToChat={() => {}}
          onScreenshotToChat={() => {}}
        />,
      );
    });
    const closeButtons = root!.root.findAll(
      (n) => n.type === "button" && n.props.className === "agentz-browser-tab-close",
    );
    expect(closeButtons).toHaveLength(2);

    const stopPropagation = vi.fn();
    await act(async () => {
      closeButtons[1].props.onClick({ stopPropagation });
    });
    expect(stopPropagation).toHaveBeenCalled();
    expect(svc.browserCloseTab).toHaveBeenCalledWith("s", "t2");
    expect(svc.browserSwitchTab).not.toHaveBeenCalled();
  });
});
