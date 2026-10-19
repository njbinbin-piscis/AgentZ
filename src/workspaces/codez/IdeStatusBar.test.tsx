import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import IdeStatusBar, {
  estimateTokenCostUsd,
  formatCodiconText,
  formatCompactTokens,
} from "./IdeStatusBar";
import zh from "../../i18n/zh";
import en from "../../i18n/en";

const tFn = (key: string) => key;
const translationHook = { t: tFn };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

const extUiState = vi.hoisted(() => ({ running: false }));

vi.mock("../../extensions/ui/useExtensionUi", () => ({
  useExtensionUi: () => ({
    statusBar: [],
    running: extUiState.running,
    scm: [],
    hostError: null,
  }),
}));

vi.mock("../../extensions/remotePorts", () => ({
  useRemotePortWatch: () => [],
}));

const extServiceMock = vi.hoisted(() => ({
  start: vi.fn(() => Promise.resolve()),
  executeCommand: vi.fn(),
  isRunning: false,
}));

vi.mock("../../extensions/extensionService", () => ({
  extensionService: extServiceMock,
}));

let chatListener: ((env: unknown) => void) | null = null;
vi.mock("../../services/tauri/chat", () => ({
  onChatEvent: (cb: (env: unknown) => void) => {
    chatListener = cb;
    return Promise.resolve(() => {});
  },
}));

vi.mock("../../hooks/useAppSettings", () => ({
  useAppSettings: () => ({
    appSettings: null,
    llmProviders: [
      { id: "claude-sonnet", label: "Claude Sonnet", provider: "anthropic", model: "claude-3-7" },
      { id: "gpt-4o", label: "GPT-4o", provider: "openai", model: "gpt-4o" },
    ],
    defaultModelLabel: "Default (Claude)",
    defaultModelHint: "",
    reloadSettings: () => {},
  }),
}));

describe("IdeStatusBar model switcher & token/cost pill (R10-13)", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    chatListener = null;
    extUiState.running = false;
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("formats token counts and estimates cost accurately", () => {
    expect(formatCompactTokens(500)).toBe("500");
    expect(formatCompactTokens(12_500)).toBe("12.5k");
    expect(estimateTokenCostUsd(100_000, 10_000)).toBe("$0.450");
  });

  it("provides translated chat.modelTitle and chat.defaultModel in zh and en locales", () => {
    expect(zh.chat.modelTitle).toBeTruthy();
    expect(zh.chat.modelTitle).not.toBe("chat.modelTitle");
    expect(en.chat.modelTitle).toBeTruthy();
    expect(en.chat.modelTitle).not.toBe("chat.modelTitle");
    expect(zh.chat.defaultModel).toBeTruthy();
    expect(en.chat.defaultModel).toBeTruthy();
  });

  it("formats VS Code $(codicon) tokens and strips raw $Version placeholders", () => {
    expect(formatCodiconText("$(versions) Node 22.14.0")).toBe("⧉ Node 22.14.0");
    expect(formatCodiconText("$(play) Node Run")).toBe("▶ Node Run");
    expect(formatCodiconText("Node.$Version")).toBe("Node.");
    expect(formatCodiconText("$(sync~spin) Loading")).toBe("↻ Loading");
  });

  it("updates token/cost pill on context_usage event and cycles models on click", async () => {
    await act(async () => {
      root = create(
        <IdeStatusBar
          projectDir="/repo"
          onOpenPanel={() => {}}
          onOpenExtensions={() => {}}
          onOpenRemote={() => {}}
        />,
      );
    });

    const modelBtn = root!.root.findByProps({ "data-testid": "statusbar-model-switcher" });
    expect(modelBtn.children.join("")).toContain("Default (Claude)");

    await act(async () => {
      modelBtn.props.onClick();
    });
    expect(modelBtn.children.join("")).toContain("Claude Sonnet");

    await act(async () => {
      chatListener?.({
        channel: "agent_event",
        payload: {
          type: "context_usage",
          cumulative_input_tokens: 10_000,
          cumulative_output_tokens: 2_000,
        },
      });
    });

    const usagePill = root!.root.findByProps({ "data-testid": "statusbar-token-cost-pill" });
    expect(usagePill.children.join("")).toContain("12.0k");
  });

  it.each([false, true])(
    "status bar [扩展] opens the Extensions sidebar, not the bottom panel (host running=%s)",
    async (running) => {
      extUiState.running = running;
      extServiceMock.start.mockClear();
      const onOpenPanel = vi.fn();
      const onOpenExtensions = vi.fn();
      await act(async () => {
        root = create(
          <IdeStatusBar
            projectDir="/repo"
            onOpenPanel={onOpenPanel}
            onOpenExtensions={onOpenExtensions}
            onOpenRemote={() => {}}
          />,
        );
      });

      const extBtn = root!.root.findByProps({ "data-testid": "statusbar-extensions" });
      await act(async () => {
        extBtn.props.onClick();
      });

      // Regression: running used to route to onOpenPanel("output"), which the
      // bottom panel falls back to the terminal when no output channel exists.
      expect(onOpenExtensions).toHaveBeenCalledTimes(1);
      expect(onOpenPanel).not.toHaveBeenCalled();
      expect(extServiceMock.start).not.toHaveBeenCalled();
    },
  );
});
