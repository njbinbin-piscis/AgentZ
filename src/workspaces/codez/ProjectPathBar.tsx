import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import LocationBadge from "../../components/LocationBadge";
import { CloseIcon, FolderIcon } from "../../components/TitleBarIcons";
import type { WorkzProject } from "../../services/tauri/workz";
import { canPortalToBody, useFixedPopupPosition } from "../../utils/popupPosition";

function normProjectPath(p: string): string {
  return p.replace(/\\/g, "/").replace(/\/+$/, "");
}

/** CodeZ's own folder. WorkZ and BotZ keep their directories elsewhere. */
export default function ProjectPathBar({
  projectDir,
  recentProjects,
  onOpenFolder,
  onOpenRecentProject,
  onRemoveRecentProject,
  onCloseProject,
  onRefreshRecent,
}: {
  projectDir: string | null;
  recentProjects: WorkzProject[];
  onOpenFolder: () => void;
  onOpenRecentProject?: (dir: string) => void;
  onRemoveRecentProject?: (dir: string) => void;
  onCloseProject?: () => void;
  onRefreshRecent?: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const popoverStyle = useFixedPopupPosition(open, menuRef, popoverRef, "down", "text", {
    align: "left",
    minWidth: 260,
    maxWidth: 340,
    maxHeight: 340,
  });

  useEffect(() => {
    if (!open) return;
    onRefreshRecent?.();
    const close = (e: MouseEvent) => {
      const target = e.target as Node;
      if (menuRef.current && !menuRef.current.contains(target) && !popoverRef.current?.contains(target)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open, onRefreshRecent]);

  const toggle = () => {
    if (recentProjects.length === 0) onOpenFolder();
    else setOpen((v) => !v);
  };

  return (
    <div className="ide-project-bar">
      <div className="agentz-project-menu" ref={menuRef}>
        <button
          type="button"
          className={`agentz-titlebar-icon agentz-titlebar-folder ${open ? "active" : ""}`}
          onClick={toggle}
          title={projectDir ? t("app.changeFolder") : t("app.openFolder")}
          aria-label={projectDir ? t("app.changeFolder") : t("app.openFolder")}
          aria-expanded={open}
        >
          <FolderIcon />
        </button>
        {open &&
          (() => {
            const node = (
              <div ref={popoverRef} className="agentz-project-popover is-fixed" role="menu" style={popoverStyle}>
                <button
                  type="button"
                  className="agentz-project-popover-pick"
                  role="menuitem"
                  onClick={() => {
                    setOpen(false);
                    onOpenFolder();
                  }}
                >
                  {t("app.chooseFolder")}
                </button>
                {recentProjects.length > 0 && (
                  <>
                    <div className="agentz-project-popover-divider" />
                    <div className="agentz-project-popover-heading">{t("app.recentProjects")}</div>
                    <div className="agentz-project-popover-list">
                      {recentProjects.map((p) => {
                        const isCurrent = Boolean(projectDir && normProjectPath(p.path) === normProjectPath(projectDir));
                        return (
                          <div key={p.path} className={`agentz-project-popover-row ${isCurrent ? "active" : ""}`}>
                            <button
                              type="button"
                              className="agentz-project-popover-item"
                              role="menuitem"
                              title={p.path}
                              onClick={() => {
                                setOpen(false);
                                onOpenRecentProject?.(p.path);
                              }}
                            >
                              <strong>{p.name}</strong>
                              <span>{p.path}</span>
                            </button>
                            {onRemoveRecentProject && (
                              <button
                                type="button"
                                className="agentz-project-popover-remove"
                                title={t("app.removeRecentProject")}
                                aria-label={t("app.removeRecentProject")}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  onRemoveRecentProject(p.path);
                                }}
                              >
                                ×
                              </button>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </>
                )}
              </div>
            );
            return canPortalToBody() ? createPortal(node, document.body) : node;
          })()}
      </div>
      <span
        className={`agentz-project-path ${recentProjects.length > 0 ? "clickable" : ""}`}
        title={projectDir ?? undefined}
        onClick={toggle}
      >
        {projectDir ?? t("app.noProjectOpen")}
      </span>
      {projectDir && onCloseProject && (
        <button
          type="button"
          className="agentz-titlebar-icon agentz-titlebar-close-project"
          onClick={() => void onCloseProject()}
          title={t("app.closeFolder")}
          aria-label={t("app.closeFolder")}
        >
          <CloseIcon size={16} />
        </button>
      )}
      <LocationBadge dir={projectDir} />
    </div>
  );
}
