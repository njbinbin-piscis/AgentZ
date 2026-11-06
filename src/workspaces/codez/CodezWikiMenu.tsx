import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { WikiIcon } from "../../components/TitleBarIcons";
import {
  graphIndexVisualState,
  type IndexBuildStatus,
} from "../../services/tauri/graphIndex";
import type { WikiBuildAction } from "../workz";
import { canPortalToBody, useFixedPopupPosition } from "../../utils/popupPosition";

/** CodeZ repository wiki and code map. WorkZ and BotZ do not use this. */
export default function CodezWikiMenu({
  projectDir,
  busy,
  graphStatus,
  onOpenCodeMap,
  onAction,
}: {
  projectDir: string | null;
  busy: boolean;
  graphStatus: IndexBuildStatus | null;
  onOpenCodeMap: () => void;
  onAction: (action: WikiBuildAction) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverStyle = useFixedPopupPosition(open, menuRef, popoverRef, "down", "text", {
    align: "right",
    minWidth: 210,
    maxWidth: 320,
    maxHeight: 340,
  });
  const visual = graphStatus ? graphIndexVisualState(graphStatus) : null;

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      const target = e.target as Node;
      if (menuRef.current && !menuRef.current.contains(target) && !popoverRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const title = useMemo(() => {
    if (!graphStatus || busy) return t("agent.repoWikiHint");
    switch (visual) {
      case "indexing":
        return t("agent.repoWikiGraphStatusIndexing");
      case "ready":
        return t("agent.repoWikiGraphStatusReady", { nodes: graphStatus.nodes });
      case "error":
        return graphStatus.last_error ?? t("agent.repoWikiGraphError");
      default:
        return t("agent.repoWikiHint");
    }
  }, [busy, graphStatus, t, visual]);

  const graphLabel = useMemo(() => {
    if (!graphStatus) return null;
    switch (visual) {
      case "indexing":
        return t("agent.repoWikiGraphIndexing");
      case "ready":
        return t("agent.repoWikiGraphReadyShort", { nodes: graphStatus.nodes });
      case "error":
        return t("agent.repoWikiGraphError");
      default:
        return t("agent.repoWikiGraphEmpty");
    }
  }, [graphStatus, t, visual]);

  const run = (action: WikiBuildAction) => {
    setOpen(false);
    onAction(action);
  };

  return (
    <div className="agentz-wiki-menu" ref={menuRef}>
      <button
        type="button"
        className={[
          "agentz-titlebar-icon",
          open ? "active" : "",
          busy ? "loading" : "",
          !busy && visual === "indexing" ? "graph-indexing" : "",
          !busy && visual === "ready" ? "graph-ready" : "",
          !busy && visual === "error" ? "graph-error" : "",
        ]
          .filter(Boolean)
          .join(" ")}
        onClick={() => setOpen((v) => !v)}
        disabled={!projectDir || busy}
        title={title}
        aria-label={t("agent.repoWiki")}
        aria-expanded={open}
        aria-haspopup="menu"
      >
        <WikiIcon />
      </button>
      {open &&
        (() => {
          const node = (
            <div ref={popoverRef} className="agentz-wiki-popover is-fixed" role="menu" style={popoverStyle}>
              <button
                type="button"
                role="menuitem"
                onClick={() => {
                  setOpen(false);
                  onOpenCodeMap();
                }}
              >
                {t("agent.codeMapOpen")}
                <span>{t("agent.codeMapOpenHint")}</span>
              </button>
              <button type="button" role="menuitem" onClick={() => run("overview")}>
                {t("agent.repoWikiOverview")}
                <span>{t("agent.repoWikiOverviewHint")}</span>
              </button>
              <button type="button" role="menuitem" onClick={() => run("deep")}>
                {t("agent.repoWikiDeep")}
                <span>{t("agent.repoWikiDeepHint")}</span>
              </button>
              <button type="button" role="menuitem" onClick={() => run("memory")}>
                {t("agent.repoWikiMemory")}
                <span>{t("agent.repoWikiMemoryHint")}</span>
              </button>
              <button
                type="button"
                role="menuitem"
                className={[
                  visual === "indexing" ? "graph-indexing" : "",
                  visual === "ready" ? "graph-ready" : "",
                  visual === "error" ? "graph-error" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
                onClick={() => run("graph")}
              >
                <span className="agentz-wiki-menu-row">
                  {t("agent.repoWikiGraph")}
                  {graphLabel && <span className="agentz-wiki-graph-status">{graphLabel}</span>}
                </span>
                <span>{t("agent.repoWikiGraphHint")}</span>
              </button>
            </div>
          );
          return canPortalToBody() ? createPortal(node, document.body) : node;
        })()}
    </div>
  );
}
