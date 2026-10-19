import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getProjectScope, useProjectScope } from "../services/tauri/projectScope";
import { disableReminder, reminderStatus, setReminder, type ReminderStatus } from "../services/tauri/automationReminder";
import "./TaskReminderPanel.css";

interface Props { projectDir: string; taskId: string; chinese?: boolean }

export default function TaskReminderPanel({ projectDir, taskId, chinese }: Props) {
  const { t, i18n } = useTranslation();
  const lng = chinese !== undefined ? (chinese ? "zh" : "en") : i18n.language;
  const tr = (key: string) => t(key, { lng });
  const scope = useProjectScope(projectDir);
  const scopeRef = useRef(scope?.handle);
  scopeRef.current = scope?.handle;
  const [status, setStatus] = useState<ReminderStatus | null>(null);
  const [interval, setIntervalMinutes] = useState(60);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    setStatus(null);
    setError(null);
    if (!scope) return;
    void reminderStatus(scope.handle, taskId).then((value) => {
      if (!cancelled) { setStatus(value); if (value) setIntervalMinutes(value.interval_minutes); }
    }).catch((cause) => { if (!cancelled) setError(String(cause)); });
    return () => { cancelled = true; };
  }, [scope, taskId]);

  async function save() {
    if (!scope || getProjectScope(projectDir)?.handle !== scope.handle) { setError(tr("taskReminder.unavailable")); return; }
    setBusy(true); setError(null);
    try { const next = await setReminder(scope.handle, taskId, interval); if (scopeRef.current === scope.handle) setStatus(next); }
    catch (cause) { if (scopeRef.current === scope.handle) setError(String(cause)); }
    finally { setBusy(false); }
  }

  async function stop() {
    if (!scope || getProjectScope(projectDir)?.handle !== scope.handle) { setError(tr("taskReminder.unavailable")); return; }
    setBusy(true); setError(null);
    try { await disableReminder(scope.handle, taskId); if (scopeRef.current === scope.handle) setStatus(null); }
    catch (cause) { if (scopeRef.current === scope.handle) setError(String(cause)); }
    finally { setBusy(false); }
  }

  return <div className="agentz-task-reminder">
    <strong>{tr("taskReminder.title")}</strong>
    <span>{tr("taskReminder.hint")}</span>
    <select aria-label={tr("taskReminder.intervalLabel")} value={interval}
      onChange={(event) => setIntervalMinutes(Number(event.target.value))} disabled={busy}>
      <option value={60}>{tr("taskReminder.everyHour")}</option>
      <option value={1440}>{tr("taskReminder.everyDay")}</option>
      <option value={10080}>{tr("taskReminder.everyWeek")}</option>
    </select>
    <button type="button" onClick={() => void save()} disabled={busy}>{tr("taskReminder.setBtn")}</button>
    {status && <>
      <span>{tr("taskReminder.nextDue")}{new Date(status.next_due_ms).toLocaleString()}</span>
      <button type="button" onClick={() => void stop()} disabled={busy}>{tr("taskReminder.disableBtn")}</button>
    </>}
    {error && <span role="alert">{error}</span>}
  </div>;
}
