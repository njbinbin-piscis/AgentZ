import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  chatSend,
  chatCancel,
  onChatEvent,
  listSessions,
  SESSION_SOURCE_WORKZ,
  SESSION_SOURCE_WORKZ_TEAM,
  SESSION_SOURCES_PROJECT_SHARED,
  getMessages,
  deleteSession,
  journalListChanges,
  type AgentEvent,
  type ChatAttachment,
  type ChatEventEnvelope,
  type PlanTodoItem,
  type SessionMeta,
} from "../../services/tauri/chat";
import { useAppSettings, pruneModelId } from "../../hooks/useAppSettings";
import { useStickToBottom } from "../../hooks/useStickToBottom";
import { loadScopedModelId, saveScopedModelId } from "../../utils/modelPrefs";
import { useInputHistory } from "../../components/useInputHistory";
import { useAtMention } from "../../hooks/useAtMention";
import { useSlashCompletion } from "../../hooks/useSlashCompletion";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { agentTaskApi, type AgentTaskInfo } from "../../services/tauri/agentTask";
import {
  MISSING_TASK_ENVIRONMENT,
  resolveTaskWorktree,
} from "../../services/tauri/taskEnvironment";
import {
  generateDeepRepoWiki,
  generateRepoWiki,
  readRepoMemory,
} from "../../services/tauri/repoWiki";
import { requestGraphIndex } from "../../services/tauri/graphIndex";
import type { ComposerMenuOption } from "../../components/ChatComposer";
import type { DropdownOption } from "../../components/DropdownSelect";
import {
  modelLabel,
  modelDisplayLabel,
  pickChatAttachment,
  attachmentFromPath,
  blobToDataUrl,
  dataUrlToBase64,
} from "../../components/chatComposerUtils";
import { visionCapable } from "../../components/visionUtils";
import { queryTaskPlan, type DurablePlan } from "../../services/tauri/chatPlan";
import { createProjectlessSession, lookupProjectlessSession, getProjectScope, pickProjectScope, restoreProjectScope } from "../../services/tauri/projectScope";
import { useInteractiveCards } from "../../hooks/useInteractiveCards";
import type { PermissionRequestCard } from "../../components/chat/PermissionCard";
import { useProjectEdgeActions, useProjectEdgeState } from "../../contexts/ProjectEdgeContext";
import { listTeams, createPoolFromTeam, type TeamInfo } from "../../services/tauri/teams";
import { listAgents, type AgentInfo } from "../../services/tauri/agents";
import { listInstalledSkills, type InstalledSkill } from "../../services/tauri/workbench";
import { listConnectors, type ConnectorInfo } from "../../services/tauri/connectors";
import {
  startWorkflow,
  subscribeWorkflowEvents,
  type WorkflowStatus,
} from "../../services/tauri/workflow";
import { collectArtifacts, type AgentStep } from "./agentArtifacts";
import { applyToolEnd, applyToolStart, finalizeTools } from "./agentTools";
import { workzGoalFromText } from "./taskTitle";
import {
  chatSetSessionCwd,
  workzListAllSessions,
  workzOverview,
  workzProjectAdd,
  workzProjectRemove,
  type WorkzOverview,
} from "../../services/tauri/workz";
import { openFolderDialog } from "../../services/tauri";
import "./Agent.css";

import type { LibraryInitialState } from "../codez/resources/types";

export type WikiBuildAction = "overview" | "deep" | "memory" | "graph";

interface WorkZWorkspaceProps {
  projectDir: string | null;
  openTaskRequest?: { projectDir: string; taskId: string; nonce: number } | null;
  onOpenFolder: () => void;
  /** Switch the CodeZ project to `dir` and show CodeZ. */
  onOpenProjectInCodeZ?: (dir: string) => void;
  /** Handoff a task's isolated worktree (and optional file) to CodeZ with a return pill. */
  onOpenTaskWorktreeInCodeZ?: (opts: {
    taskId: string;
    worktreePath: string;
    filePath?: string | null;
  }) => void;
  /** Increment from title bar to trigger repo wiki generation. */
  wikiBuildNonce?: number;
  wikiBuildAction?: WikiBuildAction;
  onWikiBusyChange?: (busy: boolean) => void;
  onOpenLibrary?: (initial?: LibraryInitialState) => void;
}

/**
 * Agent mode (≈ Codex) — task-centric autonomous coding.
 *
 * Each task is a kernel session: submit a goal, the agent plans → edits → runs
 * tools in the open project, streaming its steps. The task list shows past runs
 * and a Changes panel surfaces the resulting `git status` for review.
 */
const LAST_TASK_KEY_PREFIX = "agentz-workz-last-task:";

/** Older versions stored Windows paths with the verbatim `\\?\` prefix; saved values may still have it. */
function plainPath(path: string): string {
  if (path.startsWith("\\\\?\\UNC\\")) return "\\\\" + path.slice(8);
  if (/^\\\\\?\\[A-Za-z]:\\/.test(path)) return path.slice(4);
  return path;
}

/** Remember the open WorkZ task so a restart can reopen it (per project). */
function rememberLastTask(projectDir: string | null | undefined, id: string | null): void {
  if (!projectDir) return;
  try {
    const key = LAST_TASK_KEY_PREFIX + plainPath(projectDir);
    if (id) localStorage.setItem(key, id);
    else localStorage.removeItem(key);
  } catch {
    // storage unavailable — restore is best-effort
  }
}

function readLastTask(projectDir: string): string | null {
  try {
    return (
      localStorage.getItem(LAST_TASK_KEY_PREFIX + plainPath(projectDir)) ??
      localStorage.getItem(LAST_TASK_KEY_PREFIX + projectDir)
    );
  } catch {
    return null;
  }
}

const MAX_AUTO_RESUMES = 6;
const STREAM_FLUSH_MS = 80;

type WorkzScope = { kind: "free" } | { kind: "project"; dir: string };
const SCOPE_KEY = "agentz-workz-scope";
const COLLAPSED_KEY = "agentz-workz-collapsed";
const RECENT_CWDS_KEY = "agentz-workz-recent-cwds";

function readScope(appProjectDir: string | null): WorkzScope {
  try {
    const raw = localStorage.getItem(SCOPE_KEY);
    if (raw) {
      const v = JSON.parse(raw) as WorkzScope;
      if (v.kind === "free") return v;
      if (v.kind === "project" && v.dir) return { kind: "project", dir: plainPath(v.dir) };
    }
  } catch {
    // fall through to default
  }
  return appProjectDir ? { kind: "project", dir: appProjectDir } : { kind: "free" };
}

function readCollapsed(): Record<string, boolean> {
  try {
    return JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "{}") as Record<string, boolean>;
  } catch {
    return {};
  }
}

function readRecentCwds(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_CWDS_KEY) ?? "[]");
    if (!Array.isArray(v)) return [];
    const plain = v.filter((x): x is string => typeof x === "string").map(plainPath);
    return plain.filter((x, i) => plain.indexOf(x) === i).slice(0, 8);
  } catch {
    return [];
  }
}

