import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useProjectEdgeActions, useProjectEdgeState } from "../contexts/ProjectEdgeContext";
import {
  filterArtifactPaths,
  normalizePathSlashes,
  sortArtifactPaths,
  type ArtifactSortMode,
} from "../workspaces/workz/artifactPaths";
import EdgeBookmarkDrawer from "./EdgeBookmarkDrawer";
import "./ProjectEdgePanel.css";

interface ProjectEdgePanelProps {
  projectDir: string | null;
  /** Disk-verified artifact paths; falls back to the raw store list. */
  artifacts?: string[];
  onRefresh?: () => void;
  /** Revert pending agent edits; no paths = all. */
  onUndo?: (paths?: string[]) => Promise<void>;
  /** Accept pending agent edits; no paths = all. */
  onKeep?: (paths?: string[]) => Promise<void>;
  undoing?: boolean;
}

const STATUS_BADGE: Record<string, string> = { added: "A", modified: "M", deleted: "D" };

type EdgeTabId = "changes" | "artifacts";

/** Extract the bare filename from a path (last segment). */
function basename(p: string): string {
  const parts = normalizePathSlashes(p).split("/");
  return parts[parts.length - 1] ?? p;
}

function dirname(p: string): string {
  const parts = normalizePathSlashes(p).split("/");
  return parts.slice(0, -1).join("/");
}

