import { useEffect, useRef, useState } from "react";
import { getProjectScope, useProjectScope } from "../services/tauri/projectScope";
import { disableReminder, reminderStatus, setReminder, type ReminderStatus } from "../services/tauri/automationReminder";
import "./TaskReminderPanel.css";

interface Props { projectDir: string; taskId: string; chinese: boolean }

export default function TaskReminderPanel({ projectDir, taskId, chinese }: Props) {
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
    if (!scope || getProjectScope(projectDir)?.handle !== scope.handle) { setError(chinese ? "项目访问不可用" : "Project access is unavailable"); return; }
    setBusy(true); setError(null);
    try { const next = await setReminder(scope.handle, taskId, interval); if (scopeRef.current === scope.handle) setStatus(next); }
    catch (cause) { if (scopeRef.current === scope.handle) setError(String(cause)); }
    finally { setBusy(false); }
  }

  async function stop() {
    if (!scope || getProjectScope(projectDir)?.handle !== scope.handle) { setError(chinese ? "项目访问不可用" : "Project access is unavailable"); return; }
    setBusy(true); setError(null);
    try { await disableReminder(scope.handle, taskId); if (scopeRef.current === scope.handle) setStatus(null); }
    catch (cause) { if (scopeRef.current === scope.handle) setError(String(cause)); }
    finally { setBusy(false); }
  }

  return <div className="agentz-task-reminder">
    <strong>{chinese ? "任务提醒" : "Task reminder"}</strong>
    <span>{chinese ? " 打开项目时到期后显示提醒，不会自动运行任务。" : " Shows an inbox reminder when this project is open and due; does not run the task."}</span>
    <select aria-label={chinese ? "提醒间隔" : "Reminder interval"} value={interval}
      onChange={(event) => setIntervalMinutes(Number(event.target.value))} disabled={busy}>
      <option value={60}>{chinese ? "每小时" : "Every hour"}</option>
      <option value={1440}>{chinese ? "每天" : "Every day"}</option>
      <option value={10080}>{chinese ? "每周" : "Every week"}</option>
    </select>
    <button type="button" onClick={() => void save()} disabled={busy}>{chinese ? "设置提醒" : "Set reminder"}</button>
    {status && <>
      <span>{chinese ? "下次：" : "Next: "}{new Date(status.next_due_ms).toLocaleString()}</span>
      <button type="button" onClick={() => void stop()} disabled={busy}>{chinese ? "关闭提醒" : "Disable reminder"}</button>
    </>}
    {error && <span role="alert">{error}</span>}
  </div>;
}
