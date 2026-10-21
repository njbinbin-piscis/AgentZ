import { useRef } from "react";
import { createPortal } from "react-dom";
import { revealInFolder } from "../../services/tauri";
import type { PickedElement } from "../../services/tauri/browser";
import type { WorkzProject } from "../../services/tauri/workz";
import type { EditorSnapshot, LayoutSnapshot } from "../../services/tauri/workspace";
import { canPortalToBody, useContextMenuPosition } from "../../utils/popupPosition";
import FileTree from "./FileTree";
import EditorTabs from "./EditorTabs";
import FileViewer from "./FileViewer";
import SearchPanel from "./SearchPanel";
import GitPanel from "./GitPanel";
import BottomPanel from "./BottomPanel";
import BrowserPanel from "./BrowserPanel";
import { BROWSER_TAB_PATH, isBrowserTab } from "./browserTab";
import { isRecoveryPath, recoveryOriginalPath } from "./editorRecovery";
import ExtensionsManager from "./ExtensionsManager";
import IdeStatusBar from "./IdeStatusBar";
import RemoteConnectDialog from "./RemoteConnectDialog";
import CommandPalette from "./CommandPalette";
import {
  ExplorerIcon,
  SearchIcon,
  SourceControlIcon,
  ExtensionsIcon,
  TerminalIcon,
} from "./ActivityIcons";
import {
  useCodezWorkspace,
  collectSelectedPaths,
  joinProjectPath,
  parentPathForCreate,
} from "./useCodezWorkspace";
import "./IDE.css";

export interface IDEProps {
  projectDir: string | null;
  onOpenFolder: () => void;
  /** Projects known to WorkZ — surfaced on the empty screen for one-click reopen. */
  recentProjects?: WorkzProject[];
  onOpenRecentProject?: (dir: string) => void;
  onRemoveRecentProject?: (dir: string) => void;
  /** Send one or more relative paths from Explorer to the chat composer. */
  onSendToChat?: (paths: string[]) => void;
  /** Send selected terminal output to the chat composer. */
  onSendTerminalToChat?: (text: string) => void;
  /** External request to open a file path inside the IDE (e.g. from Title Bar). */
  openPathRequest?: { path: string; nonce: number } | null;
  onOpenPathRequestHandled?: () => void;
  /** Whether the built-in browser tab is open in the editor area. */
  browserOpen?: boolean;
  onBrowserOpenChange?: (open: boolean) => void;
  /** Send a picked DOM element from the browser tab into the chat composer. */
  onSendElementToChat?: (element: PickedElement) => void;
  /** Attach a base64 PNG screenshot from the browser tab to the chat composer. */
  onScreenshotToChat?: (base64Png: string) => void;
  /** Snapshot to restore when a project opens (loaded by App). */
  workspaceRestore?: {
    key: number;
    editor: EditorSnapshot;
    layout: LayoutSnapshot;
  } | null;
  /** Debounced callback so App can persist editor/layout state to disk. */
  onWorkspacePatch?: (patch: {
    editor: EditorSnapshot;
    layout: Pick<
      LayoutSnapshot,
      | "sidebar_tab"
      | "sidebar_collapsed"
      | "sidebar_width"
      | "bottom_open"
      | "bottom_tab"
      | "bottom_height"
      | "explorer_expanded_paths"
    >;
  }) => void;
  /** Exposes a synchronous reader for the live editor state before navigation. */
  onRegisterNavigationSnapshot?: (reader: (() => EditorSnapshot) | null) => void;
}

