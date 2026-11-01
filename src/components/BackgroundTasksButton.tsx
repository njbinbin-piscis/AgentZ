import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { useSyncExternalStore } from "react";
import { getActiveChatSession, subscribeActiveChatSession } from "../services/activeChatSession";
import { useBackgroundTasks } from "../hooks/useBackgroundTasks";
import type { BackgroundTask } from "../services/tauri/backgroundTasks";
import BackgroundTasksPanel from "../workspaces/codez/BackgroundTasksPanel";
import { requestOpenCodezSession } from "../workspaces/codez/sessionRoute";
import { canPortalToBody, useFixedPopupPosition } from "../utils/popupPosition";

export const OPEN_WORKZ_TASK_EVENT = "agentz:open-workz-task";
export const SHOW_MODE_EVENT = "agentz:show-mode";

/** Open the session that owns a background task, in the mode that created it. */
export function openBackgroundTaskSession(projectDir: string, task: BackgroundTask): void {
  const source = task.session_source ?? "";
  if (source === "workz" || source === "workz-team") {
    window.dispatchEvent(
      new CustomEvent(OPEN_WORKZ_TASK_EVENT, {
        detail: { projectDir, taskId: task.session_id, nonce: Date.now() },
      }),
    );
    window.dispatchEvent(new CustomEvent(SHOW_MODE_EVENT, { detail: "workz" }));
    return;
  }
  requestOpenCodezSession({
    projectDir,
    sessionId: task.session_id,
    source: source || null,
  });
  window.dispatchEvent(new CustomEvent(SHOW_MODE_EVENT, { detail: "codez" }));
}

/**
 * Project-wide background task list. CodeZ puts it on the status bar;
 * WorkZ puts the same control on the task bar. The popover opens upward.
 */
export default function BackgroundTasksButton({
  projectDir,
  className,
  preferredSource,
  currentSessionId,
  onOpenFile,
}: {
  projectDir: string | null;
  className?: string;
  preferredSource: "codez" | "workz";
  /** Overrides the CodeZ active-session store. WorkZ passes its open task. */
  currentSessionId?: string | null;
  onOpenFile?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const bg = useBackgroundTasks(projectDir);
  const codezSessionId = useSyncExternalStore(subscribeActiveChatSession, getActiveChatSession);
  const sessionId = currentSessionId === undefined ? codezSessionId : currentSessionId;
  const [open, setOpen] = useState(false);
  const anchorRef = useRef<HTMLButtonElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverStyle = useFixedPopupPosition(open, anchorRef, popoverRef, "up", "text", {
    align: preferredSource === "workz" ? "right" : "left",
    minWidth: 360,
    maxWidth: 560,
    maxHeight: 360,
  });

  useEffect(() => {
    if (bg.tasks.length === 0) setOpen(false);
  }, [bg.tasks.length]);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const target = e.target as Node;
      if (!anchorRef.current?.contains(target) && !popoverRef.current?.contains(target)) {
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

  if (bg.tasks.length === 0) return null;

  return (
    <>
      <button
        type="button"
        ref={anchorRef}
        className={className}
        title={bg.running.map((task) => task.label).join("\n") || t("chat.bgTitle")}
        onClick={() => setOpen((current) => !current)}
      >
        <span className={`agentz-session-dot${bg.running.length > 0 ? " running" : ""}`} />{" "}
        {bg.running.length > 0
          ? t("chat.bgRunningCount", { count: bg.running.length })
          : t("chat.bgRecentCount", { count: bg.tasks.length })}
      </button>
      {open &&
        canPortalToBody() &&
        createPortal(
          <div ref={popoverRef} className="agentz-bg-popover" style={popoverStyle}>
            <BackgroundTasksPanel
              tasks={bg.tasks}
              currentSessionId={sessionId}
              preferredSource={preferredSource}
              onKill={(id) => void bg.kill(id)}
              onOpenFile={onOpenFile}
              onOpenSession={(task) => {
                if (projectDir) openBackgroundTaskSession(projectDir, task);
                setOpen(false);
              }}
            />
          </div>,
          document.body,
        )}
    </>
  );
}
