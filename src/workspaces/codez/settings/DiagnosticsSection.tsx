import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  exportDiagnosticLogs,
  previewDiagnosticLogs,
  type DiagnosticExportPreview,
} from "../../../services/tauri/diagnostics";

export default function DiagnosticsSection() {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<DiagnosticExportPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [exportPath, setExportPath] = useState("");

  async function loadPreview() {
    setBusy(true);
    setError("");
    setPreview(null);
    setExportPath("");
    try {
      setPreview(await previewDiagnosticLogs());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function exportLogs() {
    setBusy(true);
    setError("");
    setExportPath("");
    try {
      setExportPath(await exportDiagnosticLogs());
    } catch (e) {
      setError(String(e));
      setPreview(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="agentz-settings-section" aria-label={t("settings.diagnosticsTitle")}>
      <h3>{t("settings.diagnosticsTitle")}</h3>
      <p className="agentz-settings-hint">{t("settings.diagnosticsDescription")}</p>
      <button
        type="button"
        className="agentz-settings-cancel"
        data-testid="diagnostics-preview-btn"
        disabled={busy}
        onClick={() => void loadPreview()}
      >
        {t("settings.diagnosticsPreview")}
      </button>
      {preview && (
        <div className="agentz-diagnostics-card" data-testid="diagnostics-preview-card">
          <p>
            {t("settings.diagnosticsScope", {
              version: preview.app_version,
              files: preview.files.length,
              bytes: preview.total_bytes,
            })}
          </p>
          {preview.context_summary && (
            <p data-testid="diagnostics-context-summary">
              {t("settings.diagnosticsContextSummary", {
                taskId: preview.context_summary.latest_task_id || "-",
                envId: preview.context_summary.latest_environment_id || "-",
                runId: preview.context_summary.latest_run_id || "-",
                failures: preview.context_summary.failure_count ?? 0,
              })}
            </p>
          )}
          {preview.background_tasks && preview.background_tasks.total > 0 && (
            <p data-testid="diagnostics-background-summary">
              {t("settings.diagnosticsBackground", {
                running: preview.background_tasks.running,
                total: preview.background_tasks.total,
              })}
            </p>
          )}
          <ul className="agentz-diagnostics-files">
            {preview.files.map((file) => (
              <li key={file.name}>
                {file.name} ({file.bytes} B)
              </li>
            ))}
          </ul>
          <button
            type="button"
            className="agentz-settings-save"
            data-testid="diagnostics-export-btn"
            disabled={busy || !preview.redaction_ready}
            onClick={() => void exportLogs()}
          >
            {t("settings.diagnosticsExport")}
          </button>
        </div>
      )}
      {error && (
        <p className="agentz-diagnostics-error" role="alert">
          {error}
        </p>
      )}
      {exportPath && (
        <p className="agentz-settings-hint" data-testid="diagnostics-saved-path">
          {t("settings.diagnosticsSaved", { path: exportPath })}
        </p>
      )}
    </section>
  );
}
