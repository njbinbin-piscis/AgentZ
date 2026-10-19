import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import IdeStatusBar, { estimateTokenCostUsd, formatCompactTokens } from "./IdeStatusBar";

const tFn = (key: string) => key;
const translationHook = { t: tFn };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

vi.mock("../../extensions/ui/useExtensionUi", () => ({
  useExtensionUi: () => ({
    statusBar: [],
    running: false,
    scm: [],
    hostError: null,
  }),
}));

vi.mock("../../extensions/remotePorts", () => ({
  useRemotePortWatch: () => [],
}));

vi.mock("../../extensions/extensionService", () => ({
  extensionService: {
    start: vi.fn(),
    executeCommand: vi.fn(),
    isRunning: false,
  },
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
      { id: "claude-sonnet", name: "Claude Sonnet", provider: "anthropic", model: "claude-3-7" },
      { id: "gpt-4o", name: "GPT-4o", provider: "openai", model: "gpt-4o" },
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
});
