import { useEffect, useRef, useState } from "react";
import { getProjectScope, useProjectScope } from "../services/tauri/projectScope";
import { listNotifications, markNotificationRead, type InboxPage, type TaskNotice } from "../services/tauri/notificationInbox";
import { pollReminders } from "../services/tauri/automationReminder";
import { coalesce, FOCUS_REFRESH_GAP_MS } from "../utils/coalesce";
import "./NotificationInbox.css";

interface Props {
  projectDir: string | null;
  onOpenTask: (taskId: string) => void;
}

export default function NotificationInbox({ projectDir, onOpenTask }: Props) {
  const scope = useProjectScope(projectDir);
  const projectRef = useRef(projectDir);
  projectRef.current = projectDir;
  const scopeRef = useRef(scope?.handle);
  scopeRef.current = scope?.handle;
  const [open, setOpen] = useState(false);
  const [page, setPage] = useState<InboxPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => { setOpen(false); setPage(null); setError(null); }, [projectDir]);
  useEffect(() => { if (!scope) { setOpen(false); setPage(null); } }, [scope]);
  useEffect(() => {
    if (!projectDir || !scope) return;
    const handle = scope.handle;
    let disposed = false;
    let inFlight = false;
    async function tick() {
      if (inFlight) return;
      inFlight = true;
      try {
        await pollReminders(handle);
        const next = await listNotifications(handle);
        if (!disposed && projectRef.current === projectDir && scopeRef.current === handle) setPage(next);
      } catch (cause) {
        if (!disposed && projectRef.current === projectDir && scopeRef.current === handle) setError(String(cause));
      } finally { inFlight = false; }
    }
    void tick();
    const timer = window.setInterval(() => void tick(), 60_000);
    const onFocus = coalesce(() => void tick(), FOCUS_REFRESH_GAP_MS);
    window.addEventListener("focus", onFocus.call);
    return () => {
      disposed = true;
      window.clearInterval(timer);
      onFocus.cancel();
      window.removeEventListener("focus", onFocus.call);
    };
  }, [projectDir, scope]);

  async function refresh() {
    if (!projectDir) return;
    if (!scope) { setError("Project access is unavailable"); return; }
    setBusy(true);
    try { const next = await listNotifications(scope.handle); if (projectRef.current === projectDir && scopeRef.current === scope.handle) { setPage(next); setError(null); } }
    catch (cause) { if (projectRef.current === projectDir && scopeRef.current === scope.handle) setError(String(cause)); }
    finally { setBusy(false); }
  }

  async function openTask(notice: TaskNotice) {
    if (!projectDir) return;
    if (projectRef.current !== projectDir) return;
    if (!scope || getProjectScope(projectDir)?.handle !== scope.handle) { setError("Project access is unavailable"); return; }
    // Reading a task must remain possible if persisting its read state fails.
    setOpen(false);
    onOpenTask(notice.task_id);
    try {
      await markNotificationRead(scope.handle, notice.event_key);
      if (projectRef.current === projectDir && scopeRef.current === scope.handle) await refresh();
    } catch (cause) {
      if (projectRef.current === projectDir && scopeRef.current === scope.handle) setError(String(cause));
    }
  }

  if (!projectDir) return null;
  return <div className="agentz-notification-inbox">
    <button type="button" className="agentz-titlebar-icon" aria-label="Notifications" title="Notifications" aria-expanded={open}
      onClick={() => { setOpen((wasOpen) => !wasOpen); if (!open) void refresh(); }}>
      <span aria-hidden="true">✉</span>{page && page.unread_count > 0 && <span className="agentz-notification-count">{page.unread_count}</span>}
    </button>
    {open && <div className="agentz-notification-popover" role="region" aria-label="Notifications">
      <div className="agentz-notification-heading"><strong>Notifications</strong><button type="button" onClick={() => void refresh()} disabled={busy}>Refresh</button></div>
      {error && <p role="alert">{error}</p>}
      {busy && !page && <p>Loading…</p>}
      {page && page.notices.length === 0 && <p>No notifications</p>}
      {page?.notices.map((item) => <button type="button" className="agentz-notification-row" key={item.event_key}
        onClick={() => void openTask(item)}>
        <span className={item.read_ms === null ? "agentz-notification-unread" : ""}>{item.summary}</span>
        <small>{new Date(item.created_ms).toLocaleString()}</small>
      </button>)}
    </div>}
  </div>;
}
