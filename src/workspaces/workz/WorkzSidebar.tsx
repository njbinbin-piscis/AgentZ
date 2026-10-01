import { useTranslation } from "react-i18next";
import type { SessionMeta } from "../../services/tauri/chat";
import type { WorkzOverview } from "../../services/tauri/workz";
import { taskDisplayTitle } from "./taskTitle";

interface Props {
  overview: WorkzOverview | null;
  /** Directory key of the active scope (project path, or the free-session dir). */
  activeDir: string | null;
  /** Live session list of the active scope (fresher than `groups`). */
  activeTasks: SessionMeta[];
  groups: Record<string, SessionMeta[]>;
  groupErrors: Record<string, string>;
  collapsed: Record<string, boolean>;
  selectedId: string | null;
  runningIds: string[];
  onToggle: (dir: string) => void;
  onOpenSession: (dir: string, id: string) => void;
  onNewSession: (dir: string) => void;
  onDeleteSession: (dir: string, id: string) => void;
  onAddProject: () => void;
  onRemoveProject: (dir: string) => void;
  onOpenInCodeZ?: (dir: string) => void;
}

export default function WorkzSidebar({
  overview,
  activeDir,
  activeTasks,
  groups,
  groupErrors,
  collapsed,
  selectedId,
  runningIds,
  onToggle,
  onOpenSession,
  onNewSession,
  onDeleteSession,
  onAddProject,
  onRemoveProject,
  onOpenInCodeZ,
}: Props) {
  const { t } = useTranslation();
  const freeDir = overview?.free_dir ?? null;

  const sessionsOf = (dir: string): SessionMeta[] =>
    dir === activeDir ? activeTasks : (groups[dir] ?? []);

  const renderSessions = (dir: string) => {
    const list = sessionsOf(dir);
    if (list.length === 0) {
      return <div className="agentz-workz-tasks-empty">{t("agent.noTasks")}</div>;
    }
    return list.map((task) => (
      <div
        key={task.id}
        className={`agentz-workz-task ${task.id === selectedId && dir === activeDir ? "active" : ""} ${runningIds.includes(task.id) ? "running" : ""}`}
        onClick={() => onOpenSession(dir, task.id)}
      >
        <span
          className={`agentz-workz-task-dot ${runningIds.includes(task.id) ? "running" : task.status}`}
        />
        <span className="agentz-workz-task-title">
          {taskDisplayTitle(task.title, t("agent.untitled"))}
        </span>
        <span className="agentz-workz-task-count">{task.message_count}</span>
        <button
          className="agentz-workz-task-del"
          title={t("agent.deleteTask")}
          onClick={(e) => {
            e.stopPropagation();
            onDeleteSession(dir, task.id);
          }}
        >
          ×
        </button>
      </div>
    ));
  };

  const groupRunning = (dir: string) =>
    sessionsOf(dir).some((s) => runningIds.includes(s.id));

  return (
    <>
      <div className="agentz-workz-sidebar-head">
        <span>{t("agent.tasks")}</span>
        <button
          onClick={() => freeDir && onNewSession(freeDir)}
          disabled={!freeDir}
          title={t("workz.newSessionHint")}
        >
          ＋ {t("agent.new")}
        </button>
      </div>
      <div className="agentz-workz-tasklist">
        <div className="agentz-workz-group-head">
          <span>{t("workz.projects")}</span>
          <button onClick={onAddProject} title={t("workz.addProjectHint")}>
            ＋ {t("workz.addProject")}
          </button>
        </div>
        {overview && overview.projects.length === 0 && (
          <div className="agentz-workz-tasks-empty">{t("workz.noProjects")}</div>
        )}
        {overview?.projects.map((p) => {
          const isCollapsed = collapsed[p.path] ?? false;
          return (
            <div key={p.path} className="agentz-workz-group">
              <div
                className={`agentz-workz-group-row${p.exists ? "" : " missing"}${p.path === activeDir ? " active" : ""}`}
                onClick={() => onToggle(p.path)}
                title={p.exists ? p.path : t("workz.projectMissing")}
              >
                <span className="agentz-workz-group-caret">{isCollapsed ? "▸" : "▾"}</span>
                <span className="agentz-workz-group-icon">{p.kind === "repo" ? "⎇" : "▤"}</span>
                <span className="agentz-workz-group-name">{p.name}</span>
                {groupRunning(p.path) && <span className="agentz-workz-task-dot running" />}
                {p.exists && onOpenInCodeZ && (
                  <button
                    className="agentz-workz-group-btn"
                    title={t("workz.openInCodeZ")}
                    onClick={(e) => {
                      e.stopPropagation();
                      onOpenInCodeZ(p.path);
                    }}
                  >
                    ⌨
                  </button>
                )}
                {p.exists && (
                  <button
                    className="agentz-workz-group-btn"
                    title={t("workz.newInProject")}
                    onClick={(e) => {
                      e.stopPropagation();
                      onNewSession(p.path);
                    }}
                  >
                    ＋
                  </button>
                )}
                <button
                  className="agentz-workz-group-btn"
                  title={t("workz.removeProject")}
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemoveProject(p.path);
                  }}
                >
                  ×
                </button>
              </div>
              {!isCollapsed && (
                <div className="agentz-workz-group-body">
                  {!p.exists ? (
                    <div className="agentz-workz-tasks-empty">{t("workz.projectMissing")}</div>
                  ) : groupErrors[p.path] ? (
                    <div className="agentz-workz-tasks-empty">{groupErrors[p.path]}</div>
                  ) : (
                    renderSessions(p.path)
                  )}
                </div>
              )}
            </div>
          );
        })}

        {freeDir && (
          <div className="agentz-workz-group">
            <div
              className={`agentz-workz-group-row${freeDir === activeDir ? " active" : ""}`}
              onClick={() => onToggle(freeDir)}
            >
              <span className="agentz-workz-group-caret">
                {(collapsed[freeDir] ?? false) ? "▸" : "▾"}
              </span>
              <span className="agentz-workz-group-icon">◌</span>
              <span className="agentz-workz-group-name">{t("workz.freeSessions")}</span>
              {groupRunning(freeDir) && <span className="agentz-workz-task-dot running" />}
              <button
                className="agentz-workz-group-btn"
                title={t("workz.newSessionHint")}
                onClick={(e) => {
                  e.stopPropagation();
                  onNewSession(freeDir);
                }}
              >
                ＋
              </button>
            </div>
            {!(collapsed[freeDir] ?? false) && (
              <div className="agentz-workz-group-body">{renderSessions(freeDir)}</div>
            )}
          </div>
        )}
      </div>
    </>
  );
}
