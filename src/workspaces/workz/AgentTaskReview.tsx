import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  agentTaskApi,
  type TaskChangeSet,
  type TaskFileView,
  type AgentTaskInfo,
} from "../../services/tauri/agentTask";
import { diffLines } from "../codez/lineDiff";
import "./AgentTaskReview.css";

function reviewError(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String(error.message);
  }
  return String(error);
}

interface AgentTaskReviewProps {
  projectDir: string;
  task: AgentTaskInfo;
  onClose: () => void;
  /** Called after a merge / discard so the parent can refresh its task list. */
  onResolved: () => void;
}

/**
 * Review the task's current checkout against its frozen base, including dirty
 * and new files. Branch operations remain unavailable while content is uncommitted.
 */
export default function AgentTaskReview(props: AgentTaskReviewProps) {
  const { projectDir, task } = props;
  const identity = JSON.stringify([
    projectDir,
    task.id,
    task.environment_id,
    task.worktree_path,
    task.branch,
    task.base,
    task.base_sha,
  ]);
  return <TaskReviewContent key={identity} {...props} />;
}

function TaskReviewContent({ projectDir, task, onClose, onResolved }: AgentTaskReviewProps) {
  const { t } = useTranslation();
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const [changes, setChanges] = useState<TaskChangeSet["changes"]>([]);
  const [reviewHead, setReviewHead] = useState("");
  const [mergeRecoveryRef, setMergeRecoveryRef] = useState<string | null>(null);
  const [mergeBlocked, setMergeBlocked] = useState(false);
  const [uncommitted, setUncommitted] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [view, setView] = useState<TaskFileView | null>(null);
  const [loading, setLoading] = useState(true);
  const [loaded, setLoaded] = useState(false);
  const [fileLoading, setFileLoading] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [archived, setArchived] = useState(false);
  const [recoveryRoot, setRecoveryRoot] = useState<string | null>(null);

  const base = task.base_sha || task.base || "HEAD";
  // Frozen SHA is for comparison; merge/PR require the named target branch.
  const targetBranch = task.base || "HEAD";

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setReviewHead("");
    setMergeRecoveryRef(null);
    setMergeBlocked(false);
    setArchived(false);
    setRecoveryRoot(null);
    setLoaded(false);
    setError(null);
    setChanges([]);
    setSelected(null);
    setView(null);
    agentTaskApi
      .changeSet(projectDir, task.id)
      .then((set) => {
        if (cancelled) return;
        setChanges(set.changes);
        setReviewHead(set.head_sha ?? "");
        setUncommitted(
          set.has_uncommitted ?? set.changes.some((c) => c.sources.some((s) => s !== "committed")),
        );
        setSelected(set.changes[0]?.path ?? null);
        setLoaded(true);
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(reviewError(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [projectDir, task.id, refresh]);

  useEffect(() => {
    setView(null);
    setFileError(null);
    if (!selected) {
      setFileLoading(false);
      return;
    }
    setFileLoading(true);
    let cancelled = false;
    (async () => {
      try {
        const d = await agentTaskApi.fileView(projectDir, task.id, selected);
        if (cancelled) return;
        setView(d);
      } catch (e) {
        if (!cancelled) setFileError(reviewError(e));
      } finally {
        if (!cancelled) setFileLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selected, projectDir, task.id, refresh]);

  const merge = useCallback(async () => {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const preview = await agentTaskApi.mergePreview(
        projectDir,
        task.id,
        targetBranch,
        reviewHead,
      );
      // A completed preview is not permission to merge a review that has
      // since closed or switched to another task/environment.
      if (!active.current) return;
      const receipt = await agentTaskApi.merge(
        projectDir,
        task.id,
        targetBranch,
        reviewHead,
        preview.target_head,
      );
      if (!active.current) return;
      setMergeRecoveryRef(receipt.recovery_ref);
      setMergeBlocked(true);
      setStatus(t("agent.mergeOk", { base: targetBranch }));
      onResolved();
    } catch (e) {
      if (!active.current) return;
      setError(t("agent.mergeFail", { msg: reviewError(e) }));
      if (typeof e === "object" && e !== null && "recovery_ref" in e && e.recovery_ref) {
        setMergeRecoveryRef(String(e.recovery_ref));
        if ("state" in e && ["needs_recovery", "merged", "unchanged"].includes(String(e.state)))
          setMergeBlocked(true);
      }
    } finally {
      if (active.current) setBusy(false);
    }
  }, [projectDir, task.id, targetBranch, reviewHead, t, onResolved]);

  const discard = useCallback(async () => {
    if (!window.confirm(t("agent.discardConfirm"))) return;
    setBusy(true);
    setError(null);
    try {
      const receipt = await agentTaskApi.discard(
        projectDir,
        task.worktree_path,
        task.branch,
        task.id,
      );
      if (!active.current) return;
      setArchived(true);
      setRecoveryRoot(receipt.retained_root);
      setStatus(t("agent.archiveOk"));
      onResolved();
    } catch (e) {
      if (!active.current) return;
      setError(reviewError(e));
      if (typeof e === "object" && e !== null && "retained_root" in e && e.retained_root) {
        setRecoveryRoot(String(e.retained_root));
        setArchived(true);
      }
    } finally {
      if (active.current) setBusy(false);
    }
  }, [projectDir, task, t, onResolved]);

  const openPr = useCallback(async () => {
    setBusy(true);
    setError(null);
    setStatus(null);
    try {
      const res = await agentTaskApi.openPr(
        projectDir,
        task.branch,
        targetBranch,
        `Agent task ${task.id}`,
        undefined,
      );
      if (!active.current) return;
      if (res.ok && res.url) setStatus(t("agent.prOk", { url: res.url }));
      else setError(t("agent.prFail", { msg: res.message }));
    } catch (e) {
      if (!active.current) return;
      setError(String(e));
    } finally {
      if (active.current) setBusy(false);
    }
  }, [projectDir, task, targetBranch, t]);

  const ops =
    view && !view.binary
      ? diffLines(
          view.original ? view.original.split("\n") : [],
          view.modified ? view.modified.split("\n") : [],
        )
      : [];
  const actionsDisabled =
    archived ||
    busy ||
    !loaded ||
    loading ||
    fileLoading ||
    !!fileError ||
    !view ||
    changes.length === 0;

  return (
    <div className="agentz-task-review-overlay" onClick={onClose}>
      <div className="agentz-task-review" onClick={(e) => e.stopPropagation()}>
        <div className="agentz-task-review-head">
          <div className="agentz-task-review-titles">
            <span className="agentz-task-review-title">{t("agent.reviewTitle")}</span>
            <span className="agentz-task-review-meta">
              {t("agent.reviewBranch")}: <code>{task.branch}</code> · {t("agent.reviewBase")}:{" "}
              <code>{base}</code>
            </span>
          </div>
          <button
            type="button"
            disabled={archived || busy || loading}
            onClick={() => setRefresh((n) => n + 1)}
          >
            {t("agent.reviewRefresh")}
          </button>
          <button type="button" className="agentz-task-review-close" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="agentz-task-review-body">
          <div className="agentz-task-review-files">
            {loading ? (
              <div className="agentz-task-review-empty">{t("common.loading")}</div>
            ) : !loaded ? (
              <div role="alert" className="agentz-task-review-empty">
                {error}
              </div>
            ) : changes.length === 0 ? (
              <div className="agentz-task-review-empty">{t("agent.reviewEmpty")}</div>
            ) : (
              changes.map((c) => (
                <button
                  key={c.path}
                  type="button"
                  className={`agentz-task-review-file ${c.path === selected ? "active" : ""}`}
                  onClick={() => setSelected(c.path)}
                >
                  <span className={`agentz-task-review-badge ${c.status}`}>
                    {c.status.slice(0, 1).toUpperCase()}
                  </span>
                  <span className="agentz-task-review-path" title={c.path}>
                    {c.old_path ? `${c.old_path} → ` : ""}
                    {c.path}
                    <small>
                      {c.sources.map((s) => t(`agent.reviewSource.${s}`)).join(" · ")}
                      {c.binary ? ` · ${t("agent.reviewBinary")}` : ""}
                    </small>
                  </span>
                </button>
              ))
            )}
          </div>
          <div className="agentz-task-review-diff">
            {fileLoading ? (
              <div className="agentz-task-review-empty">{t("common.loading")}</div>
            ) : fileError ? (
              <div role="alert" className="agentz-task-review-empty">
                {fileError}
              </div>
            ) : view?.binary ? (
              <div className="agentz-task-review-empty">{t("agent.reviewBinaryNoPreview")}</div>
            ) : selected && view ? (
              <pre className="agentz-task-review-diff-pre">
                {ops.map((op, i) => (
                  <div key={i} className={`agentz-diff-line ${op.type}`}>
                    <span className="agentz-diff-sign">
                      {op.type === "add" ? "+" : op.type === "remove" ? "-" : " "}
                    </span>
                    {op.text}
                  </div>
                ))}
              </pre>
            ) : (
              <div className="agentz-task-review-empty">{t("agent.reviewSelectFile")}</div>
            )}
          </div>
        </div>

        {view && (
          <div className="agentz-task-review-meta" title={view.review_version}>
            {t("agent.reviewVersion")}:{" "}
            {view.review_version
              .split(":")
              .map((part) => part.slice(0, 8))
              .join(" / ")}
          </div>
        )}
        {uncommitted && (
          <div className="agentz-task-review-status">{t("agent.reviewUncommitted")}</div>
        )}

        {status && <div className="agentz-task-review-status ok">{status}</div>}
        {mergeRecoveryRef && (
          <div className="agentz-task-review-status">
            {t("agent.mergeRecoveryRef")}: <code>{mergeRecoveryRef}</code>
          </div>
        )}
        {recoveryRoot && (
          <div className="agentz-task-review-status">
            {t("agent.archiveRetainedAt")}: <code>{recoveryRoot}</code>
          </div>
        )}
        {error && <div className="agentz-task-review-status err">{error}</div>}

        <div className="agentz-task-review-actions">
          <button
            type="button"
            onClick={discard}
            disabled={archived || busy || !loaded || loading}
            className="danger"
          >
            {t("agent.discard")}
          </button>
          <div className="agentz-task-review-actions-right">
            <button type="button" onClick={openPr} disabled={actionsDisabled || uncommitted}>
              {t("agent.openPr")}
            </button>
            <button
              type="button"
              onClick={merge}
              disabled={actionsDisabled || uncommitted || mergeBlocked || !reviewHead}
              className="primary"
            >
              {busy ? t("agent.merging") : t("agent.merge")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
