import { useCallback, useEffect, useRef, useState } from "react";
import { useProjectEdgeActions, useProjectEdgeState } from "../contexts/ProjectEdgeContext";
import { ideApi } from "../services/tauri/ide";
import {
  journalKeep,
  journalListPending,
  journalUndo,
  workspaceExistingFiles,
} from "../services/tauri/chat";
import ProjectEdgePanel from "./ProjectEdgePanel";

interface ProjectEdgeShellProps {
  projectDir: string | null;
}

/**
 * Agent review drawer shared by CodeZ and WorkZ.
 * Changes = journaled agent edits awaiting Keep/Undo (not git status);
 * Artifacts = files the agent wrote that still exist on disk.
 */
export default function ProjectEdgeShell({ projectDir }: ProjectEdgeShellProps) {
  const { setGitChanges, registerWorkspaceRefresh, scheduleWorkspaceRefresh, setPendingReview } =
    useProjectEdgeActions();
  const pendingReview = useProjectEdgeState((s) => s.pendingReview);
  const artifacts = useProjectEdgeState((s) => s.artifacts);
  const artifactsSessionId = useProjectEdgeState((s) => s.artifactsSessionId);
  const [undoing, setUndoing] = useState(false);
  const [presentArtifacts, setPresentArtifacts] = useState<string[]>([]);
  const reviewSessionRef = useRef<string | null>(null);
  reviewSessionRef.current = pendingReview?.sessionId ?? null;
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

  const refreshAll = useCallback(() => {
    const git = projectDir
      ? ideApi
          .gitStatus(projectDir)
          .then(setGitChanges)
          .catch(() => setGitChanges([]))
      : Promise.resolve(setGitChanges([]));
    return Promise.allSettled([git, reloadReview(), reloadArtifacts()]);
  }, [projectDir, setGitChanges, reloadReview, reloadArtifacts]);

  useEffect(() => registerWorkspaceRefresh("git", refreshAll), [registerWorkspaceRefresh, refreshAll]);

  useEffect(() => {
    scheduleWorkspaceRefresh({ git: true, force: true, delayMs: 0 });
  }, [projectDir, scheduleWorkspaceRefresh]);

  const undo = useCallback(
    async (paths?: string[]) => {
      if (!projectDir || !pendingReview) return;
      setUndoing(true);
      try {
        await journalUndo(projectDir, pendingReview.sessionId, paths);
        await reloadReview();
        scheduleWorkspaceRefresh({ git: true, fileTree: true, force: true, delayMs: 0 });
      } finally {
        setUndoing(false);
      }
    },
    [projectDir, pendingReview, reloadReview, scheduleWorkspaceRefresh],
  );

  const keep = useCallback(
    async (paths?: string[]) => {
      if (!projectDir || !pendingReview) return;
      await journalKeep(projectDir, pendingReview.sessionId, paths);
      await reloadReview();
    },
    [projectDir, pendingReview, reloadReview],
  );

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