export default function ProjectEdgePanel({
  projectDir,
  artifacts: verifiedArtifacts,
  onRefresh,
  onUndo,
  onKeep,
  undoing = false,
}: ProjectEdgePanelProps) {
  const { t } = useTranslation();
  const { onSelectPath, setPreviewPath } = useProjectEdgeActions();
  const storeArtifacts = useProjectEdgeState((s) => s.artifacts);
  const artifacts = verifiedArtifacts ?? storeArtifacts;
  const pendingReview = useProjectEdgeState((s) => s.pendingReview);
  const previewPath = useProjectEdgeState((s) => s.previewPath);

  const [activeTab, setActiveTab] = useState<EdgeTabId>("changes");
  const [forceDismissed, setForceDismissed] = useState(false);
  const [artifactQuery, setArtifactQuery] = useState("");
  // Default: time sort, newest first — artifacts arrive newest-first from
  // the collectors, so time-desc is just the incoming order.
  const [artifactSort, setArtifactSort] = useState<ArtifactSortMode>("time-desc");

  const pendingChanges = pendingReview?.changes ?? [];
  const changesCount = pendingChanges.length;
  const hasChanges = changesCount > 0;
  const hasArtifacts = artifacts.length > 0;

  const artifactsCount = artifacts.length;

  const badgeCount = useMemo(() => {
    const paths = new Set<string>();
    for (const c of pendingChanges) paths.add(c.rel_path);
    for (const p of artifacts) paths.add(p);
    return paths.size;
  }, [pendingChanges, artifacts]);

  const isPinned = hasChanges && !forceDismissed;
  const isHidden = !hasChanges && !hasArtifacts;

  // Deduped in arrival order (newest first); no alphabetical re-sort here —
  // ordering is the sort control's job below.
  const flatArtifacts = useMemo(() => {
    const seen = new Set<string>();
    const list: { path: string; name: string; dir: string }[] = [];
    for (const p of artifacts) {
      if (seen.has(p)) continue;
      seen.add(p);
      list.push({ path: p, name: basename(p), dir: dirname(p) });
    }
    return list;
  }, [artifacts]);

  const visibleArtifacts = useMemo(() => {
    const filtered = filterArtifactPaths(
      flatArtifacts.map((a) => a.path),
      artifactQuery,
    );
    const byPath = new Map(flatArtifacts.map((a) => [a.path, a]));
    return sortArtifactPaths(filtered, artifactSort)
      .map((p) => byPath.get(p))
      .filter((a): a is { path: string; name: string; dir: string } => Boolean(a));
  }, [flatArtifacts, artifactQuery, artifactSort]);

  const handleClose = useCallback(() => {
    setForceDismissed(true);
    setPreviewPath(null);
  }, [setPreviewPath]);

  useEffect(() => {
    if (!hasChanges && !hasArtifacts) {
      setForceDismissed(false);
    }
  }, [hasChanges, hasArtifacts]);

  useEffect(() => {
    if (hasChanges) setActiveTab("changes");
  }, [hasChanges, pendingReview]);

  // If the changes tab has nothing to show but artifacts exist, land on artifacts.
  useEffect(() => {
    if (activeTab === "changes" && !hasChanges && hasArtifacts) {
      setActiveTab("artifacts");
    }
  }, [activeTab, hasChanges, hasArtifacts]);

  if (!projectDir) return null;

  const changesLabel = t("agent.changes");
  const artifactsLabel = t("agent.artifacts");
  const badgeTitle = `${changesLabel} (${changesCount}) · ${artifactsLabel} (${artifactsCount})`;

  return (
    <div className="agentz-project-edge-panel" aria-label={badgeTitle}>
      <EdgeBookmarkDrawer
        badgeTitle={badgeTitle}
        count={badgeCount}
        top={8}
        pinned={isPinned}
        hidden={isHidden}
        onClose={handleClose}
        closeLabel={t("common.close")}
        tabs={[
          { id: "changes", label: changesLabel, count: changesCount },
          { id: "artifacts", label: artifactsLabel, count: artifactsCount },
        ]}
        activeTab={activeTab}
        onTabChange={(id) => setActiveTab(id as EdgeTabId)}
      >
        {activeTab === "changes" && (
          <>
            <div className="agentz-edge-changes-toolbar">
              {hasChanges && (
                <>
                  <span className="agentz-edge-review-title">
                    {t("chat.reviewChanges", { count: changesCount })}
                  </span>
                  <button
                    type="button"
                    className="agentz-edge-review-btn danger"
                    disabled={undoing}
                    onClick={() => void onUndo?.()}
                  >
                    {undoing ? t("chat.undoing") : t("chat.undoAll")}
                  </button>
                  <button
                    type="button"
                    className="agentz-edge-review-btn"
                    disabled={undoing}
                    onClick={() => void onKeep?.()}
                  >
                    {t("chat.keepAll")}
                  </button>
                </>
              )}
              <button
                type="button"
                className="agentz-edge-refresh"
                onClick={() => onRefresh?.()}
                title={t("agent.refreshChanges")}
              >
                ?
              </button>
            </div>
            {hasChanges ? (
              <ul className="agentz-edge-changes-list journal">
                {pendingChanges.map((c) => {
                  const deleted = c.status === "deleted";
                  return (
                    <li key={c.rel_path} className="agentz-edge-change-row">
                      <button
                        type="button"
                        className="agentz-edge-change"
                        disabled={deleted}
                        onClick={() => onSelectPath(c.rel_path)}
                        title={deleted ? `${c.rel_path} (${t("agent.changeDeleted")})` : c.rel_path}
                      >
                        <span className={`agentz-edge-change-badge ${c.status}`}>
                          {STATUS_BADGE[c.status] ?? "M"}
                        </span>
                        <span className="agentz-edge-change-path">{basename(c.rel_path)}</span>
                      </button>
                      <span className="agentz-edge-change-actions">
                        <button
                          type="button"
                          className="agentz-edge-change-act"
                          disabled={undoing}
                          title={t("chat.undo")}
                          onClick={() => void onUndo?.([c.rel_path])}
                        >
                          ?
                        </button>
                        <button
                          type="button"
                          className="agentz-edge-change-act"
                          disabled={undoing}
                          title={t("chat.keep")}
                          onClick={() => void onKeep?.([c.rel_path])}
                        >
                          ?
                        </button>
                      </span>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <div className="agentz-edge-empty">{t("agent.noChanges")}</div>
            )}
          </>
        )}

        {activeTab === "artifacts" && (
          <>
            {flatArtifacts.length > 0 && (
              <div className="agentz-edge-artifacts-tools">
                <div className="agentz-edge-artifacts-filter">
                  <input
                    type="search"
                    className="agentz-edge-artifacts-search"
                    data-testid="artifact-filter-input"
                    value={artifactQuery}
                    onChange={(e) => setArtifactQuery(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Escape") setArtifactQuery("");
                    }}
                    placeholder={t("agent.artifactSearchPlaceholder")}
                    aria-label={t("agent.artifactSearchPlaceholder")}
                  />
                  {artifactQuery && (
                    <button
                      type="button"
                      className="agentz-edge-artifacts-clear"
                      data-testid="artifact-filter-clear"
                      onClick={() => setArtifactQuery("")}
                      title={t("common.close")}
                    >
                      ✕
                    </button>
                  )}
                </div>
                <select
                  className="agentz-edge-artifacts-sort"
                  data-testid="artifact-sort-select"
                  value={artifactSort}
                  onChange={(e) => setArtifactSort(e.target.value as ArtifactSortMode)}
                  aria-label={t("agent.artifactSortLabel")}
                  title={t("agent.artifactSortLabel")}
                >
                  <option value="time-desc">{t("agent.artifactSortTimeDesc")}</option>
                  <option value="time-asc">{t("agent.artifactSortTimeAsc")}</option>
                  <option value="name-asc">{t("agent.artifactSortNameAsc")}</option>
                  <option value="name-desc">{t("agent.artifactSortNameDesc")}</option>
                  <option value="path-asc">{t("agent.artifactSortPathAsc")}</option>
                </select>
                {artifactQuery.trim() && (
                  <span className="agentz-edge-artifacts-count" data-testid="artifact-filter-count">
                    {t("agent.artifactFilteredCount", {
                      filtered: visibleArtifacts.length,
                      total: flatArtifacts.length,
                    })}
                  </span>
                )}
              </div>
            )}
            {visibleArtifacts.length > 0 ? (
              <ul className="agentz-edge-artifacts-flat">
                {visibleArtifacts.map((a) => (
                  <li key={a.path}>
                    <button
                      type="button"
                      className={`agentz-edge-artifact-item${previewPath === a.path ? " active" : ""}`}
                      onClick={() => onSelectPath(a.path)}
                      title={a.path}
                    >
                      <span className="agentz-edge-artifact-name">{a.name}</span>
                      {a.dir && <span className="agentz-edge-artifact-path">{a.dir}</span>}
                    </button>
                  </li>
                ))}
              </ul>
            ) : (
              <div className="agentz-edge-empty">
                {flatArtifacts.length > 0
                  ? t("agent.noMatchingArtifacts")
                  : t("agent.noArtifacts")}
              </div>
            )}
          </>
        )}
      </EdgeBookmarkDrawer>
    </div>
  );
}
