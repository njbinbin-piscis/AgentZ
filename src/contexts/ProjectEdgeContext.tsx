import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import type { GitFileStatus } from "../workspaces/codez/types";
import type { JournalChange } from "../services/tauri/chat";
import { perfCounters } from "../utils/perfCounters";

export interface PendingReview {
  sessionId: string;
  turnId: string;
  changes: JournalChange[];
}

type RefreshKind = "git" | "fileTree";
/** May return a promise so the scheduler can avoid overlapping runs. */
type RefreshHandler = () => void | Promise<unknown>;

export interface ProjectEdgeState {
  gitChanges: GitFileStatus[];
  artifacts: string[];
  pendingReview: PendingReview | null;
  previewPath: string | null;
  agentTurnBusy: boolean;
}

/** Stable callbacks — identities never change for the provider's lifetime. */
export interface ProjectEdgeActions {
  setGitChanges: (changes: GitFileStatus[]) => void;
  /** @deprecated Prefer scheduleWorkspaceRefresh({ git: true }) */
  refreshGitChanges: () => void;
  registerRefreshGitChanges: (fn: () => void) => () => void;
  registerWorkspaceRefresh: (kind: RefreshKind, fn: RefreshHandler) => () => void;
  scheduleWorkspaceRefresh: (opts?: {
    git?: boolean;
    fileTree?: boolean;
    delayMs?: number;
    /** Bypass agent-turn pause (e.g. turn finished). */
    force?: boolean;
  }) => void;
  setAgentTurnBusy: (busy: boolean) => void;
  setArtifacts: (paths: string[]) => void;
  setPendingReview: (review: PendingReview | null) => void;
  setPreviewPath: (path: string | null) => void;
  onSelectPath: (path: string) => void;
  registerOnSelectPath: (fn: (path: string) => void) => () => void;
}

interface ProjectEdgeStore {
  getState: () => ProjectEdgeState;
  subscribe: (listener: () => void) => () => void;
}

const ProjectEdgeStoreContext = createContext<ProjectEdgeStore | null>(null);
const ProjectEdgeActionsContext = createContext<ProjectEdgeActions | null>(null);

const DEFAULT_REFRESH_DELAY_MS = 250;

function sameStrings(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i]);
}

function sameGitChanges(a: GitFileStatus[], b: GitFileStatus[]): boolean {
  return (
    a.length === b.length &&
    a.every((v, i) => v.path === b[i].path && v.status === b[i].status && v.staged === b[i].staged)
  );
}

