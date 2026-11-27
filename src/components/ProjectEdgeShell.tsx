import { useCallback, useEffect, useRef, useState } from "react";
import { useProjectEdgeActions, useProjectEdgeState } from "../contexts/ProjectEdgeContext";
import { useTranslation } from "react-i18next";
import { ideApi } from "../services/tauri/ide";
import { confirmDialog } from "../services/tauri/confirm";
import {
  journalKeep,
  journalListPending,
  journalUndo,
  workspaceExistingFiles,
} from "../services/tauri/chat";
import ProjectEdgePanel from "./ProjectEdgePanel";

interface ProjectEdgeShellProps {
  projectDir: string | null;
  /** Show the floating orb + drawer. State/refresh wiring stays mounted either way. */
  visible?: boolean;
}

/**
 * Agent review drawer shown in CodeZ only.
 * Changes = journaled agent edits awaiting Keep/Undo (not git status);
 * Artifacts = files the agent wrote that still exist on disk.
 */
export default function ProjectEdgeShell({ projectDir, visible = true }: ProjectEdgeShellProps) {
  const { t } = useTranslation();
  const { setGitChanges, registerWorkspaceRefresh, scheduleWorkspaceRefresh, setPendingReview } =
    useProjectEdgeActions();
  const pendingReview = useProjectEdgeState((s) => s.pendingReview);
  const artifacts = useProjectEdgeState((s) => s.artifacts);
  const artifactsSessionId = useProjectEdgeState((s) => s.artifactsSessionId);
  const [undoing, setUndoing] = useState(false);
  const [presentArtifacts, setPresentArtifacts] = useState<string[]>([]);
  const reviewSessionRef = useRef<string | null>(null);
  reviewSessionRef.current = pendingReview?.sessionId ?? null;
  const pendingReviewRef = useRef<{ rel_path: string }[]>([]);
  pendingReviewRef.current = pendingReview?.changes ?? [];
  const artifactsRef = useRef({ paths: artifacts, sessionId: artifactsSessionId });
  artifactsRef.current = { paths: artifacts, sessionId: artifactsSessionId };

  const reloadReview = useCallback(async () => {
    const sid = reviewSessionRef.current;
    if (!projectDir || !sid) return;
    try {
      const changes = await journalListPending(projectDir, sid);
      if (reviewSessionRef.current === sid) setPendingReview({ sessionId: sid, changes });
    } catch {
      /* journal is best-effort */
    }
  }, [projectDir, setPendingReview]);

  const reloadArtifacts = useCallback(async () => {
    const { paths, sessionId } = artifactsRef.current;
    if (!projectDir || paths.length === 0) {
      setPresentArtifacts([]);
      return;
    }
    try {
      const present = new Set(await workspaceExistingFiles(projectDir, sessionId, paths));
      if (artifactsRef.current.paths === paths) {
        setPresentArtifacts(paths.filter((p) => present.has(p)));
      }
    } catch {
      setPresentArtifacts(paths);
    }
  }, [projectDir]);

  useEffect(() => {
    void reloadArtifacts();
  }, [artifacts, artifactsSessionId, reloadArtifacts]);

  // Files that were dirty in git on the previous refresh. A pending agent edit
  // whose file goes dirty -> clean was committed (or reverted) outside the
  // review, so accept it instead of leaving a stale entry. Never-seen-dirty
  // files are left alone: a clean status alone could also mean "not a repo".
  const prevDirtyRef = useRef<Set<string>>(new Set());
  const autoKeepCommitted = useCallback(
    async (changes: { path: string }[]) => {
      const now = new Set(changes.map((c) => c.path.replace(/\\/g, "/")));
      const prev = prevDirtyRef.current;
      prevDirtyRef.current = now;
      const sid = reviewSessionRef.current;
      const pending = pendingReviewRef.current;
      if (!projectDir || !sid || prev.size === 0 || pending.length === 0) return;
      const sameFile = (rel: string, g: string) =>
        rel === g || g.endsWith("/" + rel) || rel.endsWith("/" + g);
      const committed = pending
        .map((c) => c.rel_path.replace(/\\/g, "/"))
        .filter(
          (rel) =>
            [...prev].some((g) => sameFile(rel, g)) && ![...now].some((g) => sameFile(rel, g)),
        );
      if (committed.length === 0) return;
      try {
        await journalKeep(projectDir, sid, committed);
        await reloadReview();
      } catch {
        /* best-effort */
      }
    },
    [projectDir, reloadReview],
  );

  const refreshAll = useCallback(() => {
    const git = projectDir
      ? ideApi
          .gitStatus(projectDir)
          .then(async (changes) => {
            setGitChanges(changes);
            await autoKeepCommitted(changes);
          })
          .catch(() => setGitChanges([]))
      : Promise.resolve(setGitChanges([]));
    return Promise.allSettled([git, reloadReview(), reloadArtifacts()]);
  }, [projectDir, setGitChanges, reloadReview, reloadArtifacts, autoKeepCommitted]);
  useEffect(() => registerWorkspaceRefresh("git", refreshAll), [registerWorkspaceRefresh, refreshAll]);

  useEffect(() => {
    scheduleWorkspaceRefresh({ git: true, force: true, delayMs: 0 });
  }, [projectDir, scheduleWorkspaceRefresh]);

  const undo = useCallback(
    async (paths?: string[]) => {
      if (!projectDir || !pendingReview) return;
      const ok = await confirmDialog(
        paths && paths.length > 0
          ? t("agent.reviewUndoSome", { count: paths.length })
          : t("agent.reviewUndoAll"),
        { title: t("agent.rollbackTurn") },
      );
      if (!ok) return;
      setUndoing(true);
      try {
        await journalUndo(projectDir, pendingReview.sessionId, paths);
        await reloadReview();
        scheduleWorkspaceRefresh({ git: true, fileTree: true, force: true, delayMs: 0 });
      } finally {
        setUndoing(false);
      }
    },
    [projectDir, pendingReview, reloadReview, scheduleWorkspaceRefresh, t],
  );

  const keep = useCallback(
    async (paths?: string[]) => {
      if (!projectDir || !pendingReview) return;
      await journalKeep(projectDir, pendingReview.sessionId, paths);
      await reloadReview();
    },
    [projectDir, pendingReview, reloadReview],
  );

  if (!visible) return null;

  return (
    <ProjectEdgePanel
      projectDir={projectDir}
      artifacts={presentArtifacts}
      onRefresh={() => scheduleWorkspaceRefresh({ git: true, force: true, delayMs: 0 })}
      onUndo={undo}
      onKeep={keep}
      undoing={undoing}
    />
  );
}
