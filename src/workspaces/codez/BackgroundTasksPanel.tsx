import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getBackgroundOutput,
  type BackgroundTask,
} from "../../services/tauri/backgroundTasks";
import { redirectTargets } from "./bgRedirect";
import { openBgTerminal } from "../../services/bgTerminal";
import "./AssistantPanel.css";

function formatElapsed(ms: number): string {
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  return `${Math.floor(mins / 60)}h ${mins % 60}m`;
}

function statusLabel(
  t: ReturnType<typeof useTranslation>["t"],
  task: BackgroundTask,
): string {
  if (task.status === "running") {
    return task.ready ? t("chat.bgReady") : t("chat.bgRunning");
  }
  if (task.status === "exited") {
    return task.exit_code === 0
      ? t("chat.bgExitedOk")
      : t("chat.bgExitedFail", { code: task.exit_code ?? "?" });
  }
  if (task.status === "killed") return t("chat.bgKilled");
  if (task.status === "failed") return t("chat.bgFailed");
  return task.status;
}

/**
 * Background tasks (builds, tests, dev servers) registered for this
 * project, from any mode. The current session's tasks come first; tasks
 * owned by other sessions stay visible so nothing runs invisibly.
 */
export function backgroundSourceLabel(
  t: ReturnType<typeof useTranslation>["t"],
  source: string | null | undefined,
): string {
  switch (source) {
    case "codez":
    case "agentz":
      return t("chat.bgSourceCodez");
    case "workz":
      return t("chat.bgSourceWorkz");
    case "workz-team":
      return t("chat.bgSourceWorkzTeam");
    case "pool":
    case "piscis-heartbeat-pool":
      return t("chat.bgSourcePool");
    default:
      return source?.trim() ? source : t("chat.bgSourceUnknown");
  }
}

function sourceFirst(source: string | null | undefined, preferred: "codez" | "workz"): boolean {
  if (preferred === "workz") return source === "workz" || source === "workz-team";
  return source === "codez" || source === "agentz";
}

export default function BackgroundTasksPanel({
  tasks,
  currentSessionId,
  preferredSource = "codez",
  onKill,
  onOpenSession,
  onOpenFile,
}: {
  tasks: BackgroundTask[];
  currentSessionId: string | null;
  /** Tasks started in this mode are listed first. */
  preferredSource?: "codez" | "workz";
  onKill: (bgId: string) => void;
  onOpenSession?: (task: BackgroundTask) => void;
  onOpenFile?: (path: string) => void;
}) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<string | null>(null);
  const [output, setOutput] = useState<string>("");
  const [redirects, setRedirects] = useState<string[]>([]);

  if (tasks.length === 0) return null;

  const sorted = tasks.slice().sort((a, b) => {
    const aHere = sourceFirst(a.session_source, preferredSource) ? 0 : 1;
    const bHere = sourceFirst(b.session_source, preferredSource) ? 0 : 1;
    if (aHere !== bHere) return aHere - bHere;
    const aMine = a.session_id === currentSessionId ? 0 : 1;
    const bMine = b.session_id === currentSessionId ? 0 : 1;
    if (aMine !== bMine) return aMine - bMine;
    const aRun = a.status === "running" ? 0 : 1;
    const bRun = b.status === "running" ? 0 : 1;
    if (aRun !== bRun) return aRun - bRun;
    return b.started_unix_ms - a.started_unix_ms;
  });
  const runningCount = tasks.filter((x) => x.status === "running").length;

  const toggleOutput = async (task: BackgroundTask) => {
    if (expanded === task.id) {
      setExpanded(null);
      return;
    }
    setExpanded(task.id);
    setOutput("");
    setRedirects([]);
    try {
      const res = await getBackgroundOutput(task.id);
      const text = [res.stdout, res.stderr].filter(Boolean).join("\n");
      setOutput(text || t("chat.bgNoOutput"));
      setRedirects(text ? [] : redirectTargets(task.label, task.cwd || task.workspace_root));
    } catch (e) {
      setOutput(String(e));
    }
  };

  return (
    <div className="agentz-bg-panel">
      <div className="agentz-bg-head">
        <span>
          {t("chat.bgTitle")}
          {runningCount > 0 && (
            <span className="agentz-bg-count">{runningCount}</span>
          )}
        </span>
      </div>
      {sorted.map((task) => (
          <div key={task.id} className="agentz-bg-row">
            <div className="agentz-bg-main">
              <span
                className={`agentz-session-dot ${
                  task.status === "running"
                    ? "running"
                    : task.status === "exited" && task.exit_code === 0
                      ? "idle"
                      : task.status === "exited" || task.status === "failed"
                        ? "error"
                        : ""
                }`}
              />
              <span className="agentz-bg-source">{backgroundSourceLabel(t, task.session_source)}</span>
              <span className="agentz-bg-label" title={task.label}>
                {task.label}
              </span>
              <span className="agentz-bg-meta">
                {statusLabel(t, task)} · {formatElapsed(task.elapsed_ms)}
                {task.session_id !== currentSessionId && (
                  <>
                    {" · "}
                    <button
                      type="button"
                      className="agentz-bg-link"
                      onClick={() => onOpenSession?.(task)}
                    >
                      {t("chat.bgOtherSession")}
                    </button>
                  </>
                )}
              </span>
              <button
                type="button"
                className="agentz-bg-action"
                onClick={() => void toggleOutput(task)}
              >
                {expanded === task.id ? t("chat.bgHide") : t("chat.bgOutput")}
              </button>
              <button
                type="button"
                className="agentz-bg-action"
                onClick={() => openBgTerminal(task.id)}
              >
                {t("chat.bgTerminal")}
              </button>
              {task.status === "running" && (
                <button
                  type="button"
                  className="agentz-bg-action danger"
                  onClick={() => onKill(task.id)}
                >
                  {t("chat.bgKill")}
                </button>
              )}
            </div>
            {expanded === task.id && (
              <pre className="agentz-bg-output">{output}</pre>
            )}
            {expanded === task.id && redirects.length > 0 && (
              <div className="agentz-bg-redirect">
                {t("chat.bgRedirected")}
                {redirects.map((p) => (
                  <button
                    key={p}
                    type="button"
                    className="agentz-bg-link"
                    title={p}
                    disabled={!onOpenFile}
                    onClick={() => onOpenFile?.(p)}
                  >
                    {p.split(/[\\/]/).pop()}
                  </button>
                ))}
              </div>
            )}
          </div>
      ))}
    </div>
  );
}