export default function CodeZWorkspace(props: IDEProps) {
  const {
    projectDir,
    onOpenFolder,
    recentProjects = [],
    onOpenRecentProject,
    onRemoveRecentProject,
    onSendToChat,
    onSendTerminalToChat,
    browserOpen = false,
    onSendElementToChat,
    onScreenshotToChat,
  } = props;

  const s = useCodezWorkspace(props);
  const fileTreeMenuRef = useRef<HTMLDivElement>(null);
  const fileTreeMenuStyle = useContextMenuPosition(s.fileTreeContextMenu, fileTreeMenuRef);

  return (
    <div className="pond-ide" ref={s.ideRef}>
      <div className="ide-body">
        {/* Activity bar (icon strip) */}
        <div className="ide-activity-bar">
          <button
            className={s.sidebarTab === "explorer" && !s.sidebarCollapsed ? "active" : ""}
            onClick={() => s.switchSidebarTab("explorer")}
            title={s.t("ide.explorer") || "Explorer"}
          >
            <ExplorerIcon />
          </button>
          <button
            className={s.sidebarTab === "search" && !s.sidebarCollapsed ? "active" : ""}
            onClick={() => s.switchSidebarTab("search")}
            title={s.t("ide.search") || "Search"}
          >
            <SearchIcon />
          </button>
          <button
            className={s.sidebarTab === "git" && !s.sidebarCollapsed ? "active" : ""}
            onClick={() => s.switchSidebarTab("git")}
            title={s.t("ide.sourceControl") || "Source Control"}
          >
            <SourceControlIcon />
            {s.gitModified.size + s.gitAdded.size > 0 && (
              <span className="activity-badge">{s.gitModified.size + s.gitAdded.size}</span>
            )}
          </button>
          <button
            className={s.sidebarTab === "extensions" && !s.sidebarCollapsed ? "active" : ""}
            onClick={() => s.switchSidebarTab("extensions")}
            title={s.t("extensions.nav") || "Extensions"}
          >
            <ExtensionsIcon />
          </button>
          <div style={{ flex: 1 }} />
          <button
            className={s.bottomOpen ? "active" : ""}
            disabled={!projectDir}
            onClick={() => {
              if (!projectDir) return;
              s.setBottomOpen((v) => !v);
            }}
            title={projectDir ? s.t("ide.terminal") || "Terminal" : s.t("ide.terminalNeedProject")}
          >
            <TerminalIcon />
          </button>
        </div>

        {/* Sidebar content */}
        {!s.sidebarCollapsed && (
          <div className="ide-sidebar" style={{ width: s.sidebarWidth }}>
            {s.sidebarTab === "extensions" ? (
              <div className="ide-sidebar-section ide-extensions-sidebar">
                <div className="ide-sidebar-title">{s.t("extensions.nav") || "Extensions"}</div>
                <ExtensionsManager projectDir={projectDir} />
              </div>
            ) : !projectDir ? (
              <div className="ide-sidebar-empty">
                <div className="ide-sidebar-empty-title">
                  {s.sidebarTab === "explorer" && (s.t("ide.explorer") || "Explorer")}
                  {s.sidebarTab === "search" && (s.t("ide.search") || "Search")}
                  {s.sidebarTab === "git" && (s.t("ide.sourceControl") || "Source Control")}
                </div>
                <p>{s.t("ide.noProjectDir") || "No folder open."}</p>
                <button type="button" className="ide-open-folder-btn" onClick={onOpenFolder}>
                  {s.t("ide.openFolder") || "Open Folder"}
                </button>
                <button
                  type="button"
                  className="ide-open-folder-btn ide-open-remote-btn"
                  onClick={() => s.setRemoteDialogOpen(true)}
                >
                  {s.t("remote.dialog.open")}
                </button>
                <p className="ide-sidebar-empty-hint">{s.t("ide.noProjectDirHint")}</p>
              </div>
            ) : (
              <>
                {s.sidebarTab === "explorer" && (
                  <FileTree
                    nodes={s.fileTree}
                    activePath={s.fileTreeFocusPath}
                    selectedPaths={s.fileTreeSelection}
                    gitModified={s.gitModified}
                    gitAdded={s.gitAdded}
                    projectDir={projectDir}
                    onFileClick={(node) => s.openFile(node.path)}
                    onRefresh={() =>
                      s.scheduleWorkspaceRefresh({ git: true, fileTree: true, force: true, delayMs: 0 })
                    }
                    onSelect={s.handleFileTreeSelect}
                    onContextMenu={s.handleFileTreeContextMenu}
                    containerRef={s.fileTreeRef}
                    expandedPaths={s.explorerExpanded}
                    onExpandedPathsChange={s.setExplorerExpanded}
                  />
                )}
                {s.sidebarTab === "search" && (
                  <SearchPanel
                    projectDir={projectDir}
                    onResultClick={(path, line, column) => s.revealInFile(path, line, column)}
                  />
                )}
                {s.sidebarTab === "git" && (
                  <GitPanel
                    projectDir={projectDir}
                    onDiffClick={(path) => s.openDiff(path)}
                    onOpenFile={(path) => s.openFile(path)}
                    onRefresh={async () => {
                      s.scheduleWorkspaceRefresh({ git: true, force: true, delayMs: 0 });
                    }}
                  />
                )}
              </>
            )}
          </div>
        )}

        {/* Sidebar resize handle */}
        {!s.sidebarCollapsed && (
          <div className="ide-resize-handle-h" onMouseDown={s.startSidebarResize} />
        )}

        {/* Editor area */}
        <div className="ide-editor-area">
          <EditorTabs
            tabs={s.headerTabs}
            activeTabPath={s.activeTabPath}
            onTabClick={s.setActiveTabPath}
            onTabClose={s.handleTabClose}
            onSave={s.saveFile}
            onTabContextMenu={s.handleTabContextMenu}
            onCloseAll={s.closeAllTabs}
            onCloseSaved={s.closeSavedTabs}
            onCloseOther={s.closeOtherTabs}
            onReloadFromDisk={(path) => void s.reloadTabFromDisk(path)}
            contextMenu={s.tabContextMenu}
            onDismissContextMenu={() => s.setTabContextMenu(null)}
          />
          <div className="ide-editor">
            {!projectDir ? (
              <div className="ide-no-project">
                <div className="icon">📂</div>
                <div>{s.t("ide.noProjectDir") || "No folder open."}</div>
                <button
                  type="button"
                  className="ide-open-folder-btn ide-open-folder-btn-lg"
                  onClick={onOpenFolder}
                >
                  {s.t("ide.openFolder") || "Open Folder"}
                </button>
                <button
                  type="button"
                  className="ide-open-folder-btn ide-open-folder-btn-lg ide-open-remote-btn"
                  onClick={() => s.setRemoteDialogOpen(true)}
                >
                  {s.t("remote.dialog.open")}
                </button>
                <div style={{ fontSize: 12, opacity: 0.6 }}>{s.t("ide.noProjectDirHint")}</div>
                {recentProjects.length > 0 && onOpenRecentProject && (
                  <div className="ide-recent-projects">
                    <div className="ide-recent-projects-title">{s.t("app.recentProjects")}</div>
                    <div className="ide-recent-projects-list">
                      {recentProjects.slice(0, 8).map((p) => (
                        <div key={p.path} className="ide-recent-project-row">
                          <button
                            type="button"
                            className="ide-recent-project-item"
                            title={p.path}
                            onClick={() => onOpenRecentProject(p.path)}
                          >
                            <span className="ide-recent-project-name">{p.name}</span>
                            <span className="ide-recent-project-path">{p.path}</span>
                          </button>
                          {onRemoveRecentProject && (
                            <button
                              type="button"
                              className="ide-recent-project-remove"
                              title={s.t("app.removeRecentProject")}
                              aria-label={s.t("app.removeRecentProject")}
                              onClick={() => onRemoveRecentProject(p.path)}
                            >
                              ×
                            </button>
                          )}
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            ) : s.fileLoading ? (
              <div className="ide-file-loading">
                <div className="ide-file-loading-spinner" />
                <div>
                  {s.t("ide.openingFile", {
                    name: s.fileLoading.split("/").pop() || s.fileLoading,
                  })}
                </div>
              </div>
            ) : s.fileLoadError ? (
              <div className="ide-file-error">
                <div className="ide-file-error-title">{s.t("ide.fileErrorTitle")}</div>
                <div className="ide-file-error-msg">{s.fileLoadError}</div>
                <button
                  type="button"
                  className="ide-open-folder-btn"
                  onClick={() => s.setFileLoadError(null)}
                >
                  {s.t("common.dismiss")}
                </button>
              </div>
            ) : isBrowserTab(s.activeTabPath) && browserOpen ? (
              <BrowserPanel
                onClose={() => s.handleTabClose(BROWSER_TAB_PATH)}
                onSendElementToChat={(el) => onSendElementToChat?.(el)}
                onScreenshotToChat={(base64) => onScreenshotToChat?.(base64)}
                chatEnabled={Boolean(projectDir)}
                refreshSignal={s.browserRefreshSignal}
                agentAction={s.agentBrowserAction}
                onRequestClose={s.requestBrowserClose}
              />
            ) : s.activeTab && isRecoveryPath(s.activeTab.path) ? (
              <section
                aria-label="Recovered unsaved draft"
                style={{ padding: 16, overflow: "auto", height: "100%" }}
              >
                <h3>Recovered unsaved draft</h3>
                <p>Original file: {recoveryOriginalPath(s.activeTab.path) ?? "Unknown"}</p>
                <p>
                  This read-only copy preserves an older draft. Copy the text to review it; the
                  current file has not been replaced.
                </p>
                <button
                  type="button"
                  onClick={() => {
                    void navigator.clipboard
                      .writeText(s.activeTab!.content)
                      .catch(() =>
                        s.setFileLoadError(
                          "Could not copy the recovered draft. Select and copy its text manually.",
                        ),
                      );
                  }}
                >
                  Copy draft
                </button>
                <textarea
                  readOnly
                  aria-label="Recovered draft text"
                  value={s.activeTab.content}
                  style={{ width: "100%", height: "70%", display: "block", marginTop: 12 }}
                />
              </section>
            ) : s.activeTab ? (
              <FileViewer
                tab={s.activeTab}
                projectDir={projectDir}
                reveal={s.reveal && s.reveal.path === s.activeTab.path ? s.reveal : null}
                onChange={s.handleEditorChange}
                onViewModeChange={(mode) => s.setTabViewMode(s.activeTab!.path, mode)}
                onSave={() => {
                  const p = s.activeTabPathRef.current;
                  if (p && !p.startsWith("diff:")) s.saveFile(p);
                }}
                conflict={
                  s.diskConflicts[s.activeTab.path]
                    ? {
                        fileName: s.activeTab.name || s.activeTab.path,
                        onViewDiff: () => s.handleViewConflictDiff(s.activeTab!.path),
                        onAcceptDisk: () => s.handleAcceptDiskConflict(s.activeTab!.path),
                        onKeepMine: () => s.handleKeepMineConflict(s.activeTab!.path),
                      }
                    : null
                }
              />
            ) : (
              <div className="ide-editor-welcome">
                <div className="welcome-logo welcome-logo-text">CodeZ</div>
                <div className="welcome-title">
                  {s.t("ide.welcome") || "Select a file to start editing"}
                </div>
                <div className="welcome-hint">{s.t("ide.welcomeHint")}</div>
              </div>
            )}
          </div>

          {projectDir && (
            <>
              {s.bottomOpen && (
                <div className="ide-resize-handle-v" onMouseDown={s.startBottomResize} />
              )}
              <BottomPanel
                projectDir={projectDir}
                open={s.bottomOpen}
                activeTab={s.bottomTab}
                onTabChange={s.setBottomTab}
                onClose={() => s.setBottomOpen(false)}
                height={s.bottomHeight}
                onSendTerminalToChat={onSendTerminalToChat}
              />
            </>
          )}
        </div>
      </div>

      {/* Full-width application status bar */}
      <IdeStatusBar
        projectDir={projectDir}
        onOpenPanel={s.openBottomPanel}
        onOpenExtensions={s.openExtensionsSidebar}
        onOpenRemote={() => s.setRemoteDialogOpen(true)}
      />
      {s.remoteDialogOpen && (
        <RemoteConnectDialog
          projectDir={projectDir}
          onClose={() => s.setRemoteDialogOpen(false)}
        />
      )}
      <CommandPalette
        open={s.paletteOpen}
        initialMode={s.paletteMode}
        fileTree={s.fileTree}
        recentPaths={s.tabs.map((tab) => tab.path)}
        commands={s.paletteCommands}
        onSelectFile={(path) => void s.openFile(path)}
        onClose={() => s.setPaletteOpen(false)}
      />

      {/* File tree right-click context menu */}
      {s.fileTreeContextMenu &&
        (() => {
          const menuNode = (
            <div
              ref={fileTreeMenuRef}
              className="ide-tab-context-menu"
              style={fileTreeMenuStyle}
              onMouseDown={(e) => e.stopPropagation()}
              onClick={(e) => e.stopPropagation()}
            >
              <button
                onClick={() => {
                  s.openFile(s.fileTreeContextMenu!.targetPath);
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.openFile") || "Open"}
              </button>
              {onSendToChat && (
                <button
                  onClick={() => {
                    const paths =
                      s.fileTreeSelection.size > 1 &&
                      s.fileTreeSelection.has(s.fileTreeContextMenu!.targetPath)
                        ? collectSelectedPaths(s.fileTree, s.fileTreeSelection)
                        : [
                            s.fileTreeContextMenu!.isDir
                              ? s.fileTreeContextMenu!.targetPath.endsWith("/")
                                ? s.fileTreeContextMenu!.targetPath
                                : `${s.fileTreeContextMenu!.targetPath}/`
                              : s.fileTreeContextMenu!.targetPath,
                          ];
                    if (paths.length > 0) onSendToChat(paths);
                    s.setFileTreeContextMenu(null);
                  }}
                >
                  {s.t("ide.sendToChat")}
                </button>
              )}
              <button
                onClick={() => {
                  s.fileTreeRef.current?.renameActive?.();
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.renameFile") || "Rename"}{" "}
                <span style={{ opacity: 0.5, fontSize: 11, marginLeft: 8 }}>F2</span>
              </button>
              <button
                onClick={() => {
                  s.fileTreeRef.current?.deleteSelected?.();
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.deleteFile") || "Delete"}{" "}
                <span style={{ opacity: 0.5, fontSize: 11, marginLeft: 8 }}>Del</span>
              </button>
              <div className="ide-tab-context-menu-sep" />
              <button
                onClick={() => {
                  const dir = s.projectDirRef.current;
                  if (dir)
                    navigator.clipboard
                      .writeText(`${dir}/${s.fileTreeContextMenu!.targetPath}`)
                      .catch(() => {});
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.copyPath") || "Copy Path"}
              </button>
              <button
                onClick={() => {
                  navigator.clipboard.writeText(s.fileTreeContextMenu!.targetPath).catch(() => {});
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.copyRelPath") || "Copy Relative Path"}
              </button>
              <button
                onClick={() => {
                  const dir = s.projectDirRef.current;
                  if (!dir || !s.fileTreeContextMenu) return;
                  const abs = joinProjectPath(dir, s.fileTreeContextMenu.targetPath);
                  void revealInFolder(abs).catch((e: unknown) => {
                    window.alert(String(e));
                  });
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.revealInExplorer") || "Reveal in File Manager"}
              </button>
              <div className="ide-tab-context-menu-sep" />
              <button
                onClick={() => {
                  if (!s.fileTreeContextMenu) return;
                  const parent = parentPathForCreate(
                    s.fileTreeContextMenu.targetPath,
                    s.fileTreeContextMenu.isDir,
                  );
                  s.fileTreeRef.current?.startCreate?.(false, parent);
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.newFile") || "New File"}
              </button>
              <button
                onClick={() => {
                  if (!s.fileTreeContextMenu) return;
                  const parent = parentPathForCreate(
                    s.fileTreeContextMenu.targetPath,
                    s.fileTreeContextMenu.isDir,
                  );
                  s.fileTreeRef.current?.startCreate?.(true, parent);
                  s.setFileTreeContextMenu(null);
                }}
              >
                {s.t("ide.newFolder") || "New Folder"}
              </button>
            </div>
          );
          return canPortalToBody() ? createPortal(menuNode, document.body) : menuNode;
        })()}
    </div>
  );
}
