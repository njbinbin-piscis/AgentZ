import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { afterEach, describe, expect, it, vi } from "vitest";
import DiagnosticsSection from "./DiagnosticsSection";
import { compatStore } from "../../../extensions/compatStore";
import { HOST_VSCODE_VERSION, MainContext, type ExtensionCompatDto } from "../../../extensions/common/protocol";
import { satisfies } from "../../../extensions/semver";

const previewMock = vi.fn().mockResolvedValue({
  schema_version: 1,
  app_version: "1.0.0",
  files: [{ name: "agentz.log", bytes: 1024 }],
  total_bytes: 1024,
  redaction_ready: true,
  context_summary: {
    latest_run_id: "run-901",
    latest_task_id: "task-42",
    latest_environment_id: "env-wt-7",
    failure_count: 2,
  },
});

const exportMock = vi.fn().mockResolvedValue("/config/exports/agentz-diagnostics-1700000000.log");

vi.mock("../../../services/tauri/diagnostics", () => ({
  previewDiagnosticLogs: (...args: unknown[]) => previewMock(...args),
  exportDiagnosticLogs: (...args: unknown[]) => exportMock(...args),
}));

const tFn = (key: string, opts?: Record<string, unknown>) =>
  opts ? `${key}:${JSON.stringify(opts)}` : key;
const translationHook = { t: tFn };
vi.mock("react-i18next", () => ({
  useTranslation: () => translationHook,
}));

describe("DiagnosticsSection & Top 8 VS Code Extension Compatibility (R10-23)", () => {
  let root: ReactTestRenderer | null = null;

  afterEach(async () => {
    compatStore.reset();
    if (root) {
      await act(async () => {
        root?.unmount();
      });
      root = null;
    }
  });

  it("previews bounded logs with active task/env/run context summary and exports redacted bundle", async () => {
    await act(async () => {
      root = create(<DiagnosticsSection />);
    });

    const previewBtn = root!.root.findByProps({ "data-testid": "diagnostics-preview-btn" });
    await act(async () => {
      await previewBtn.props.onClick();
    });

    expect(previewMock).toHaveBeenCalledTimes(1);
    const ctxNode = root!.root.findByProps({ "data-testid": "diagnostics-context-summary" });
    expect(JSON.stringify(ctxNode.children)).toContain("task-42");
    expect(JSON.stringify(ctxNode.children)).toContain("env-wt-7");
    expect(JSON.stringify(ctxNode.children)).toContain("run-901");

    const exportBtn = root!.root.findByProps({ "data-testid": "diagnostics-export-btn" });
    await act(async () => {
      await exportBtn.props.onClick();
    });
    expect(exportMock).toHaveBeenCalledTimes(1);

    const savedNode = root!.root.findByProps({ "data-testid": "diagnostics-saved-path" });
    expect(JSON.stringify(savedNode.children)).toContain("agentz-diagnostics-1700000000.log");
  });

  it("verifies Top 8 VS Code extension compatibility matrix against HOST_VSCODE_VERSION (1.96.0)", () => {
    const top8Catalog: Array<{
      id: string;
      displayName: string;
      version: string;
      enginesVscode: string;
      tier: "full" | "degraded";
      unsupportedProposals?: string[];
    }> = [
      {
        id: "esbenp.prettier-vscode",
        displayName: "Prettier - Code formatter",
        version: "11.0.0",
        enginesVscode: "^1.90.0",
        tier: "full",
      },
      {
        id: "dbaeumer.vscode-eslint",
        displayName: "ESLint",
        version: "3.0.10",
        enginesVscode: "^1.89.0",
        tier: "full",
      },
      {
        id: "yzhang.markdown-all-in-one",
        displayName: "Markdown All in One",
        version: "3.6.2",
        enginesVscode: "^1.74.0",
        tier: "full",
      },
      {
        id: "redhat.vscode-yaml",
        displayName: "YAML",
        version: "1.15.0",
        enginesVscode: "^1.82.0",
        tier: "full",
      },
      {
        id: "bradlc.vscode-tailwindcss",
        displayName: "Tailwind CSS IntelliSense",
        version: "0.14.0",
        enginesVscode: "^1.86.0",
        tier: "full",
      },
      {
        id: "ms-python.python",
        displayName: "Python",
        version: "2025.2.0",
        enginesVscode: "^1.94.0",
        tier: "degraded",
        unsupportedProposals: ["terminalShellIntegration"],
      },
      {
        id: "rust-lang.rust-analyzer",
        displayName: "rust-analyzer",
        version: "0.4.2100",
        enginesVscode: "^1.92.0",
        tier: "degraded",
      },
      {
        id: "eamodio.gitlens",
        displayName: "GitLens — Git supercharged",
        version: "16.0.0",
        enginesVscode: "^1.93.0",
        tier: "degraded",
        unsupportedProposals: ["scmHistoryProvider"],
      },
    ];

    const verdicts: ExtensionCompatDto[] = top8Catalog.map((ext) => {
      const compatible = satisfies(HOST_VSCODE_VERSION, ext.enginesVscode);
      return {
        id: ext.id,
        displayName: ext.displayName,
        version: ext.version,
        enginesVscode: ext.enginesVscode,
        compatible,
        unsupportedProposals: ext.unsupportedProposals,
        activated: compatible,
      };
    });

    compatStore.setReport({
      hostVersion: HOST_VSCODE_VERSION,
      extensions: verdicts,
    });
    compatStore.recordMissing(MainContext.MainThreadScm.nid, "$registerHistoryProvider");

    const snap = compatStore.getSnapshot();
    expect(snap.hostVersion).toBe("1.96.0");
    expect(snap.extensions).toHaveLength(8);
    expect(snap.extensions.every((e) => e.compatible)).toBe(true);
    expect(compatStore.incompatibleCount).toBe(0);
    expect(snap.missingApis).toContain("MainThreadScm.$registerHistoryProvider");
  });
});
