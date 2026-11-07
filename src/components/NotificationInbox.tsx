import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { createPortal } from "react-dom";
import { getProjectScope, useProjectScope } from "../services/tauri/projectScope";
import { clearNotifications, listNotifications, markNotificationRead, type InboxPage, type NoticeKind, type TaskNotice } from "../services/tauri/notificationInbox";
import { confirmDialog } from "../services/tauri/confirm";
import { pollReminders } from "../services/tauri/automationReminder";
import { coalesce, FOCUS_REFRESH_GAP_MS } from "../utils/coalesce";
import { canPortalToBody, useFixedPopupPosition } from "../utils/popupPosition";
import "./NotificationInbox.css";

interface Props {
  projectDir: string | null;
  onOpenTask: (taskId: string) => void;
}

export default function NotificationInbox({ projectDir, onOpenTask }: Props) {
  const { t } = useTranslation();
  const scope = useProjectScope(projectDir);
  const projectRef = useRef(projectDir);
  projectRef.current = projectDir;
  const scopeRef = useRef(scope?.handle);
  scopeRef.current = scope?.handle;
  const [open, setOpen] = useState(false);
  const [page, setPage] = useState<InboxPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverStyle = useFixedPopupPosition(open, rootRef, popoverRef, "down", "text", {
    align: "right",
    minWidth: 300,
    maxWidth: 420,
    maxHeight: 420,
  });
  useEffect(() => { setOpen(false); setPage(null); setError(null); }, [projectDir]);
  useEffect(() => { if (!scope) { setOpen(false); setPage(null); } }, [scope]);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!rootRef.current?.contains(target) && !popoverRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);
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
      await markNotificationRead(scope.handle, notice.event_key, notice.task_id);
      if (projectRef.current === projectDir && scopeRef.current === scope.handle) await refresh();
    } catch (cause) {
      if (projectRef.current === projectDir && scopeRef.current === scope.handle) setError(String(cause));
    }
  }

  async function clearAll() {
    if (!projectDir || !scope) return;
    if (!(await confirmDialog(t("notifications.clearConfirm"), { title: t("notifications.clear") }))) return;
    setBusy(true);
    try {
      await clearNotifications(scope.handle);
      if (projectRef.current === projectDir && scopeRef.current === scope.handle) { setPage({ notices: [], unread_count: 0 }); setError(null); }
    } catch (cause) {
      if (projectRef.current === projectDir && scopeRef.current === scope.handle) setError(String(cause));
    } finally { setBusy(false); }
  }

  if (!projectDir) return null;
  const popoverNode = open ? (
    <div
      ref={popoverRef}
      className="agentz-notification-popover is-fixed"
      role="region"
      aria-label={t("notifications.title")}
      style={popoverStyle}
    >
      <div className="agentz-notification-heading"><strong>{t("notifications.title")}</strong><span className="agentz-notification-actions"><button type="button" onClick={() => void clearAll()} disabled={busy || !page?.notices.length}>{t("notifications.clear")}</button><button type="button" onClick={() => void refresh()} disabled={busy}>{t("notifications.refresh")}</button></span></div>
      {error && <p role="alert">{error}</p>}
      {busy && !page && <p>{t("notifications.loading")}</p>}
      {page && page.notices.length === 0 && <p>{t("notifications.empty")}</p>}
      {page?.notices.map((item) => <NoticeRow key={item.event_key} item={item} onOpen={() => void openTask(item)} />)}
    </div>
  ) : null;
  return <div ref={rootRef} className="agentz-notification-inbox">
    <button type="button" className="agentz-titlebar-icon" aria-label={t("notifications.title")} title={t("notifications.title")} aria-expanded={open}
      onClick={() => { setOpen((wasOpen) => !wasOpen); if (!open) void refresh(); }}>
      <span aria-hidden="true">✉</span>{page && page.unread_count > 0 && <span className="agentz-notification-count">{page.unread_count}</span>}
    </button>
    {popoverNode && (canPortalToBody() ? createPortal(popoverNode, document.body) : popoverNode)}
  </div>;
}

const HIDDEN_STATUS = new Set(["Task completed", "Task failed"]);

function NoticeRow({ item, onOpen }: { item: TaskNotice; onOpen: () => void }) {
  const title = item.session_title?.trim() || item.summary;
  const reply = item.last_reply?.trim() ?? "";
  const status = item.session_title && item.summary !== title && !HIDDEN_STATUS.has(item.summary) ? item.summary : "";
  const unread = item.read_ms === null;
  return (
    <button type="button" className="agentz-notification-row" onClick={onOpen}>
      <span className="agentz-notification-main">
        <NoticeIcon kind={item.kind} />
        <span className="agentz-notification-copy">
          <span className={`agentz-notification-title${unread ? " agentz-notification-unread" : ""}`} title={title}>{title}</span>
          {status && <span className="agentz-notification-status" title={status}>{status}</span>}
          {reply && <span className="agentz-notification-reply" title={reply}>{reply}</span>}
          <small>{new Date(item.created_ms).toLocaleString()}</small>
        </span>
      </span>
    </button>
  );
}

function NoticeIcon({ kind }: { kind: NoticeKind }) {
  const tone = kind === "completed" ? "ok" : kind === "failed" ? "bad" : kind === "action_required" ? "warn" : "info";
  const label = kind === "completed" ? "Completed" : kind === "failed" ? "Failed" : kind === "action_required" ? "Needs attention" : "Update";
  return (
    <span className={`agentz-notification-icon ${tone}`}>
      {kind === "failed" ? <CrossIcon /> : kind === "completed" ? <CheckIcon /> : kind === "action_required" ? <AlertIcon /> : <DotIcon />}
      <span className="agentz-notification-sr">{label}</span>
    </span>
  );
}

function CheckIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="M4.7 8.2 7 10.4 11.4 5.6" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function CrossIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="M5.4 5.4 10.6 10.6M10.6 5.4 5.4 10.6" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function AlertIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="M8 4.6v4.2M8 11.2v.2" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}

function DotIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="3" fill="currentColor" />
    </svg>
  );
}
