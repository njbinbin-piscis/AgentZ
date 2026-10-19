import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import WorkzEmptyState, { WORKZ_STARTER_SCENARIOS } from "./WorkzEmptyState";

const tFn = (key: string) => key;
const i18nObj = { language: "zh-CN" };
const translationHook = { t: tFn, i18n: i18nObj };

vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

describe("WorkzEmptyState (R10-12)", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("renders all 4 starter scenario cards and triggers onSelectScenario when clicked", async () => {
    const onSelect = vi.fn();

    await act(async () => {
      root = create(
        <WorkzEmptyState
          title="AgentZ WorkZ"
          subtitle="Choose a task"
          onSelectScenario={onSelect}
        />,
      );
    });

    expect(WORKZ_STARTER_SCENARIOS).toHaveLength(4);
    for (const sc of WORKZ_STARTER_SCENARIOS) {
      const btn = root!.root.findByProps({ "data-testid": `workz-starter-${sc.id}` });
      expect(btn).toBeTruthy();
    }

    const codeCard = root!.root.findByProps({ "data-testid": "workz-starter-code" });
    await act(async () => {
      codeCard.props.onClick();
    });

    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect.mock.calls[0][0].id).toBe("code");
    expect(onSelect.mock.calls[0][0].recommendIsolate).toBe(true);
  });

  it("styles starter cards with theme-aware CSS tokens for both dark and light themes", async () => {
    const fsSpecifier = "node:fs";
    const urlSpecifier = "node:url";
    const fs = (await import(/* @vite-ignore */ fsSpecifier)) as {
      readFileSync: (path: string, encoding: string) => string;
    };
    const url = (await import(/* @vite-ignore */ urlSpecifier)) as {
      fileURLToPath: (u: URL) => string;
    };
    const agentCss = fs.readFileSync(
      url.fileURLToPath(new URL("./Agent.css", import.meta.url)),
      "utf8",
    );
    const indexCss = fs.readFileSync(
      url.fileURLToPath(new URL("../../index.css", import.meta.url)),
      "utf8",
    );

    expect(agentCss).toContain(".agentz-workz-starter-card");
    expect(agentCss).not.toContain("#181822");
    expect(agentCss).toMatch(/\.agentz-workz-starter-card\s*\{[\s\S]*?background:\s*var\(--bg-elev\)/);
    expect(indexCss).toMatch(/\[data-theme="light"\][\s\S]*?--bg-elev:\s*#ffffff/);
    expect(indexCss).toMatch(/\[data-theme="light"\][\s\S]*?--bg-panel:\s*#ffffff/);
  });
});
