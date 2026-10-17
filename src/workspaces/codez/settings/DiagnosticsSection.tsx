import { useState } from "react";
import { useTranslation } from "react-i18next";
import { exportDiagnosticLogs, previewDiagnosticLogs, type DiagnosticExportPreview } from "../../../services/tauri/diagnostics";

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
      <p>{t("settings.diagnosticsDescription")}</p>
      <button type="button" disabled={busy} onClick={() => void loadPreview()}>{t("settings.diagnosticsPreview")}</button>
      {preview && (
        <div>
          <p>{t("settings.diagnosticsScope", { version: preview.app_version, files: preview.files.length, bytes: preview.total_bytes })}</p>
          <ul>{preview.files.map((file) => <li key={file.name}>{file.name} ({file.bytes} B)</li>)}</ul>
          <button type="button" disabled={busy || !preview.redaction_ready} onClick={() => void exportLogs()}>{t("settings.diagnosticsExport")}</button>
        </div>
      )}
      {error && <p role="alert">{error}</p>}
      {exportPath && <p>{t("settings.diagnosticsSaved", { path: exportPath })}</p>}
    </section>
  );
}