export function useWorkzSessionStore({
  projectDir: appProjectDir,
  openTaskRequest,
  wikiBuildNonce = 0,
  wikiBuildAction = "overview",
  onWikiBusyChange,
}: WorkZWorkspaceProps) {
  const { t, i18n } = useTranslation();
  const { setArtifacts, setPreviewPath, refreshGitChanges, setPendingReview } =
    useProjectEdgeActions();
  const previewPath = useProjectEdgeState((s) => s.previewPath);
  const [tasks, setTasks] = useState<SessionMeta[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [steps, setSteps] = useState<AgentStep[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [goal, setGoal] = useState("");
  const [wikiBusy, setWikiBusy] = useState(false);
  const [modelId, setModelId] = useState(() => loadScopedModelId("workz"));
  const { appSettings, llmProviders, defaultModelLabel, defaultModelHint } = useAppSettings();
  const inputHistory = useInputHistory("agentz-input-history-workz");
  const [attachment, setAttachment] = useState<ChatAttachment | null>(null);
  const [attachmentPreview, setAttachmentPreview] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [planItems, setPlanItems] = useState<PlanTodoItem[]>([]);
  const applyPersistedPlan = useCallback((plan: DurablePlan) => {
    setPlanItems(plan.status === "cancelled" || plan.status === "replaced" ? [] : plan.items.map((item) => ({
      id: item.id,
      content: item.description,
      status: item.state === "done" ? "completed" : item.state,
    })));
  }, []);
  const [taskPanelOpen, setTaskPanelOpen] = useState(false);
  const [isolate, setIsolateState] = useState(
    () => localStorage.getItem("agentz-workz-isolate") !== "0",
  );
  const setIsolate = useCallback((next: boolean) => {
    setIsolateState(next);
    localStorage.setItem("agentz-workz-isolate", next ? "1" : "0");
  }, []);
  const [worktree, setWorktree] = useState<AgentTaskInfo | null>(null);
  const [startMode, setStartMode] = useState<"commit" | "working">("working");
  const [startBase, setStartBase] = useState("");
  const [reviewTask, setReviewTask] = useState<AgentTaskInfo | null>(null);
  const [teams, setTeams] = useState<TeamInfo[]>([]);
  const [activeTeam, setActiveTeam] = useState<string>("");
  const [agents, setAgents] = useState<AgentInfo[]>([]);
  const [activeAgentId, setActiveAgentId] = useState<string>(
    () => localStorage.getItem("agentz-workz-agent") ?? "",
  );
  const [activePoolId, setActivePoolId] = useState<string | null>(null);
  const [boardOpen, setBoardOpen] = useState(false);
  const [workflowRunId, setWorkflowRunId] = useState<string | null>(null);
  const [workflowOpen, setWorkflowOpen] = useState(false);
  const [workflowStatus, setWorkflowStatus] = useState<WorkflowStatus | null>(null);
  const [runsOpen, setRunsOpen] = useState(false);
  const [swarmMainTab, setSwarmMainTab] = useState<"main" | "chatroom" | "coordination">("main");
  const [installedSkills, setInstalledSkills] = useState<InstalledSkill[]>([]);
  const [enabledSkills, setEnabledSkills] = useState<string[]>(() => {
    try {
      const raw = localStorage.getItem("agentz-workz-enabled-skills");
      return raw ? (JSON.parse(raw) as string[]) : [];
    } catch {
      return [];
    }
  });
  const [connectors, setConnectors] = useState<ConnectorInfo[]>([]);
  const [enabledConnectors, setEnabledConnectors] = useState<string[]>(() => {
    try {
      const raw = localStorage.getItem("agentz-workz-enabled-connectors");
      return raw ? (JSON.parse(raw) as string[]) : [];
    } catch {
      return [];
    }
  });

  /** All WorkZ user tasks — sidebar lists single-agent and team sessions together. */
  // Same project database as CodeZ: list the shared sources so both views show the same sessions.
  const workzTaskSources = useMemo(() => SESSION_SOURCES_PROJECT_SHARED, []);
  const activePoolRef = useRef<string | null>(null);
  activePoolRef.current = activePoolId;

  const [overview, setOverview] = useState<WorkzOverview | null>(null);
  const [scope, setScope] = useState<WorkzScope>(() => readScope(appProjectDir));
  const [groups, setGroups] = useState<Record<string, SessionMeta[]>>({});
  const [groupErrors, setGroupErrors] = useState<Record<string, string>>({});
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(readCollapsed);
  const [cwd, setCwd] = useState<string | null>(null);
  const [managedWorkspace, setManagedWorkspace] = useState<string | null>(null);
  const [recentCwds, setRecentCwds] = useState<string[]>(readRecentCwds);
  const isFree = scope.kind === "free";
  /** Session database / project root: a registered project, or the internal free-session folder. */
  const projectDir: string | null = isFree ? (overview?.free_dir ?? null) : scope.dir;
  /** Where the agent actually works for plain sessions. */
  const freeCwd = cwd ?? overview?.default_work_dir ?? null;
  const freeTaskWorkspace = managedWorkspace ?? freeCwd;
  const [planProject, setPlanProject] = useState<{ selected: string; canonical: string } | null>(null);
  const projectPickerBusy = useRef(false);
  useEffect(() => {
    let cancelled = false;
    setPlanProject(null);
    if (!projectDir || !overview) return;
    const selected = projectDir;
    const existing = getProjectScope(selected);
    if (existing) {
      setPlanProject({ selected, canonical: existing.project_dir });
      return;
    }
    // The database root is host-known; cwd/worktree never mint project authority.
    void restoreProjectScope(isFree ? "workz_free" : "workz_project", isFree ? undefined : selected)
      .then((opened) => {
        if (!cancelled) setPlanProject({ selected, canonical: opened.project_dir });
      }).catch(() => {
        // Editing/chat remain available, but no plan authority is fabricated.
      });
    return () => { cancelled = true; };
  }, [projectDir, isFree, overview]);

  useEffect(() => {
    workzOverview()
      .then(setOverview)
      .catch(() => setOverview(null));
  }, []);

  useEffect(() => {
    if (!appProjectDir) return;
    workzProjectAdd(appProjectDir)
      .then(setOverview)
      .catch(() => {});
  }, [appProjectDir]);

  useEffect(() => {
    try {
      localStorage.setItem(SCOPE_KEY, JSON.stringify(scope));
    } catch {
      // best-effort persistence
    }
  }, [scope]);

  const refreshGroups = useCallback(() => {
    if (!overview) return;
    const dirs = [
      ...overview.projects.filter((p) => p.exists).map((p) => p.path),
      overview.free_dir,
    ];
    workzListAllSessions(dirs, workzTaskSources)
      .then((list) => {
        const g: Record<string, SessionMeta[]> = {};
        const errs: Record<string, string> = {};
        for (const item of list) {
          g[item.project_dir] = item.sessions;
          if (item.error) errs[item.project_dir] = item.error;
        }
        setGroups(g);
        setGroupErrors(errs);
      })
      .catch(() => {});
  }, [overview, workzTaskSources]);

  useEffect(() => {
    refreshGroups();
  }, [refreshGroups]);

  const rememberCwd = useCallback((dir: string) => {
    setRecentCwds((prev) => {
      const next = [dir, ...prev.filter((d) => d !== dir)].slice(0, 8);
      try {
        localStorage.setItem(RECENT_CWDS_KEY, JSON.stringify(next));
      } catch {
        // best-effort persistence
      }
      return next;
    });
  }, []);

  // Session ids of tasks currently running (foreground or background). Drives
  // the sidebar running indicators and whether the active task shows as busy.
  const [runningIds, setRunningIds] = useState<string[]>([]);

  const sessionRef = useRef<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const taRef = useRef<HTMLTextAreaElement>(null);
  const { slash, slashMatches, pickSlash, slashLabel, detectSlash, handleSlashKeyDown } =
    useSlashCompletion(setGoal, taRef);
  const { mention, mentionMatches, pickMention, detectAtMention, handleMentionKeyDown } =
    useAtMention(setGoal, taRef);
  const panelRef = useRef<HTMLDivElement>(null);
  const runRef = useRef<(text: string, att: ChatAttachment | null) => Promise<void>>(
    async () => {},
  );

  const showToast = useCallback((msg: string) => setToast(msg), []);

  // One timer per visible toast: a newer toast restarts it, so an older
  // timeout can never dismiss (or leave stuck) a later message.
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 4500);
    return () => window.clearTimeout(timer);
  }, [toast]);

  const { pendingCards, handleAgentEvent, markSubmitted, markActionSent, clearCards } =
    useInteractiveCards();
  const [permissionRequest, setPermissionRequest] = useState<PermissionRequestCard | null>(null);

  // ── Parallel task bookkeeping (M7) ──────────────────────────────────────
  // `live` gates whether incoming kernel events update the foreground view;
  // `foregroundSession` binds that view to a specific session id (null while a
  // brand-new task hasn't been assigned one yet). Background runs keep going
  // server-side under their own task key + cancel flag.
  const liveRef = useRef(false);
  // Token deltas are coalesced so a streaming reply re-renders at most every
  // STREAM_FLUSH_MS instead of once per token.
  const streamPendingRef = useRef("");
  const streamTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flushStreamDelta = useCallback(() => {
    if (streamTimerRef.current) {
      clearTimeout(streamTimerRef.current);
      streamTimerRef.current = null;
    }
    const chunk = streamPendingRef.current;
    streamPendingRef.current = "";
    if (!chunk) return;
    setSteps((prev) => {
      if (prev.length === 0) return prev;
      const last = prev[prev.length - 1];
      if (last.role !== "assistant") return prev;
      const copy = prev.slice();
      copy[copy.length - 1] = { ...last, text: last.text + chunk };
      return copy;
    });
  }, []);
  const discardStreamDelta = useCallback(() => {
    if (streamTimerRef.current) clearTimeout(streamTimerRef.current);
    streamTimerRef.current = null;
    streamPendingRef.current = "";
  }, []);
  useEffect(() => discardStreamDelta, [discardStreamDelta]);
  const foregroundSessionRef = useRef<string | null>(null);
  const foregroundTaskKeyRef = useRef<string | null>(null);
  const projectDirRef = useRef<string | null>(null);
  projectDirRef.current = projectDir;
  const runningSessionsRef = useRef<Set<string>>(new Set());
  const taskKeyBySessionRef = useRef<Map<string, string>>(new Map());
  const openTaskRequestRef = useRef(0);

  const markRunning = useCallback((id: string | null, on: boolean) => {
    if (!id) return;
    const set = runningSessionsRef.current;
    if (on) set.add(id);
    else set.delete(id);
    setRunningIds(Array.from(set));
  }, []);

  useEffect(() => {
    saveScopedModelId("workz", modelId);
  }, [modelId]);

  useEffect(() => {
    setModelId((id) => pruneModelId(id, llmProviders));
  }, [llmProviders]);

  useEffect(() => {
    listTeams()
      .then((list) => {
        setTeams(list);
        setActiveTeam((cur) => (cur && !list.some((tm) => tm.id === cur) ? "" : cur));
      })
      .catch(() => setTeams([]));
    listAgents()
      .then((list) => {
        setAgents(list);
        setActiveAgentId((cur) => (cur && !list.some((a) => a.id === cur) ? "" : cur));
      })
      .catch(() => setAgents([]));
    listInstalledSkills()
      .then((skills) => {
        setInstalledSkills(skills);
        const slugs = new Set(skills.map((s) => s.slug));
        setEnabledSkills((cur) => cur.filter((s) => slugs.has(s)));
      })
      .catch(() => setInstalledSkills([]));
    listConnectors()
      .then((list) => {
        setConnectors(list);
        const ids = new Set(list.map((c) => c.id));
        setEnabledConnectors((cur) => cur.filter((id) => ids.has(id)));
      })
      .catch(() => setConnectors([]));
  }, []);

  useEffect(() => {
    localStorage.setItem("agentz-workz-agent", activeAgentId);
  }, [activeAgentId]);

  useEffect(() => {
    localStorage.setItem("agentz-workz-enabled-skills", JSON.stringify(enabledSkills));
  }, [enabledSkills]);

  useEffect(() => {
    localStorage.setItem("agentz-workz-enabled-connectors", JSON.stringify(enabledConnectors));
  }, [enabledConnectors]);

  // Swarm team selected → materialise/reuse the pool immediately so the
  // collaboration board is available before the first coordinator turn
  // (workflow「历史」is team-scoped the same way).
  useEffect(() => {
    if (!projectDir || !activeTeam) return;
    const team = teams.find((tm) => tm.id === activeTeam);
    if (team?.mode !== "swarm") return;

    let cancelled = false;
    createPoolFromTeam(projectDir, activeTeam)
      .then((created) => {
        if (!cancelled) {
          setActivePoolId(created.pool_id);
          activePoolRef.current = created.pool_id;
        }
      })
      .catch((e) => {
        if (!cancelled) {
          const msg = String(e);
          setError(msg);
          setToast(msg);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [projectDir, activeTeam, teams]);

  // Track the active workflow run's status so the reopen button reflects it.
  useEffect(() => {
    if (!workflowRunId) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    subscribeWorkflowEvents((e) => {
      if (e.runId === workflowRunId) setWorkflowStatus(e.status);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [workflowRunId]);

  const clearAttachment = useCallback(() => {
    setAttachment(null);
    if (attachmentPreview?.startsWith("blob:")) URL.revokeObjectURL(attachmentPreview);
    setAttachmentPreview(null);
  }, [attachmentPreview]);

  useEffect(() => {
    sessionRef.current = null;
    openTaskRequestRef.current += 1;
    setSelectedId(null);
    setSteps([]);
    setTasks([]);
    setError(null);
    setGoal("");
    clearAttachment();
    setPreviewPath(null);
    setPlanItems([]);
    setWorktree(null);
    setIsolateState(localStorage.getItem("agentz-workz-isolate") !== "0");
    setReviewTask(null);
    liveRef.current = false;
    discardStreamDelta();
    foregroundSessionRef.current = null;
    foregroundTaskKeyRef.current = null;
    restoredTaskForRef.current = null;
    setBusy(false);
    setCwd(null);
    setManagedWorkspace(null);
    clearCards();
  }, [projectDir, clearAttachment, clearCards]);

  const refreshTasks = useCallback(() => {
    if (!projectDir) {
      setTasks([]);
      return;
    }
    listSessions(projectDir, workzTaskSources, null)
      .then(setTasks)
      .catch(() => setTasks([]));
    refreshGroups();
  }, [projectDir, workzTaskSources, refreshGroups]);

  /** True once a task session (or workflow run) is bound — mode pickers lock. */
  const taskBound = selectedId !== null || workflowRunId !== null;

  const applyTaskModeBinding = useCallback((meta: SessionMeta | undefined) => {
    const boundTeam = meta?.team_id?.trim() ?? "";
    setActiveTeam(boundTeam);
    setSwarmMainTab("main");
    if (meta?.pool_id) {
      setActivePoolId(meta.pool_id);
      activePoolRef.current = meta.pool_id;
    } else {
      setActivePoolId(null);
      activePoolRef.current = null;
    }
  }, []);

  const handleTeamChange = useCallback(
    (teamId: string) => {
      if (taskBound) return;
      setSwarmMainTab("main");
      setActivePoolId(null);
      activePoolRef.current = null;
      setActiveTeam(teamId);
    },
    [taskBound],
  );

  const handleAgentChange = useCallback(
    (agentId: string) => {
      if (taskBound) return;
      setActiveAgentId(agentId);
    },
    [taskBound],
  );

  useEffect(() => {
    refreshTasks();
    refreshGitChanges();
  }, [refreshTasks, refreshGitChanges]);

  useEffect(() => {
    setArtifacts(
      collectArtifacts(steps, undefined, [
        worktree?.worktree_path,
        freeTaskWorkspace,
        projectDir,
      ]),
    );
  }, [steps, setArtifacts, worktree?.worktree_path, freeTaskWorkspace, projectDir]);

  // Stream kernel events into the in-flight step. The event channel is shared
  // with the IDE chat panel and with any background Agent tasks, so we only
  // consume events that belong to the foreground run: once a session id is
  // known we match on it; while a brand-new task is still session-less we
  // accept events that aren't claimed by another running session.
  const applyEvent = useCallback(
    (env: ChatEventEnvelope) => {
      if (env.channel === "session_title" && env.sessionId) {
        const title = (env.payload as { title?: string }).title;
        if (title) {
          setTasks((prev) => {
            const idx = prev.findIndex((task) => task.id === env.sessionId);
            if (idx < 0) return prev;
            const next = prev.slice();
            next[idx] = { ...next[idx], title };
            return next;
          });
        }
        return;
      }
      // Turn completion must update every session — including background runs
      // after the user clicks「新建」— or the sidebar dot stays yellow forever.
      if (env.channel === "agent_final") {
        const fin = env.payload as { ok: boolean; error?: string; timed_out?: boolean };
        const fg = foregroundSessionRef.current;
        const isFg = liveRef.current && !!fg && env.sessionId === fg;
        if (isFg && fin.ok) autoResumeRef.current = 0;
        // A wall-clock turn timeout is not a failure: the run loop restarts the
        // turn itself once this one has fully wound down.
        const willResume = isFg && !!fin.timed_out && autoResumeRef.current < MAX_AUTO_RESUMES;
        if (willResume) {
          autoResumeRef.current += 1;
          pendingResumeRef.current = true;
          setToast(t("chat.turnTimeoutResuming"));
        } else if (isFg && !fin.ok && fin.error) {
          setError(
            fin.timed_out
              ? `${fin.error} (stopped after ${MAX_AUTO_RESUMES} automatic continuations — send a message to continue.)`
              : fin.error,
          );
        }
        markRunning(env.sessionId, false);
        setTasks((prev) =>
          prev.map((t) =>
            t.id === env.sessionId ? { ...t, status: fin.ok ? "idle" : "error" } : t,
          ),
        );
        // The user may be *viewing* this session without being live-attached
        // to it (reopened while it ran in the background). Its stream was
        // never applied here, so on completion the busy spinner must clear
        // and the transcript must reload from the DB — otherwise the view
        // stays busy and stale until the user reopens the task.
        if (env.sessionId && fg === env.sessionId && !liveRef.current) {
          setBusy(false);
          if (!fin.ok && fin.error) setError(fin.error);
          const dir = projectDirRef.current;
          const sid = env.sessionId;
          if (dir) {
            void getMessages(sid, dir)
              .then((history) => {
                if (foregroundSessionRef.current !== sid || liveRef.current) return;
                setSteps(
                  history.map((m) => ({
                    id: m.id,
                    role: m.role,
                    text: m.content,
                    tools: (m.tools ?? []).map((tool) => ({
                      id: tool.id,
                      name: tool.name,
                      input: tool.input,
                      result: tool.result ?? undefined,
                      status: tool.is_error || tool.result == null ? ("error" as const) : ("done" as const),
                      textOffset: tool.text_offset,
                    })),
                  })),
                );
              })
              .catch(() => {});
          }
        }
        return;
      }

      // The kernel multiplexes every session (coordinator turn + each member
      // Koi turn) onto one channel. Apply only events for the bound foreground
      // session; the id is always known up front now (pre-generated for new
      // tasks), so anything else — broadcasts included — is dropped.
      const fg = foregroundSessionRef.current;
      if (!fg || env.sessionId !== fg) return;
      if (env.channel !== "agent_event") return;
      const evt = env.payload as AgentEvent;

      if (evt.type === "permission_request") {
        setPermissionRequest({
          requestId: evt.request_id,
          toolName: evt.tool_name,
          toolInput: evt.tool_input,
          description: evt.description,
          approval: evt.approval,
        });
        return;
      }

      // A card's lifecycle must not depend on the React live flag: an
      // interactive event can race the initial state update, while a terminal
      // event can arrive just after the turn resolves. Keep those cards in sync
      // with their backend response channels in either case.
      // Every other event may depend on the text so far (tool_start records the
      // text offset it interleaves at), so buffered deltas land first.
      if (evt.type !== "text_delta") flushStreamDelta();
      if (
        evt.type === "interactive_ui" ||
        evt.type === "interactive_ui_patch" ||
        evt.type === "interactive_ui_listen" ||
        evt.type === "done" ||
        evt.type === "cancelled"
      ) {
        handleAgentEvent(evt);
        return;
      }
      if (!liveRef.current) return;

      switch (evt.type) {
        case "text_delta":
          streamPendingRef.current += evt.delta;
          if (!streamTimerRef.current) {
            streamTimerRef.current = setTimeout(flushStreamDelta, STREAM_FLUSH_MS);
          }
          break;
        case "tool_start":
          setSteps((prev) => {
            if (prev.length === 0) return prev;
            const copy = prev.slice();
            const last = { ...copy[copy.length - 1] };
            if (last.role !== "assistant") return prev;
            last.tools = applyToolStart(last.tools.slice(), evt, last.text.length);
            copy[copy.length - 1] = last;
            return copy;
          });
          break;
        case "tool_end":
          setSteps((prev) => {
            if (prev.length === 0) return prev;
            const copy = prev.slice();
            const last = { ...copy[copy.length - 1] };
            if (last.role !== "assistant") return prev;
            last.tools = applyToolEnd(last.tools.slice(), evt);
            copy[copy.length - 1] = last;
            return copy;
          });
          break;
        case "plan_update":
          setPlanItems(evt.items);
          break;
        case "error":
          setSteps((prev) => {
            if (prev.length === 0) return prev;
            const copy = prev.slice();
            const last = { ...copy[copy.length - 1] };
            if (last.role !== "assistant") return prev;
            last.text += `\n\n⚠️ ${evt.message}`;
            copy[copy.length - 1] = last;
            return copy;
          });
          break;
        default:
          handleAgentEvent(evt);
          break;
      }
    },
    [handleAgentEvent, markRunning, flushStreamDelta],
  );

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    onChatEvent(applyEvent).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applyEvent]);

  useStickToBottom(scrollRef);

  const autoResumeRef = useRef(0);
  const pendingResumeRef = useRef(false);
  const creatingProjectlessRef = useRef(false);

  runRef.current = async (text: string, att: ChatAttachment | null) => {
    if (isFree && !sessionRef.current && !selectedId && creatingProjectlessRef.current) return;
    const request = ++openTaskRequestRef.current;
    setError(null);
    const startSession = sessionRef.current ?? selectedId;
    let managedDirectory: string | null = null;
    let effectiveSessionId = startSession;
    if (isFree && !startSession) {
      if (creatingProjectlessRef.current) return;
      creatingProjectlessRef.current = true;
      try {
        const created = await createProjectlessSession();
        if (created.session_project_dir !== projectDir) {
          throw new Error("Projectless session authority changed during creation");
        }
        if (request !== openTaskRequestRef.current) return;
        effectiveSessionId = created.session_id;
        managedDirectory = created.directory.scope.project_dir;
        setManagedWorkspace(managedDirectory);
      } catch (e) {
        if (request === openTaskRequestRef.current) setError(String(e));
        return;
      } finally {
        creatingProjectlessRef.current = false;
      }
    }
    const displayText =
      att && !text.trim()
        ? `📎 ${att.filename ?? att.path ?? t("chat.attachment")}`
        : att
          ? `${text}${text.trim() ? "\n" : ""}📎 ${att.filename ?? att.path ?? t("chat.attachment")}`
          : text;
    setSteps((s) => [
      ...s,
      { role: "user", text: displayText, tools: [] },
      { role: "assistant", text: "", tools: [] },
    ]);
    setBusy(true);

    // Each turn gets a unique task key so Stop and the concurrency queue can
    // target it independently of any sibling task running in the background.
    const taskKey = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    // Continue the open task's session — never spawn a sibling row on follow-up.
    if (startSession) {
      sessionRef.current = startSession;
    }
    // Pre-generate the session id for a brand-new task so the sidebar can
    // select it, show the running indicator, and (critically) the event
    // stream can be filtered to *this* session immediately — otherwise
    // member-Koi turns sharing the event channel leak into the view while the
    // turn is session-less. The backend honours a client-provided id.
    effectiveSessionId ??= crypto.randomUUID();
    liveRef.current = true;
    foregroundSessionRef.current = effectiveSessionId;
    foregroundTaskKeyRef.current = taskKey;
    sessionRef.current = effectiveSessionId;
    rememberLastTask(projectDir, effectiveSessionId);
    taskKeyBySessionRef.current.set(effectiveSessionId, taskKey);
    markRunning(effectiveSessionId, true);
    setSelectedId(effectiveSessionId);
    if (!startSession) {
      // Optimistically surface the new task in the sidebar; refreshTasks()
      // reconciles it (real title / status) once the turn finishes.
      const optimistic: SessionMeta = {
        id: effectiveSessionId,
        title: workzGoalFromText(text) || text.trim().slice(0, 60) || t("agent.untitled"),
        status: "running",
        message_count: 1,
        updated_at: new Date().toISOString(),
        source: activeTeam ? SESSION_SOURCE_WORKZ_TEAM : SESSION_SOURCE_WORKZ,
        team_id: activeTeam || null,
        pool_id: activePoolRef.current,
      };
      setTasks((prev) =>
        prev.some((p) => p.id === effectiveSessionId) ? prev : [optimistic, ...prev],
      );
    }

    const isForeground = () => foregroundTaskKeyRef.current === taskKey;
    let environmentResolved = false;
    const activeProject =
      !isFree && projectDir
        ? (overview?.projects.find((p) => p.path === projectDir) ?? null)
        : null;
    const effectiveIsolate = isolate && !isFree && activeProject?.kind !== "dir";

    try {
      const wt = await resolveTaskWorktree({
        projectDir: projectDir!,
        taskId: effectiveSessionId,
        isolate: effectiveIsolate,
        isFree,
        resuming: !!startSession,
        startMode,
        base: startMode === "commit" ? startBase.trim() || undefined : undefined,
      });
      environmentResolved = true;
      if (isForeground()) setWorktree(wt);
      // Team mode: ensure a Pool exists for the selected team, then instruct the
      // agent to orchestrate its members via the pool tools.
      let effectivePrompt = text;
      if (activeTeam) {
        try {
          // Re-sync member agent models into the collaboration pool on every task start.
          const created = await createPoolFromTeam(projectDir!, activeTeam);
          const poolId = created.pool_id;
          setActivePoolId(poolId);
          activePoolRef.current = poolId;
          // Only establish the coordinator role + org_spec on the first turn of a
          // session; follow-up messages stay raw so the contract isn't repeated
          // (the first-turn instruction persists in the session history).
          if (!startSession) {
            const orgSpec = teams.find((tm) => tm.id === activeTeam)?.org_spec?.trim();
            const orgSection = orgSpec
              ? `\n\nYour team operates under this organization contract (org_spec); ` +
                `every member assistant receives it too, so hold yourself and them to it:\n${orgSpec}`
              : "";
            effectivePrompt =
              `You are the coordinator of swarm team "${activeTeam}" (collaboration pool id: ${poolId}). ` +
              `Use the pool_org and pool_chat tools to break this down into todos, assign them ` +
              `to member assistants, and integrate their results.${orgSection}\n\nTask:\n${text}`;
          }
        } catch (e) {
          setError(String(e));
        }
      }
      const runCwd = managedDirectory ?? (isFree ? freeTaskWorkspace : null);
      const res = await chatSend({
        prompt: effectivePrompt,
        displayPrompt: text,
        sessionId: effectiveSessionId,
        projectDir: projectDir!,
        workspaceDir: wt?.worktree_path ?? runCwd,
        requireTaskEnvironment: !!wt || effectiveIsolate,
        attachment: att,
        chatMode: "agent",
        clearPlan: false,
        modelId: showModelSelector ? modelId || null : null,
        taskKey,
        // Single-agent mode: run as the chosen persona. Team mode uses the
        // coordinator path, so no per-agent persona is applied there.
        agentId: activeTeam ? null : activeAgentId || null,
        enabledSkills: showModelSelector && enabledSkills.length > 0 ? enabledSkills : null,
        enabledConnectors: showModelSelector ? enabledConnectors : null,
        sessionSource: activeTeam ? SESSION_SOURCE_WORKZ_TEAM : SESSION_SOURCE_WORKZ,
        teamId: activeTeam || null,
        poolId: activePoolRef.current,
        preferZh: i18n.language.startsWith("zh"),
      });
      // The backend echoes the (pre-generated) session id; keep the maps in
      // sync in case it ever differs, then fold in the final assistant text.
      if (isFree && !startSession && runCwd && !managedDirectory) {
        void chatSetSessionCwd(projectDir!, res.session_id, runCwd).catch(() => {});
        rememberCwd(runCwd);
        if (isForeground()) setCwd(runCwd);
      }
      if (res.session_id !== effectiveSessionId) {
        taskKeyBySessionRef.current.set(res.session_id, taskKey);
        markRunning(res.session_id, true);
      }
      if (isForeground()) {
        sessionRef.current = res.session_id;
        foregroundSessionRef.current = res.session_id;
        setSelectedId(res.session_id);
        setSteps((s) => {
          const copy = s.slice();
          const last = copy[copy.length - 1];
          if (last && last.role === "assistant") {
            copy[copy.length - 1] = {
              ...last,
              text: last.text.trim() ? last.text : res.response_text,
              turnId: res.turn_id ?? last.turnId,
            };
          }
          return copy;
        });
        if (res.turn_id && projectDir) {
          try {
            const journalChanges = await journalListChanges(
              projectDir,
              res.session_id,
              res.turn_id,
            );
            setPendingReview(
              journalChanges.length > 0
                ? {
                    sessionId: res.session_id,
                    turnId: res.turn_id,
                    changes: journalChanges,
                  }
                : null,
            );
          } catch {
            /* journal is best-effort */
          }
        }
      }
    } catch (e) {
      if (!startSession && !environmentResolved && isForeground()) {
        sessionRef.current = null;
        foregroundSessionRef.current = null;
        setSelectedId(null);
        rememberLastTask(projectDir, null);
        setTasks((previous) => previous.filter((item) => item.id !== effectiveSessionId));
      }
      if (isForeground())
        setError(
          String(e).includes(MISSING_TASK_ENVIRONMENT)
            ? t("agent.taskEnvironmentMissing")
            : String(e),
        );
    } finally {
      // Clear the running flag for *this* run's own session — never the
      // session the user may have navigated to in the meantime.
      markRunning(effectiveSessionId, false);
      if (startSession && startSession !== effectiveSessionId) {
        markRunning(startSession, false);
      }
      if (isForeground()) {
        flushStreamDelta();
        setBusy(false);
        liveRef.current = false;
        setSteps((s) =>
          s.map((step) =>
            step.role === "assistant" ? { ...step, tools: finalizeTools(step.tools) } : step,
          ),
        );
      }
      refreshTasks();
      refreshGitChanges();
      if (pendingResumeRef.current) {
        pendingResumeRef.current = false;
        if (isForeground()) void runRef.current?.(t("chat.autoContinuePrompt"), null);
      }
    }
  };

  const run = useCallback(() => {
    const text = goal.trim();
    if ((!text && !attachment) || busy) return;
    autoResumeRef.current = 0;
    if (!projectDir) {
      setError(t("agent.noProject"));
      return;
    }
    if (text) inputHistory.push(text);
    // Workflow teams run a deterministic graph (no chat turn / coordinator).
    const teamInfo = teams.find((tm) => tm.id === activeTeam);
    if (activeTeam && teamInfo?.mode === "workflow") {
      setGoal("");
      setError(null);
      setWorkflowStatus("running");
      startWorkflow(projectDir, activeTeam, text)
        .then((started) => {
          setWorkflowRunId(started.run_id);
          setWorkflowOpen(true);
        })
        .catch((e) => setError(String(e)));
      return;
    }
    const pendingAttachment = attachment;
    setGoal("");
    clearAttachment();
    void runRef.current(text, pendingAttachment);
  }, [goal, attachment, busy, projectDir, clearAttachment, inputHistory, t, teams, activeTeam]);

  const newTask = useCallback(() => {
    if (!projectDir) return;
    // Detach the current view from any in-flight run — it keeps going in the
    // background and resurfaces in the task list when it finishes. Clearing
    // the session binding unlocks team / agent mode pickers for the next task.
    liveRef.current = false;
    discardStreamDelta();
    foregroundSessionRef.current = null;
    foregroundTaskKeyRef.current = null;
    setBusy(false);
    sessionRef.current = null;
    rememberLastTask(projectDir, null);
    setSelectedId(null);
    setWorkflowRunId(null);
    setWorkflowStatus(null);
    setSteps([]);
    setPlanItems([]);
    setError(null);
    setGoal("");
    clearAttachment();
    setWorktree(null);
    setIsolateState(localStorage.getItem("agentz-workz-isolate") !== "0");
    setCwd(null);
    setManagedWorkspace(null);
    openTaskRequestRef.current += 1;
    // Land on the main chat so a fresh task is visibly empty (instead of the
    // previous run's Koi chatroom / coordination view lingering).
    setSwarmMainTab("main");
    // A pending `chat_ui` card belongs to the task that produced it; the new
    // task view must start empty or the stale form leaks into it.
    clearCards();
    requestAnimationFrame(() => taRef.current?.focus());
  }, [projectDir, clearAttachment, clearCards]);

  const openTask = useCallback(
    async (id: string) => {
      if (!projectDir) return;
      // Clicking the session that is already the live foreground run must not
      // wipe its streaming tool calls / detach the stream — that was the
      // "in-progress tools vanish on click" bug. It's already on screen.
      if (id === foregroundSessionRef.current && liveRef.current) {
        setSelectedId(id);
        return;
      }
      try {
        const request = ++openTaskRequestRef.current;
        const meta = tasks.find((task) => task.id === id);
        applyTaskModeBinding(meta);
        setCwd(meta?.cwd ?? null);
        const [historyResult, environmentResult, managedResult] = await Promise.allSettled([
          getMessages(id, projectDir),
          agentTaskApi.get(projectDir, id),
          isFree ? lookupProjectlessSession(id) : Promise.resolve(null),
        ]);
        if (request !== openTaskRequestRef.current) return;
        if (historyResult.status === "rejected") throw historyResult.reason;
        const history = historyResult.value;
        if (managedResult.status === "rejected") throw managedResult.reason;
        const managed = managedResult.value;
        if (managed && managed.session_project_dir !== projectDir) {
          throw new Error("Projectless session authority changed during reopen");
        }
        setManagedWorkspace(managed?.directory.scope.project_dir ?? null);
        const restoredEnvironment =
          environmentResult.status === "fulfilled" ? environmentResult.value : null;
        setWorktree(restoredEnvironment);
        if (environmentResult.status === "fulfilled") {
          setIsolateState(Boolean(restoredEnvironment));
        }
        // Switching to a different task detaches live streaming from the
        // previous foreground run (it keeps going in the background). We show
        // the opened task's persisted history; if it is itself running, its
        // results refresh on completion.
        liveRef.current = false;
        discardStreamDelta();
        foregroundTaskKeyRef.current = null;
        foregroundSessionRef.current = id;
        sessionRef.current = id;
        rememberLastTask(projectDir, id);
        setBusy(runningSessionsRef.current.has(id));
        setSelectedId(id);
        setSteps(
          history.map((m) => ({
            id: m.id,
            role: m.role,
            text: m.content,
            tools: (m.tools ?? []).map((t) => ({
              id: t.id,
              name: t.name,
              input: t.input,
              result: t.result ?? undefined,
              status: t.is_error || t.result == null ? ("error" as const) : ("done" as const),
              textOffset: t.text_offset,
            })),
          })),
        );
        setPlanItems([]);
        const scope = planProject?.selected === projectDir ? getProjectScope(planProject.canonical) : null;
        if (scope?.handle) {
          void queryTaskPlan(scope.handle, id)
            .then((r) => {
              if (sessionRef.current === id && r.current) applyPersistedPlan(r.current);
            })
            .catch(() => {});
        }
        setError(environmentResult.status === "rejected" ? String(environmentResult.reason) : null);
        setPreviewPath(null);
        refreshGitChanges();
        clearCards();
      } catch (e) {
        setError(String(e));
      }
    },
    [projectDir, isFree, refreshGitChanges, tasks, applyTaskModeBinding, clearCards, planProject, applyPersistedPlan],
  );

  // After a restart, reopen the task that was active when the app closed.
  const openTaskRef = useRef(openTask);
  openTaskRef.current = openTask;
  const restoredTaskForRef = useRef<string | null>(null);
  useEffect(() => {
    if (!projectDir || tasks.length === 0) return;
    if (restoredTaskForRef.current === projectDir) return;
    restoredTaskForRef.current = projectDir;
    if (sessionRef.current || liveRef.current) return;
    const lastId = readLastTask(projectDir);
    if (!lastId) return;
    if (tasks.some((task) => task.id === lastId)) void openTaskRef.current(lastId);
    else rememberLastTask(projectDir, null);
  }, [projectDir, tasks]);

  // Cancel the task bound to the active view (foreground run, or a reopened
  // background run we still hold the key for).
  const stopActive = useCallback(() => {
    const sid = sessionRef.current;
    const key =
      foregroundTaskKeyRef.current ?? (sid ? (taskKeyBySessionRef.current.get(sid) ?? null) : null);
    void chatCancel(key);
  }, []);

  const removeTask = useCallback(
    async (id: string) => {
      if (!projectDir) return;
      try {
        await deleteSession(id, projectDir);
        markRunning(id, false);
        taskKeyBySessionRef.current.delete(id);
        if (sessionRef.current === id) newTask();
        refreshTasks();
      } catch (e) {
        setError(String(e));
      }
    },
    [projectDir, newTask, refreshTasks, markRunning],
  );

  const buildWiki = useCallback(
    async (action: WikiBuildAction) => {
      if (wikiBusy) return;
      if (isFree || !projectDir) {
        setError(t("workz.wikiNeedsProject"));
        return;
      }
      setWikiBusy(true);
      onWikiBusyChange?.(true);
      setError(null);
      try {
        if (action === "graph") {
          await requestGraphIndex(projectDir);
        } else if (action === "deep") {
          const res = await generateDeepRepoWiki(projectDir);
          setPreviewPath(res.path);
        } else if (action === "memory") {
          const res = await readRepoMemory(projectDir);
          setPreviewPath(res.path);
        } else {
          const res = await generateRepoWiki(projectDir);
          setPreviewPath(res.path);
        }
      } catch (e) {
        setError(String(e));
      } finally {
        setWikiBusy(false);
        onWikiBusyChange?.(false);
      }
    },
    [projectDir, isFree, t, wikiBusy, onWikiBusyChange, setPreviewPath],
  );

  const wikiTriggerRef = useRef(0);
  useEffect(() => {
    if (wikiBuildNonce <= 0 || wikiBuildNonce === wikiTriggerRef.current) return;
    wikiTriggerRef.current = wikiBuildNonce;
    void buildWiki(wikiBuildAction);
  }, [wikiBuildNonce, wikiBuildAction, buildWiki]);

  useEffect(() => {
    const onOpenAnchor = (e: Event) => {
      const detail = (e as CustomEvent<{ path?: string }>).detail;
      if (detail?.path) {
        setPreviewPath(detail.path);
      }
    };
    window.addEventListener("agentz:open-source-anchor", onOpenAnchor);
    return () => window.removeEventListener("agentz:open-source-anchor", onOpenAnchor);
  }, [setPreviewPath]);

  const sameScope = useCallback(
    (dir: string) => (isFree ? dir === overview?.free_dir : dir === scope.dir),
    [isFree, overview, scope],
  );

  const scopeForDir = useCallback(
    (dir: string): WorkzScope =>
      dir === overview?.free_dir ? { kind: "free" } : { kind: "project", dir },
    [overview],
  );

  const openSession = useCallback(
    (dir: string, id: string) => {
      // CodeZ and WorkZ are two views of one project's sessions: any CodeZ or WorkZ session opens
      // here. The stored source keeps governing its behaviour (the host accepts either view).
      if (sameScope(dir)) {
        void openTask(id);
        return;
      }
      rememberLastTask(dir, id);
      setScope(scopeForDir(dir));
    },
    [sameScope, openTask, scopeForDir],
  );

  const lastInboxRequest = useRef<number | null>(null);
  useEffect(() => {
    if (!openTaskRequest || lastInboxRequest.current === openTaskRequest.nonce) return;
    lastInboxRequest.current = openTaskRequest.nonce;
    openSession(openTaskRequest.projectDir, openTaskRequest.taskId);
  }, [openTaskRequest, openSession]);

  const newSessionIn = useCallback(
    (dir: string) => {
      if (sameScope(dir)) {
        newTask();
        return;
      }
      rememberLastTask(dir, null);
      setScope(scopeForDir(dir));
    },
    [sameScope, newTask, scopeForDir],
  );

  const deleteSessionIn = useCallback(
    (dir: string, id: string) => {
      if (sameScope(dir)) {
        void removeTask(id);
        return;
      }
      deleteSession(id, dir)
        .then(refreshGroups)
        .catch((e) => setError(String(e)));
    },
    [sameScope, removeTask, refreshGroups],
  );

  const addProject = useCallback(async () => {
    if (projectPickerBusy.current) return;
    projectPickerBusy.current = true;
    try {
      const opened = await pickProjectScope();
      if (!opened) return;
      const dir = opened.project_dir;
      const ov = await workzProjectAdd(dir);
      setOverview(ov);
      const added = ov.projects.find(
        (p) => p.path.replace(/[\\/]+$/, "") === dir.replace(/[\\/]+$/, ""),
      );
      const key = added?.path ?? dir;
      rememberLastTask(key, null);
      setScope({ kind: "project", dir: key });
    } catch (e) {
      setError(String(e));
    } finally {
      projectPickerBusy.current = false;
    }
  }, []);

  const removeProject = useCallback(
    async (dir: string) => {
      if (managedWorkspace) return;
      try {
        const ov = await workzProjectRemove(dir);
        setOverview(ov);
        if (!isFree && scope.dir === dir) setScope({ kind: "free" });
      } catch (e) {
        setError(String(e));
      }
    },
    [isFree, scope],
  );

  const toggleGroup = useCallback((dir: string) => {
    setCollapsed((prev) => {
      const next = { ...prev, [dir]: !(prev[dir] ?? false) };
      try {
        localStorage.setItem(COLLAPSED_KEY, JSON.stringify(next));
      } catch {
        // best-effort persistence
      }
      return next;
    });
  }, []);

  const changeCwd = useCallback(
    async (dir: string) => {
      let target = dir;
      if (dir === "__pick__") {
        const picked = await openFolderDialog(freeCwd);
        if (!picked) return;
        target = picked;
      }
      setCwd(target);
      rememberCwd(target);
      if (selectedId && projectDir) {
        try {
          await chatSetSessionCwd(projectDir, selectedId, target);
        } catch (e) {
          setError(String(e));
        }
      }
    },
    [freeCwd, managedWorkspace, rememberCwd, selectedId, projectDir],
  );

  const cwdOptions = useMemo((): DropdownOption[] => {
    const opts: DropdownOption[] = [];
    const def = overview?.default_work_dir;
    if (def) opts.push({ id: def, label: `${t("workz.cwdDefault")} · ${def}` });
    for (const d of recentCwds) {
      if (d !== def) opts.push({ id: d, label: d });
    }
    if (freeCwd && !opts.some((o) => o.id === freeCwd)) opts.push({ id: freeCwd, label: freeCwd });
    if (managedWorkspace && !opts.some((o) => o.id === managedWorkspace)) opts.push({ id: managedWorkspace, label: managedWorkspace });
    opts.push({ id: "__pick__", label: t("workz.cwdPick") });
    return opts;
  }, [overview, recentCwds, freeCwd, managedWorkspace, t]);

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const ta = e.currentTarget;
    if (handleMentionKeyDown(e)) return;
    if (handleSlashKeyDown(e)) return;
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const atStart = ta.selectionStart === 0 && ta.selectionEnd === 0;
      const atEnd = ta.selectionStart === goal.length && ta.selectionEnd === goal.length;
      if (e.key === "ArrowUp" && !atStart) return;
      if (e.key === "ArrowDown" && !atEnd) return;
      const next = inputHistory.navigate(e.key === "ArrowUp" ? "up" : "down", goal);
      if (next !== null) {
        e.preventDefault();
        setGoal(next);
        requestAnimationFrame(() => {
          const el = taRef.current;
          if (!el) return;
          const pos = next.length;
          el.setSelectionRange(pos, pos);
        });
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      run();
    }
  };

  // Set the (single) attachment, guarding images behind a vision-capable model
  // so we never silently drop an image the model can't read. Returns false when
  // rejected (caller may show its own feedback).
  const acceptAttachment = useCallback(
    (att: ChatAttachment, preview: string | null): boolean => {
      if (att.media_type.startsWith("image/")) {
        if (!appSettings || !visionCapable(appSettings, modelId, llmProviders)) {
          showToast(t("chat.visionRequired"));
          return false;
        }
      }
      if (attachmentPreview?.startsWith("blob:")) URL.revokeObjectURL(attachmentPreview);
      setAttachment(att);
      setAttachmentPreview(preview);
      return true;
    },
    [appSettings, modelId, llmProviders, attachmentPreview, showToast, t],
  );

  const handleAttach = useCallback(async () => {
    try {
      const picked = await pickChatAttachment();
      if (!picked) return;
      acceptAttachment(picked.attachment, picked.preview);
    } catch (e) {
      setError(String(e));
    }
  }, [acceptAttachment]);

  const handlePaste = useCallback(
    async (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const items = e.clipboardData?.items;
      if (!items) return;
      for (const item of items) {
        if (!item.type.startsWith("image/")) continue;
        e.preventDefault();
        const file = item.getAsFile();
        if (!file) return;
        try {
          const dataUrl = await blobToDataUrl(file);
          const data = dataUrlToBase64(dataUrl);
          acceptAttachment(
            {
              media_type: file.type || "image/png",
              data,
              filename: file.name || "paste.png",
              path: null,
            },
            dataUrl,
          );
        } catch (err) {
          setError(String(err));
        }
        return;
      }
    },
    [acceptAttachment],
  );

  // OS file drag-and-drop → attach the dropped file (image goes to vision when
  // the model supports it; other types are passed by path). Tauri captures
  // native file drops at the webview level, so we subscribe to that stream and
  // only react when the pointer is over this panel (mirrors the IDE chat).
  const applyDroppedPaths = useCallback(
    async (paths: string[]) => {
      const first = paths.find((p) => p.trim());
      if (!first) return;
      try {
        const built = await attachmentFromPath(first);
        if (acceptAttachment(built.attachment, built.preview)) {
          taRef.current?.focus();
        }
      } catch (e) {
        setError(String(e));
      }
    },
    [acceptAttachment],
  );

  useEffect(() => {
    const pointInPanel = (x: number, y: number) => {
      const el = panelRef.current;
      if (!el) return false;
      const rect = el.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const cx = x / dpr;
      const cy = y / dpr;
      return cx >= rect.left && cx <= rect.right && cy >= rect.top && cy <= rect.bottom;
    };
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "over") {
          setDragOver(pointInPanel(p.position.x, p.position.y));
        } else if (p.type === "drop") {
          const over = pointInPanel(p.position.x, p.position.y);
          setDragOver(false);
          if (over && p.paths.length > 0) void applyDroppedPaths(p.paths);
        } else {
          setDragOver(false);
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [applyDroppedPaths]);

  const modelOptions = useMemo((): ComposerMenuOption[] => {
    const opts: ComposerMenuOption[] = [
      {
        id: "",
        label: defaultModelLabel || t("chat.modelDefault"),
        hint: defaultModelHint || t("chat.modelDefault"),
      },
    ];
    for (const p of llmProviders) {
      opts.push({
        id: p.id,
        label: modelDisplayLabel(p),
        hint: modelLabel(p),
      });
    }
    return opts;
  }, [llmProviders, defaultModelLabel, defaultModelHint, t]);

  const canSend = Boolean(projectDir && (goal.trim() || attachment));
  const selectedTeam = useMemo(() => teams.find((tm) => tm.id === activeTeam), [teams, activeTeam]);
  const isSwarmTeam = selectedTeam?.mode === "swarm";
  const isWorkflowTeam = selectedTeam?.mode === "workflow";
  /** Generic single-agent: model + skill + connector pickers (no team, no named agent). */
  const showModelSelector = !activeTeam && !activeAgentId;
  const showGenericComposerTools = showModelSelector;
  const composerModeNotice = useMemo(() => {
    // Lock reason lives on the disabled team/agent/isolate controls (title tooltip).
    if (taskBound) return null;
    if (activeTeam) {
      return isWorkflowTeam ? t("agent.modelWorkflowTeam") : t("agent.modelSwarmTeam");
    }
    if (activeAgentId) return t("agent.modelBoundAgent");
    return null;
  }, [taskBound, activeTeam, activeAgentId, isWorkflowTeam, t]);

  const teamOptions = useMemo((): DropdownOption[] => {
    const opts: DropdownOption[] = [
      {
        id: "",
        label: t("agent.teamNone"),
        title: t("agent.teamHint"),
      },
    ];
    for (const tm of teams) {
      const isWf = tm.mode === "workflow";
      const modeLabel = isWf ? t("workflow.modeWorkflow") : t("workflow.modeSwarm");
      const modeTip = isWf ? t("agent.teamHintWorkflow") : t("agent.teamHintSwarm");
      const desc = tm.description?.trim();
      opts.push({
        id: tm.id,
        label: tm.name,
        hint: desc ? `${modeLabel} · ${desc}` : modeLabel,
        title: desc ? `${modeTip}\n${desc}` : modeTip,
      });
    }
    return opts;
  }, [teams, t]);

  const agentOptions = useMemo((): DropdownOption[] => {
    const opts: DropdownOption[] = [
      {
        id: "",
        label: t("agent.agentGeneric"),
        title: t("agent.agentHint"),
      },
    ];
    for (const a of agents) {
      const desc = a.description?.trim();
      opts.push({
        id: a.id,
        label: `${a.icon ? `${a.icon} ` : ""}${a.name}`,
        hint: desc || undefined,
        title: desc || a.name,
      });
    }
    return opts;
  }, [agents, t]);

  return {
    t, i18n, panelRef, dragOver, toast, overview, projectDir, tasks, groups, groupErrors, collapsed, selectedId, runningIds, toggleGroup, openSession, newSessionIn, deleteSessionIn, addProject, removeProject, previewPath, isFree, freeTaskWorkspace, isSwarmTeam, swarmMainTab, setSwarmMainTab, activePoolId, scrollRef, steps, busy, sessionRef, pendingCards, permissionRequest, setPermissionRequest, markSubmitted, markActionSent, setGoal, taskBound, setIsolate, taRef, refreshGitChanges, showToast, setError, worktree, setPreviewPath, error, planItems, setPlanItems, taskPanelOpen, setTaskPanelOpen, managedWorkspace, cwdOptions, changeCwd, isolate, startMode, setStartMode, startBase, setStartBase, teams, activeTeam, selectedTeam, teamOptions, handleTeamChange, agents, activeAgentId, agentOptions, handleAgentChange, setBoardOpen, isWorkflowTeam, workflowRunId, setWorkflowRunId, workflowStatus, setWorkflowStatus, setWorkflowOpen, setRunsOpen, setReviewTask, goal, detectAtMention, detectSlash, run, stopActive, canSend, onKeyDown, handlePaste, modelId, modelOptions, setModelId, showModelSelector, showGenericComposerTools, installedSkills, enabledSkills, setEnabledSkills, connectors, enabledConnectors, setEnabledConnectors, mention, mentionMatches, pickMention, slash, slashMatches, pickSlash, slashLabel, composerModeNotice, attachment, attachmentPreview, handleAttach, clearAttachment, boardOpen, workflowOpen, runsOpen, reviewTask, refreshTasks,
  };
}