export function ProjectEdgeProvider({ children }: { children: ReactNode }) {
  const stateRef = useRef<ProjectEdgeState>({
    gitChanges: [],
    artifacts: [],
    pendingReview: null,
    previewPath: null,
    agentTurnBusy: false,
  });
  const listeners = useRef(new Set<() => void>());

  const store = useMemo<ProjectEdgeStore>(
    () => ({
      getState: () => stateRef.current,
      subscribe: (listener) => {
        listeners.current.add(listener);
        return () => listeners.current.delete(listener);
      },
    }),
    [],
  );

  const update = useCallback((patch: Partial<ProjectEdgeState>) => {
    stateRef.current = { ...stateRef.current, ...patch };
    for (const listener of listeners.current) listener();
  }, []);

  const refreshHandlers = useRef<Partial<Record<RefreshKind, RefreshHandler>>>({});
  const refreshTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingRefresh = useRef<{ git: boolean; fileTree: boolean }>({
    git: false,
    fileTree: false,
  });
  const refreshInFlight = useRef(false);
  const selectHandler = useRef<((path: string) => void) | null>(null);

  const flushWorkspaceRefresh = useCallback(() => {
    refreshTimer.current = null;
    // Requests arriving mid-flight stay pending and run once afterwards.
    if (refreshInFlight.current) return;
    const { git, fileTree } = pendingRefresh.current;
    pendingRefresh.current = { git: false, fileTree: false };
    if (!git && !fileTree) return;

    perfCounters.recordWorkspaceRefreshFlushed();
    refreshInFlight.current = true;
    const runs: Promise<unknown>[] = [];
    if (git) runs.push(Promise.resolve(refreshHandlers.current.git?.()));
    if (fileTree) runs.push(Promise.resolve(refreshHandlers.current.fileTree?.()));
    void Promise.allSettled(runs).then(() => {
      refreshInFlight.current = false;
      const next = pendingRefresh.current;
      if ((next.git || next.fileTree) && !refreshTimer.current) flushWorkspaceRefresh();
    });
  }, []);

  const actions = useMemo<ProjectEdgeActions>(() => {
    const registerWorkspaceRefresh = (kind: RefreshKind, fn: RefreshHandler) => {
      refreshHandlers.current[kind] = fn;
      return () => {
        if (refreshHandlers.current[kind] === fn) {
          delete refreshHandlers.current[kind];
        }
      };
    };
    const scheduleWorkspaceRefresh: ProjectEdgeActions["scheduleWorkspaceRefresh"] = (opts) => {
      if (stateRef.current.agentTurnBusy && !opts?.force) return;
      perfCounters.recordWorkspaceRefreshScheduled();
      if (opts?.git) pendingRefresh.current.git = true;
      if (opts?.fileTree) pendingRefresh.current.fileTree = true;
      if (!opts || (!opts.git && !opts.fileTree)) {
        pendingRefresh.current.git = true;
        pendingRefresh.current.fileTree = true;
      }

      if (refreshTimer.current) clearTimeout(refreshTimer.current);
      const delay = opts?.delayMs ?? DEFAULT_REFRESH_DELAY_MS;
      refreshTimer.current = setTimeout(flushWorkspaceRefresh, delay);
    };
    const setPreviewPath = (path: string | null) => {
      if (stateRef.current.previewPath !== path) update({ previewPath: path });
    };
    return {
      setGitChanges: (changes) => {
        if (!sameGitChanges(stateRef.current.gitChanges, changes)) update({ gitChanges: changes });
      },
      refreshGitChanges: () => scheduleWorkspaceRefresh({ git: true, delayMs: 0, force: true }),
      registerRefreshGitChanges: (fn) => registerWorkspaceRefresh("git", fn),
      registerWorkspaceRefresh,
      scheduleWorkspaceRefresh,
      setAgentTurnBusy: (busy) => {
        if (stateRef.current.agentTurnBusy !== busy) update({ agentTurnBusy: busy });
      },
      setArtifacts: (paths) => {
        if (!sameStrings(stateRef.current.artifacts, paths)) update({ artifacts: paths });
      },
      setPendingReview: (review) => {
        if (stateRef.current.pendingReview !== review) update({ pendingReview: review });
      },
      setPreviewPath,
      onSelectPath: (path) => {
        setPreviewPath(path);
        selectHandler.current?.(path);
      },
      registerOnSelectPath: (fn) => {
        selectHandler.current = fn;
        return () => {
          if (selectHandler.current === fn) selectHandler.current = null;
        };
      },
    };
  }, [flushWorkspaceRefresh, update]);

  return (
    <ProjectEdgeActionsContext.Provider value={actions}>
      <ProjectEdgeStoreContext.Provider value={store}>{children}</ProjectEdgeStoreContext.Provider>
    </ProjectEdgeActionsContext.Provider>
  );
}

/** Stable actions; never causes a re-render on its own. */
export function useProjectEdgeActions(): ProjectEdgeActions {
  const ctx = useContext(ProjectEdgeActionsContext);
  if (!ctx) throw new Error("useProjectEdgeActions must be used within ProjectEdgeProvider");
  return ctx;
}

/** Subscribe to one slice of edge state; re-renders only when that slice changes. */
export function useProjectEdgeState<T>(selector: (state: ProjectEdgeState) => T): T {
  const store = useContext(ProjectEdgeStoreContext);
  if (!store) throw new Error("useProjectEdgeState must be used within ProjectEdgeProvider");
  return useSyncExternalStore(store.subscribe, () => selector(store.getState()));
}
