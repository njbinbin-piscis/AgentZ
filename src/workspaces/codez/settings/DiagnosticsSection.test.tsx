import { act, create, type ReactTestRenderer } from "react-test-renderer";
import { expect, it, vi } from "vitest";

const { previewDiagnosticLogs, exportDiagnosticLogs } = vi.hoisted(() => ({
  previewDiagnosticLogs: vi.fn(), exportDiagnosticLogs: vi.fn(),
}));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("../../../services/tauri/diagnostics", () => ({ previewDiagnosticLogs, exportDiagnosticLogs }));
import DiagnosticsSection from "./DiagnosticsSection";

it("requires a successful scope preview before offering a local export", async () => {
  previewDiagnosticLogs.mockResolvedValue({ schema_version: 1, app_version: "0.9.7", files: [{ name: "agentz.log", bytes: 9 }], total_bytes: 9, redaction_ready: true });
  exportDiagnosticLogs.mockResolvedValue("C:/local/export.log");
  let renderer: ReactTestRenderer;
  act(() => { renderer = create(<DiagnosticsSection />); });
  expect(renderer!.root.findAllByType("button")).toHaveLength(1);
  await act(async () => { await renderer!.root.findAllByType("button")[0].props.onClick(); });
  expect(renderer!.root.findAllByType("button")).toHaveLength(2);
  expect(JSON.stringify(renderer!.toJSON())).toContain("agentz.log");
  await act(async () => { await renderer!.root.findAllByType("button")[1].props.onClick(); });
  expect(exportDiagnosticLogs).toHaveBeenCalledOnce();
  expect(JSON.stringify(renderer!.toJSON())).toContain("settings.diagnosticsSaved");
  act(() => renderer!.unmount());
});
