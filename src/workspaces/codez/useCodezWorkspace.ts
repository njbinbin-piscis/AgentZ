import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { useTranslation } from "react-i18next";
import {
  type FileTreeContextMenu,
  joinProjectPath,
  parentPathForCreate,
  defaultExplorerExpanded,
} from "./FileTree";
import type { BottomTab } from "./BottomPanel";
import type { PaletteMode } from "./CommandPalette";
import * as monaco from "monaco-editor";
import { ideApi, onFileChanged } from "../../services/tauri/ide";
import { recordUserRecentEdit } from "../../services/tauri/chat";
import { getBgTerminal, subscribeBgTerminal } from "../../services/bgTerminal";
import { extHostResourceSnapshot } from "../../services/tauri/extHostResource";
import { graphIndexResourceSnapshot } from "../../services/tauri/graphIndexResource";
import { fileUriString } from "../../services/tauri/editorUri";
import { BROWSER_TAB_PATH, isBrowserTab } from "./browserTab";
import {
  browserClose,
  browserCloseGuard,
  onBrowserChanged,
  type PickedElement,
} from "../../services/tauri/browser";
import {
  CHAT_EVENT,
  type AgentEvent,
  type ChatEventEnvelope,
} from "../../services/tauri/chat";
import { listen } from "@tauri-apps/api/event";
import type { FileNode, OpenTab, TabViewMode } from "./types";
import type { EditorSnapshot, LayoutSnapshot } from "../../services/tauri/workspace";
import { editorSnapshotFromTabs } from "../../services/tauri/workspace";
import { isPreviewablePath } from "./FileIcon";
import { useProjectEdgeActions, useProjectEdgeState } from "../../contexts/ProjectEdgeContext";
import { normalizeRelPath, shouldWatchPath } from "../../utils/pathFilter";
import {
  relativizeArtifactPath,
  resolveArtifactFullPath,
} from "../workz/artifactPaths";
import { perfCounters } from "../../utils/perfCounters";
import { startOwnedWatcher } from "./watcherLifecycle";
import { EditorConflictTracker, type CapturedOperation, type DiskConflictInfo } from "./editorConflict";
import { isRecoveryPath, mergeRestoredTabs, restoreRecoveryTab } from "./editorRecovery";
import { diskStamps as diskStampsStore } from "../../services/diskStamps";
import "./IDE.css";

type SidebarTab = "explorer" | "search" | "git" | "extensions";

/** Right-click context menu state (shown over a tab). */
interface TabContextMenu {
  x: number;
  y: number;
  /** Path of the tab that was right-clicked. */
  targetPath: string;
}

interface IDEProps {
  projectDir: string | null;
  onOpenFolder: () => void;
  recentProjects?: { path: string; name: string }[];
  onOpenRecentProject?: (dir: string) => void;
  onRemoveRecentProject?: (dir: string) => void;
  /** Insert file/dir chips into the chat composer (opens chat if needed). */
  onSendToChat?: (paths: string[]) => void;
  /** Insert a terminal selection chip into the chat composer. */
  onSendTerminalToChat?: (text: string) => void;
  /** Open a workspace-relative file when the path or nonce changes. */
  openPathRequest?: { path: string; nonce: number } | null;
  /** Called after `openPathRequest` has been consumed (clears parent state). */
  onOpenPathRequestHandled?: () => void;
  /** Embedded browser tab (editor area, not overlay). */
  browserOpen?: boolean;
  onBrowserOpenChange?: (open: boolean) => void;
  onSendElementToChat?: (el: PickedElement) => void;
  onScreenshotToChat?: (base64: string) => void;
  /** One-shot editor restore after app startup (hot exit). */
  workspaceRestore?: { key: number; editor: EditorSnapshot; layout: LayoutSnapshot } | null;
  /** Register a synchronous live snapshot getter for navigation preflight. */
  onRegisterNavigationSnapshot?: (read: (() => EditorSnapshot) | null) => void;
  /** Debounced editor + IDE layout patch for workspace persistence. */
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
}

/** Handle to the imperative methods exposed by FileTree via its root ref. */
interface FileTreeHandle {
  deleteSelected?: () => void;
  renameActive?: () => void;
  startCreate?: (isDir: boolean, parentPathOverride?: string) => void;
}

function collectSelectedPaths(nodes: FileNode[], selected: Set<string>): string[] {
  const out: string[] = [];
  const walk = (list: FileNode[]) => {
    for (const n of list) {
      if (selected.has(n.path)) {
        out.push(n.is_dir ? (n.path.endsWith("/") ? n.path : `${n.path}/`) : n.path);
      }
      if (n.children) walk(n.children);
    }
  };
  walk(nodes);
  return out;
}
import { useCodezShortcuts } from "./useCodezShortcuts";
export { collectSelectedPaths, joinProjectPath, parentPathForCreate };
export function useCodezWorkspace({
  projectDir,
  onOpenFolder,
  openPathRequest,
  onOpenPathRequestHandled,
  browserOpen = false,
  onBrowserOpenChange,
  workspaceRestore = null,
  onWorkspacePatch,
  onRegisterNavigationSnapshot,
}: IDEProps) {
  const { t } = useTranslation();

  // File tree
  const [fileTree, setFileTree] = useState<FileNode[]>([]);

  // Open tabs
  const [tabs, setTabsState] = useState<OpenTab[]>([]);
  const [activeTabPath, setActiveTabPath] = useState<string | null>(null);

  // Pending cursor reveal (search "go to result" / navigation). Applied to the
  // active editor; `nonce` re-triggers reveal even for the same position.
  const [reveal, setReveal] = useState<{
    path: string;
    line: number;
    column: number;
    nonce: number;
  } | null>(null);
  const [fileLoading, setFileLoading] = useState<string | null>(null);
  const [fileLoadError, setFileLoadError] = useState<string | null>(null);
  const [diskConflicts, setDiskConflicts] = useState<Record<string, DiskConflictInfo>>({});

  const { registerOnSelectPath, scheduleWorkspaceRefresh, registerWorkspaceRefresh } =
    useProjectEdgeActions();
  const gitChanges = useProjectEdgeState((s) => s.gitChanges);
  const agentTurnBusy = useProjectEdgeState((s) => s.agentTurnBusy);

  const { gitModified, gitAdded } = useMemo(() => {
    const modified = new Set<string>();
    const added = new Set<string>();
    for (const s of gitChanges) {
      if (s.status === "modified") modified.add(s.path);
      else if (s.status === "added" || s.status === "untracked") added.add(s.path);
    }
    return { gitModified: modified, gitAdded: added };
  }, [gitChanges]);

  // UI state — unified bottom panel (terminal + extension consoles)
  const [bottomOpen, setBottomOpen] = useState(false);
  const [bottomTab, setBottomTab] = useState<BottomTab>("terminal");
  const [sidebarTab, setSidebarTab] = useState<SidebarTab>("explorer");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  // Resizable panel widths
  const [sidebarWidth, setSidebarWidth] = useState(260);
  const [bottomHeight, setBottomHeight] = useState(240);
  const ideRef = useRef<HTMLDivElement>(null);
  const [browserRefreshSignal, setBrowserRefreshSignal] = useState(0);
  const [agentBrowserAction, setAgentBrowserAction] = useState<string | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteMode, setPaletteMode] = useState<PaletteMode>("files");

  const openBottomPanel = useCallback((tab: BottomTab) => {
    setBottomTab(tab);
    setBottomOpen(true);
  }, []);

  // "Show in terminal" on a background task opens its live output tab.
  useEffect(
    () =>
      subscribeBgTerminal(() => {
        if (getBgTerminal()) openBottomPanel("bgtask");
      }),
    [openBottomPanel],
  );

  const openExtensionsSidebar = useCallback(() => {
    setSidebarTab("extensions");
    setSidebarCollapsed(false);
  }, []);

  const browserTab = useMemo(
    (): OpenTab => ({
      path: BROWSER_TAB_PATH,
      name: t("browser.tabTitle"),
      language: null,
      content: "",
      isDirty: false,
      isReadOnly: true,
    }),
    [t],
  );

  const headerTabs = useMemo(() => {
    if (browserOpen && projectDir) return [browserTab, ...tabs];
    return tabs;
  }, [browserOpen, projectDir, browserTab, tabs]);

  useEffect(() => {
    if (browserOpen && projectDir) {
      setActiveTabPath(BROWSER_TAB_PATH);
      return;
    }
    setActiveTabPath((cur) =>
      isBrowserTab(cur) ? tabsRef.current[0]?.path ?? null : cur,
    );
  }, [browserOpen, projectDir]);

  const requestBrowserClose = useCallback(async (): Promise<boolean> => {
    try {
      const guard = await browserCloseGuard();
      if (guard.can_close && !guard.agent_active) return true;
      const msg = guard.reason ?? t("browser.closeConfirm");
      // eslint-disable-next-line no-alert
      return window.confirm(msg);
    } catch {
      // eslint-disable-next-line no-alert
      return window.confirm(t("browser.closeConfirm"));
    }
  }, [t]);

  useEffect(() => {
    if (!projectDir) return;
    let unlistenChat: (() => void) | undefined;
    let disposed = false;
    void listen<ChatEventEnvelope>(CHAT_EVENT, (ev) => {
      const env = ev.payload;
      if (env.channel !== "agent_event") return;
      const payload = env.payload as AgentEvent;
      if (payload.type === "tool_start" && payload.name === "browser") {
        onBrowserOpenChange?.(true);
        const input = payload.input as Record<string, unknown> | null;
        const action = String(input?.action ?? "browser");
        const detail =
          (input?.url as string | undefined) ??
          (input?.ref as string | undefined) ??
          (input?.selector as string | undefined);
        setAgentBrowserAction(
          detail ? `${action} → ${String(detail).slice(0, 40)}` : action,
        );
      }
      if (payload.type === "tool_end" && payload.name === "browser") {
        setBrowserRefreshSignal((n) => n + 1);
        setAgentBrowserAction(null);
      }
    }).then((raw) => {
      const fn = perfCounters.trackListener(raw);
      if (disposed) fn();
      else unlistenChat = fn;
    });
    return () => {
      disposed = true;
      unlistenChat?.();
    };
  }, [projectDir, onBrowserOpenChange]);

  useEffect(() => {
    if (!browserOpen) return;
    let unlistenChanged: (() => void) | undefined;
    let disposed = false;
    void onBrowserChanged(() => {
      setBrowserRefreshSignal((n) => n + 1);
    }).then((raw) => {
      const fn = perfCounters.trackListener(raw);
      if (disposed) fn();
      else unlistenChanged = fn;
    });
    return () => {
      disposed = true;
      unlistenChanged?.();
    };
  }, [browserOpen]);

  // Right-click context menu for tab headers
  const [tabContextMenu, setTabContextMenu] = useState<TabContextMenu | null>(null);

  // File tree: selected paths (Ctrl/Cmd multi-select) + right-click menu.
  const [fileTreeSelection, setFileTreeSelection] = useState<Set<string>>(new Set());
  const [explorerExpanded, setExplorerExpanded] = useState<Set<string>>(new Set());
  const explorerExpandedInitRef = useRef(false);
  const [fileTreeContextMenu, setFileTreeContextMenu] = useState<FileTreeContextMenu | null>(null);
  const [remoteDialogOpen, setRemoteDialogOpen] = useState(false);
  const fileTreeRef = useRef<(HTMLDivElement & FileTreeHandle) | null>(null);

  // Stable refs so keyboard shortcuts / beforeunload always read the latest
  // state without re-registering listeners on every render.
  const tabsRef = useRef<OpenTab[]>(tabs);
  const conflicts = useRef(new EditorConflictTracker());
  // Digest of the disk bytes each open file was last loaded from or saved as. Saves send it back
  // so the backend refuses to overwrite a file that changed on disk in the meantime.
  const diskStamps = useRef(diskStampsStore);
  const editorGeneration = useRef(0);
  const tabMutation = useRef(0);
  const openRequests = useRef(new Map<string, number>());
  const openSequence = useRef(0);
  const liveProject = useRef<{ value: string | null } | null>(null);
  if (!liveProject.current || liveProject.current.value !== projectDir) {
    liveProject.current = { value: projectDir };
    editorGeneration.current += 1;
    conflicts.current.setScope(projectDir);
    tabsRef.current = [];
    openRequests.current.clear();
  }
  const setTabs = useCallback((update: OpenTab[] | ((previous: OpenTab[]) => OpenTab[])) => {
    const previous = tabsRef.current;
    const next = typeof update === "function" ? update(previous) : update;
    for (const tab of previous) if (!next.some(row => row.path === tab.path)) {
      conflicts.current.close(tab.path);
      openRequests.current.delete(tab.path);
      setDiskConflicts((prev) => {
        if (!(tab.path in prev)) return prev;
        const copy = { ...prev };
        delete copy[tab.path];
        return copy;
      });
    }
    if (liveProject.current?.value) for (const tab of next) {
      const captured = conflicts.current.snapshot(tab.path);
      if (!captured) conflicts.current.open(tab.path, tab.content, tab.isDirty);
      else if (captured.content !== tab.content || (tab.isDirty && !captured.isDirty)) conflicts.current.edit(tab.path, tab.content);
    }
    tabMutation.current += 1;
    tabsRef.current = next;
    setTabsState(next);
  }, []);
  const activeTabPathRef = useRef<string | null>(activeTabPath);
  activeTabPathRef.current = activeTabPath;
  useEffect(() => {
    onRegisterNavigationSnapshot?.(() => editorSnapshotFromTabs(tabsRef.current, activeTabPathRef.current));
    return () => onRegisterNavigationSnapshot?.(null);
  }, [onRegisterNavigationSnapshot]);
  const projectDirRef = useRef<string | null>(projectDir);
  projectDirRef.current = projectDir;
  const handledOpenPathNonce = useRef(0);
  const openFileRef = useRef<(path: string, readOnly?: boolean) => Promise<void>>(async () => {});

  useEffect(() => {
    return registerOnSelectPath((path) => void openFileRef.current(path));
  }, [registerOnSelectPath]);

  const prevProjectDirRef = useRef<string | null>(null);
  const handledRestoreKeyRef = useRef(0);

  // Reset editor state when switching projects or closing the folder.
  useEffect(() => {
    handledOpenPathNonce.current = 0;
    setFileLoading(null);
    if (!projectDir) {
      prevProjectDirRef.current = null;
      setTabs([]);
      setActiveTabPath(null);
      setFileLoadError(null);
      setReveal(null);
      setFileTree([]);
      setFileTreeSelection(new Set());
      setExplorerExpanded(new Set());
      explorerExpandedInitRef.current = false;
      setBottomOpen(false);
      setBottomTab("terminal");
      return;
    }
    const prev = prevProjectDirRef.current;
    // Match the exact scope identity used by the operation tracker, including
    // path spelling changes. Do not retain rendered tabs with an invalidated ref.
    if (prev && prev !== projectDir) {
      setTabs([]);
      setActiveTabPath(null);
      setFileLoadError(null);
      setReveal(null);
      setFileTreeSelection(new Set());
      setExplorerExpanded(new Set());
      explorerExpandedInitRef.current = false;
      setBottomOpen(false);
      setBottomTab("terminal");
    }
    prevProjectDirRef.current = projectDir;
  }, [projectDir, setTabs]);

  // Hot-exit restore: reopen tabs and reapply unsaved buffers.
  useEffect(() => {
    if (!projectDir || !workspaceRestore) return;
    if (workspaceRestore.key <= handledRestoreKeyRef.current) return;
    handledRestoreKeyRef.current = workspaceRestore.key;

    const { editor, layout } = workspaceRestore;
    const capturedGeneration = editorGeneration.current;
    const capturedMutation = tabMutation.current;
    if (layout.sidebar_tab === "explorer" || layout.sidebar_tab === "search" || layout.sidebar_tab === "git" || layout.sidebar_tab === "extensions") {
      setSidebarTab(layout.sidebar_tab);
    }
    setSidebarCollapsed(layout.sidebar_collapsed);
    if (layout.sidebar_width >= 220) setSidebarWidth(layout.sidebar_width);
    if (layout.explorer_expanded_paths?.length) {
      setExplorerExpanded(new Set(layout.explorer_expanded_paths));
      explorerExpandedInitRef.current = true;
    }
    setBottomOpen(layout.bottom_open);
    const bottomTabs: BottomTab[] = ["terminal", "output", "debug", "scm", "tests", "views", "webviews"];
    if (bottomTabs.includes(layout.bottom_tab as BottomTab)) {
      setBottomTab(layout.bottom_tab as BottomTab);
    }
    if (layout.bottom_height >= 120) setBottomHeight(layout.bottom_height);

    void (async () => {
      const restored: OpenTab[] = [];
      let restoreFailed = false;
      const previewPaths = new Set(editor.preview_paths ?? []);
      for (const path of editor.open_paths) {
        const fullPath = `${projectDir}/${path}`;
        const dirty = editor.dirty_buffers[path];
        if (isRecoveryPath(path)) {
          const backup = restoreRecoveryTab(path, dirty);
          if (backup) restored.push(backup);
          else restoreFailed = true;
          continue;
        }
        try {
          const fc = await ideApi.readFile(fullPath);
          if (fc.sha256) diskStamps.current.set(fullPath, fc.sha256);
          if (fc.is_binary) {
            restoreFailed = true;
            if (dirty !== undefined) restored.push({ path, name: path.split("/").pop() || path,
              language: null, content: dirty, isDirty: true, isReadOnly: false, viewMode: "editor" });
            continue;
          }
          restored.push({
            path,
            name: path.split("/").pop() || path,
            language: fc.language,
            content: dirty !== undefined ? dirty : fc.content,
            isDirty: dirty !== undefined,
            isReadOnly: false,
            viewMode: previewPaths.has(path) && isPreviewablePath(path) ? "preview" : "editor",
            fileSize: fc.size,
          });
        } catch {
          restoreFailed = true;
          if (dirty !== undefined) {
            restored.push({
              path,
              name: path.split("/").pop() || path,
              language: null,
              content: dirty,
              isDirty: true,
              isReadOnly: false,
              viewMode: "editor",
            });
          }
        }
      }
      if (capturedGeneration !== editorGeneration.current) return;
      if (capturedMutation !== tabMutation.current) {
        // Preserve newer intent and retain older drafts as separate recovery tabs.
        setTabs(prev => mergeRestoredTabs(prev, restored));
        if (restoreFailed) setFileLoadError("Some restored files could not be read. Available unsaved buffers have been retained.");
        return;
      }
      setTabs(restored);
      if (restoreFailed) setFileLoadError("Some restored files could not be read. Available unsaved buffers have been retained.");
      const active =
        editor.active_path && restored.some((t) => t.path === editor.active_path)
          ? editor.active_path
          : restored[0]?.path ?? null;
      setActiveTabPath(active);
    })();
  }, [projectDir, workspaceRestore, setTabs]);

  const activeTab = tabs.find((t) => t.path === activeTabPath) || null;

  const fileTreeFocusPath = useMemo(() => {
    if (fileTreeSelection.size > 0) return [...fileTreeSelection][0];
    return activeTabPath && !isRecoveryPath(activeTabPath) ? activeTabPath : null;
  }, [fileTreeSelection, activeTabPath]);

  const switchSidebarTab = useCallback((tab: SidebarTab) => {
    if (sidebarTab === tab && !sidebarCollapsed) {
      setSidebarCollapsed(true);
    } else {
      setSidebarTab(tab);
      setSidebarCollapsed(false);
    }
  }, [sidebarTab, sidebarCollapsed]);

  // ─── Load file tree ──────────────────────────────────────────────
  const [showHiddenFiles, setShowHiddenFiles] = useState<boolean>(() => {
    try {
      return localStorage.getItem("agentz.explorer.showHidden") === "1";
    } catch {
      return false;
    }
  });
  const loadFileTree = useCallback(async () => {
    if (!projectDir) return;
    try {
      const nodes = await ideApi.listFiles(projectDir, 8, showHiddenFiles);
      setFileTree(nodes);
    } catch (e) {
      console.error("Failed to load file tree:", e);
    }
  }, [projectDir, showHiddenFiles]);
  const toggleShowHiddenFiles = useCallback(() => {
    setShowHiddenFiles((v) => {
      try {
        localStorage.setItem("agentz.explorer.showHidden", v ? "0" : "1");
      } catch {
        /* ignore */
      }
      return !v;
    });
  }, []);
  const firstHiddenRenderRef = useRef(true);
  useEffect(() => {
    if (firstHiddenRenderRef.current) {
      firstHiddenRenderRef.current = false;
      return;
    }
    void loadFileTree();
  }, [loadFileTree]);

  useEffect(() => {
    if (fileTree.length === 0 || explorerExpandedInitRef.current) return;
    setExplorerExpanded(defaultExplorerExpanded(fileTree, fileTreeFocusPath));
    explorerExpandedInitRef.current = true;
  }, [fileTree, fileTreeFocusPath]);

  const agentTurnBusyRef = useRef(agentTurnBusy);
  agentTurnBusyRef.current = agentTurnBusy;

  // ─── Panel resize drag handlers ──────────────────────────────────
  const startSidebarResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startW = sidebarWidth;
      const onMove = (ev: MouseEvent) => {
        const delta = ev.clientX - startX;
        setSidebarWidth(Math.min(500, Math.max(220, startW + delta)));
      };
      const onUp = () => {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
    },
    [sidebarWidth],
  );

  const startBottomResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const startY = e.clientY;
      const startH = bottomHeight;
      const onMove = (ev: MouseEvent) => {
        const delta = startY - ev.clientY;
        setBottomHeight(Math.min(560, Math.max(120, startH + delta)));
      };
      const onUp = () => {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
      document.body.style.cursor = "row-resize";
      document.body.style.userSelect = "none";
    },
    [bottomHeight],
  );

  // Debounced reload for open tabs whose backing files changed externally.
  const tabReloadTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingTabPaths = useRef<Map<string, CapturedOperation>>(new Map());
  // "deleted" watcher events are verified before acting: an atomic save
  // (temp file + rename, our own Ctrl+S included) also surfaces as a
  // removal on Windows, and closing the tab on that false positive was the
  // "Ctrl+S closes the file / claims it was deleted" bug.
  const pendingDeleteTimers = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());
  const scheduleTabReloadRef = useRef<
    ((path: string, kind: "modified" | "deleted") => void) | null
  >(null);

  const scheduleTabReload = useCallback((evtPath: string, kind: "modified" | "deleted") => {
    if (!projectDirRef.current || isRecoveryPath(evtPath)) return;

    if (kind === "deleted") {
      const project = projectDirRef.current;
      const timers = pendingDeleteTimers.current;
      const existing = timers.get(evtPath);
      if (existing) clearTimeout(existing);
      timers.set(
        evtPath,
        setTimeout(() => {
          timers.delete(evtPath);
          if (projectDirRef.current !== project) return;
          void ideApi
            .readFile(`${project}/${evtPath}`)
            .then(() => {
              // Still on disk — atomic-save rename or delete+recreate.
              // Treat it as a modification and take the reload path.
              scheduleTabReloadRef.current?.(evtPath, "modified");
            })
            .catch(() => {
              // Genuinely gone from disk.
              const tab = tabsRef.current.find((row) => row.path === evtPath);
              if (tab?.isDirty) {
                const captured = conflicts.current.beginRead(evtPath);
                if (captured) conflicts.current.settleRead(captured, { kind: "deleted" });
                setFileLoadError(
                  `"${evtPath}" was deleted on disk. Unsaved editor content has been retained.`,
                );
                return;
              }
              setTabs((p) => p.filter((t) => t.path !== evtPath));
              setActiveTabPath((cur) => {
                if (cur !== evtPath) return cur;
                const rest = tabsRef.current.filter((t) => t.path !== evtPath);
                return rest[0]?.path ?? null;
              });
            });
        }, 400),
      );
      return;
    }

    const tab = tabsRef.current.find((t) => t.path === evtPath);
    if (!tab) return;

    const captured = conflicts.current.beginRead(evtPath, { allowDiscard: false });
    if (!captured) return;
    pendingTabPaths.current.set(evtPath, captured);
    perfCounters.recordTabReloadScheduled();
    if (tabReloadTimer.current) clearTimeout(tabReloadTimer.current);
    tabReloadTimer.current = setTimeout(() => {
      tabReloadTimer.current = null;
      const paths = [...pendingTabPaths.current.entries()];
      pendingTabPaths.current.clear();
      for (const [path, capture] of paths) {
        if (!conflicts.current.snapshot(path) || capture.project !== projectDirRef.current) continue;
        const fullPath = `${capture.project}/${path}`;
        void ideApi.readFile(fullPath).then((fc) => {
          if (fc.is_binary) throw new Error("Changed file is binary; the editor buffer was retained.");
          const settled = conflicts.current.settleRead(capture, { kind: "content", content: fc.content });
          if (settled.status === "conflict") {
            if (settled.diskContent !== settled.localContent) {
              const info = conflicts.current.recordConflict(path, fc.content, fc.sha256);
              if (info) {
                setDiskConflicts((prev) => ({ ...prev, [path]: info }));
              }
            }
            return;
          }
          if (settled.status !== "applied") return;
          if (fc.sha256) diskStamps.current.set(fullPath, fc.sha256);
          setDiskConflicts((prev) => {
            if (!(path in prev)) return prev;
            const copy = { ...prev };
            delete copy[path];
            return copy;
          });
          setTabs((p) =>
            p.map((row) =>
              row.path === path ? { ...row, content: fc.content, isDirty: false } : row,
            ),
          );
        }).catch(() => {
          if (conflicts.current.settleRead(capture, { kind: "error" }).status !== "stale")
            setFileLoadError(`Could not reload "${path}". The editor buffer and unsaved state were retained.`);
        });
      }
    }, 250);
  }, [setTabs]);

  scheduleTabReloadRef.current = scheduleTabReload;

  const reloadTabFromDisk = useCallback(
    async (path: string) => {
      const dir = projectDirRef.current;
      if (!dir || isBrowserTab(path) || isRecoveryPath(path)) return;
      const tab = tabsRef.current.find((t) => t.path === path);
      if (!tab) return;
      let allowDiscard = false;
      if (tab.isDirty) {
        const name = path.split("/").pop() || path;
        const ok = window.confirm(
          (t("ide.externalChangeConflict", { name }) as string) ||
            `"${name}" changed on disk but you have unsaved edits. Reload from disk and discard local changes?`,
        );
        if (!ok) return;
        allowDiscard = true;
      }
      const captured = conflicts.current.beginRead(path, { allowDiscard });
      if (!captured) return;
      try {
        const fc = await ideApi.readFile(`${dir}/${path}`);
        if (fc.is_binary) {
          if (conflicts.current.settleRead(captured, { kind: "error" }).status !== "stale")
            setFileLoadError(`Cannot reload binary file "${path}". The editor buffer was retained.`);
          return;
        }
        if (conflicts.current.settleRead(captured, { kind: "content", content: fc.content }).status !== "applied") return;
        if (fc.sha256) diskStamps.current.set(`${dir}/${path}`, fc.sha256);
        setTabs((p) =>
          p.map((row) =>
            row.path === path ? { ...row, content: fc.content, isDirty: false } : row,
          ),
        );
      } catch {
        if (conflicts.current.settleRead(captured, { kind: "error" }).status !== "stale")
          setFileLoadError(`Could not reload "${path}". The editor buffer and unsaved state were retained.`);
      }
    },
    [t, setTabs],
  );

  const handleViewConflictDiff = useCallback(
    (path: string) => {
      const conflict = diskConflicts[path] ?? conflicts.current.getConflict(path);
      const tab = tabsRef.current.find((t) => t.path === path);
      if (!conflict || !tab) return;
      const diffPath = `diff:conflict:${path}`;
      const diffTab: OpenTab = {
        path: diffPath,
        name: `${tab.name} (Disk ↔ Mine)`,
        language: tab.language,
        content: tab.content,
        originalContent: conflict.diskContent,
        isDiff: true,
        isDirty: false,
        isReadOnly: true,
      };
      setTabs((prev) => {
        const exists = prev.some((t) => t.path === diffPath);
        return exists
          ? prev.map((t) => (t.path === diffPath ? diffTab : t))
          : [...prev, diffTab];
      });
      setActiveTabPath(diffPath);
    },
    [diskConflicts, setTabs],
  );

  const handleAcceptDiskConflict = useCallback(
    (path: string) => {
      const conflict = diskConflicts[path] ?? conflicts.current.getConflict(path);
      if (!conflict) return;
      const dir = projectDirRef.current;
      const settled = conflicts.current.resolveConflictAcceptDisk(path, conflict.diskContent);
      if (settled.status !== "applied") return;
      if (dir && conflict.sha256) {
        diskStamps.current.set(`${dir}/${path}`, conflict.sha256);
      }
      setDiskConflicts((prev) => {
        const copy = { ...prev };
        delete copy[path];
        return copy;
      });
      setTabs((prev) =>
        prev.map((row) =>
          row.path === path ? { ...row, content: conflict.diskContent, isDirty: false } : row,
        ),
      );
    },
    [diskConflicts, setTabs],
  );

  const handleKeepMineConflict = useCallback(
    (path: string) => {
      const conflict = diskConflicts[path] ?? conflicts.current.getConflict(path);
      const dir = projectDirRef.current;
      conflicts.current.resolveConflictKeepMine(path);
      if (dir && conflict?.sha256) {
        diskStamps.current.set(`${dir}/${path}`, conflict.sha256);
      }
      setDiskConflicts((prev) => {
        const copy = { ...prev };
        delete copy[path];
        return copy;
      });
    },
    [diskConflicts],
  );

  useEffect(() => registerWorkspaceRefresh("fileTree", loadFileTree), [registerWorkspaceRefresh, loadFileTree]);

  // The watcher re-walks the whole project on start, so its lifetime must follow
  // `projectDir` only — read the latest callbacks through a ref.
  const watcherCallbacksRef = useRef({ scheduleWorkspaceRefresh, scheduleTabReload });
  watcherCallbacksRef.current = { scheduleWorkspaceRefresh, scheduleTabReload };

  useEffect(() => {
    if (!projectDir) return;
    const callbacks = watcherCallbacksRef;

    callbacks.current.scheduleWorkspaceRefresh({ git: true, fileTree: true, force: true, delayMs: 0 });

    const stopOwnedWatcher = startOwnedWatcher(
      () => ideApi.startWatcher(projectDir),
      () => ideApi.stopWatcher(projectDir),
    );

    const unlistenPromise = onFileChanged((evt) => {
      if (evt.project_dir !== projectDir) return;
      perfCounters.recordWatcherReceived();
      const evtPath = normalizeRelPath(evt.path);
      if (!shouldWatchPath(evtPath)) {
        perfCounters.recordWatcherIgnored();
        return;
      }

      if (!agentTurnBusyRef.current) {
        if (evt.kind === "created") {
          callbacks.current.scheduleWorkspaceRefresh({ fileTree: true });
        } else {
          callbacks.current.scheduleWorkspaceRefresh({ git: true, fileTree: true });
        }
      }

      if (evt.kind === "modified" || evt.kind === "deleted") {
        callbacks.current.scheduleTabReload(evtPath, evt.kind);
      }
    }).then((raw) => perfCounters.trackListener(raw));

    return () => {
      for (const timer of pendingDeleteTimers.current.values()) clearTimeout(timer);
      pendingDeleteTimers.current.clear();
      if (tabReloadTimer.current) {
        clearTimeout(tabReloadTimer.current);
        tabReloadTimer.current = null;
      }
      pendingTabPaths.current.clear();
      unlistenPromise.then((fn) => fn());
      stopOwnedWatcher();
    };
  }, [projectDir]);

  const openFile = useCallback(
    async (rawPath: string, readOnly = false) => {
      if (isRecoveryPath(rawPath)) {
        if (tabsRef.current.some(tab => tab.path === rawPath)) setActiveTabPath(rawPath);
        return;
      }
      // Artifact / git paths may arrive as absolute Windows paths
      // (`D:/projects/…/file.md`); relativize under the project so the tab
      // key matches tree paths and the read never double-joins the root.
      const path = projectDir
        ? relativizeArtifactPath(rawPath, [projectDir])
        : rawPath;
      const existing = tabsRef.current.find((t) => t.path === path);
      // Agent-written plan documents open as a rendered preview.
      const isPlanDoc = /^\.agentz\/plans\/.+\.md$/i.test(path.replace(/\\/g, "/"));
      if (existing) {
        if (isPlanDoc) setTabs((prev) => prev.map((t) => (t.path === path ? { ...t, viewMode: "preview" } : t)));
        setActiveTabPath(path);
        return;
      }

      const fullPath = projectDir ? resolveArtifactFullPath(projectDir, path) : path;
      const capturedGeneration = editorGeneration.current;
      const request = ++openSequence.current;
      openRequests.current.set(path, request);
      const current = () => capturedGeneration === editorGeneration.current && openRequests.current.get(path) === request;
      setFileLoading(path);
      setFileLoadError(null);
      try {
        const fc = await ideApi.readFile(fullPath);
        if (!current() || tabsRef.current.some(tab => tab.path === path)) return;
        if (fc.sha256) diskStamps.current.set(fullPath, fc.sha256);
        if (fc.is_binary && fc.preview_data) {
          const newTab: OpenTab = {
            path,
            name: path.split("/").pop() || path,
            language: null,
            content: "",
            isDirty: false,
            isReadOnly: true,
            viewMode: "image",
            previewData: fc.preview_data,
            fileSize: fc.size,
          };
          setTabs((prev) => [...prev, newTab]);
          setActiveTabPath(path);
          return;
        }
        if (fc.is_binary) {
          setFileLoadError(t("ide.binaryFileError", { name: path.split("/").pop() || path }));
          return;
        }
        const newTab: OpenTab = {
          path,
          name: path.split("/").pop() || path,
          language: fc.language,
          content: fc.content,
          isDirty: false,
          isReadOnly: readOnly,
          viewMode: isPlanDoc ? "preview" : "editor",
          fileSize: fc.size,
        };
        setTabs((prev) => [...prev, newTab]);
        setActiveTabPath(path);
      } catch (e) {
        if (!current()) return;
        const msg = String(e);
        setFileLoadError(msg);
        console.error("Failed to read file:", e);
      } finally {
        if (current() && openSequence.current === request) setFileLoading(null);
      }
    },
    [projectDir, t, setTabs],
  );

  openFileRef.current = openFile;

  useEffect(() => {
    if (!openPathRequest?.path || !projectDir) return;
    if (openPathRequest.nonce <= handledOpenPathNonce.current) return;
    handledOpenPathNonce.current = openPathRequest.nonce;
    void openFileRef.current(openPathRequest.path).finally(() => {
      onOpenPathRequestHandled?.();
    });
  }, [openPathRequest?.nonce, openPathRequest?.path, projectDir, onOpenPathRequestHandled]);

  const setTabViewMode = useCallback((path: string, mode: TabViewMode) => {
    setTabs((prev) =>
      prev.map((t) => (t.path === path ? { ...t, viewMode: mode } : t)),
    );
  }, [setTabs]);

  // ─── Open a file and jump to a line/column (search results) ──────
  const revealInFile = useCallback(
    async (path: string, line: number, column: number) => {
      if (isRecoveryPath(path)) return;
      const generation = editorGeneration.current;
      await openFile(path);
      if (generation !== editorGeneration.current || !tabsRef.current.some(tab => tab.path === path)) return;
      setActiveTabPath(path);
      setReveal({ path, line, column: column || 1, nonce: Date.now() });
    },
    [openFile],
  );

  // ─── Open diff for a file ────────────────────────────────────────
  const openDiff = useCallback(
    async (path: string) => {
      if (!projectDir || isRecoveryPath(path)) return;
      const diffPath = `diff:${path}`;
      const existing = tabs.find((t) => t.path === diffPath);
      if (existing) {
        setActiveTabPath(diffPath);
        return;
      }

      try {
        const generation = editorGeneration.current;
        const diff = await ideApi.gitDiff(projectDir, path);
        if (generation !== editorGeneration.current || tabsRef.current.some(tab => tab.path === diffPath)) return;
        const newTab: OpenTab = {
          path: diffPath,
          name: `${path} (diff)`,
          language: null,
          content: diff.modified,
          isDirty: false,
          isReadOnly: true,
          isDiff: true,
          originalContent: diff.original,
        };
        setTabs((prev) => [...prev, newTab]);
        setActiveTabPath(diffPath);
      } catch (e) {
        console.error("Failed to get diff:", e);
      }
    },
    [projectDir, tabs, setTabs],
  );

  // ─── Handle editor content change ───────────────────────────────
  const handleEditorChange = useCallback(
    (value: string) => {
      if (!activeTabPath || isRecoveryPath(activeTabPath) || tabsRef.current.find(tab => tab.path === activeTabPath)?.isReadOnly) return;
      conflicts.current.edit(activeTabPath, value);
      setTabs((prev) =>
        prev.map((t) =>
          t.path === activeTabPath
            ? { ...t, content: value, isDirty: true }
            : t,
        ),
      );
    },
    [activeTabPath, setTabs],
  );

  // ─── Save file (Ctrl+S) ──────────────────────────────────────────
  // Uses refs so the latest tab content / projectDir are always used,
  // even if the user's keystrokes raced the React render cycle.
  const saveFile = useCallback(
    async (path: string) => {
      const tab = tabsRef.current.find((t) => t.path === path);
      const dir = projectDirRef.current;
      if (!tab || !dir || tab.isReadOnly || isRecoveryPath(path)) return false;
      const fullPath = `${dir}/${path}`;
      const beforeContent = conflicts.current.snapshot(path)?.content ?? "";
      const captured = conflicts.current.beginSave(path);
      if (!captured) return false;
      try {
        const digest = await ideApi.writeFile(fullPath, tab.content, diskStamps.current.get(fullPath));
        if (digest) diskStamps.current.set(fullPath, digest);
        else diskStamps.current.delete(fullPath);
        if (conflicts.current.settleSave(captured, true).status !== "saved") return false;
        recordUserRecentEdit({
          projectDir: dir,
          path,
          beforeContent,
          afterContent: tab.content,
        });
        setDiskConflicts((prev) => {
          if (!(path in prev)) return prev;
          const copy = { ...prev };
          delete copy[path];
          return copy;
        });
        setTabs((prev) =>
          prev.map((t) => (t.path === path ? { ...t, isDirty: false } : t)),
        );
        scheduleWorkspaceRefresh({ git: true, force: true, delayMs: 0 });
        return true;
      } catch (e) {
        if (conflicts.current.settleSave(captured, false).status !== "stale") {
          if (String(e).startsWith("conflict:")) {
            void ideApi
              .readFile(fullPath)
              .then((fc) => {
                if (!fc.is_binary) {
                  const info = conflicts.current.recordConflict(path, fc.content, fc.sha256);
                  if (info) setDiskConflicts((prev) => ({ ...prev, [path]: info }));
                }
              })
              .catch(() => {
                setFileLoadError(
                  `"${path}" changed on disk. Your edits were not saved; reload the file from disk before saving again.`,
                );
              });
          } else {
            setFileLoadError(
              `Could not save "${path}". Unsaved editor content has been retained. ${String(e)}`,
            );
          }
        }
        return false;
      }
    },
    [scheduleWorkspaceRefresh, setTabs],
  );

  // The editor keeps each file's Monaco model alive across tab switches
  // (`keepCurrentModel`), so models must be disposed once their tab is gone —
  // whichever close path removed it.
  const openModelUrisRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    const current = new Set(
      projectDir
        ? tabs
            .filter((tab) => !tab.isDiff && !isBrowserTab(tab.path) && !isRecoveryPath(tab.path))
            .map((tab) => fileUriString(`${projectDir}/${tab.path}`))
        : [],
    );
    for (const uri of openModelUrisRef.current) {
      if (!current.has(uri)) monaco.editor.getModel(monaco.Uri.parse(uri))?.dispose();
    }
    openModelUrisRef.current = current;
  }, [tabs, projectDir]);
  useEffect(() => () => {
    // Monaco retains inactive tab models. Release this workspace's models if
    // the entire pane unmounts before its normal tab-close path runs.
    for (const uri of openModelUrisRef.current) {
      monaco.editor.getModel(monaco.Uri.parse(uri))?.dispose();
    }
    openModelUrisRef.current.clear();
  }, []);

  // Development census reads Monaco's actual model registry and the host's
  // owned PTY registry. It reports counts only, never paths or terminal data.
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const debugWindow = window as typeof window & {
      __agentz_codez_resource_snapshot?: () => Promise<{
        monaco_models_total: number;
        tracked_editor_models: number;
        live_tracked_editor_models: number;
        tracked_listeners: ReturnType<typeof perfCounters.resourceSnapshot>["listener"];
        native_pty: Awaited<ReturnType<typeof ideApi.terminalResourceSnapshot>> | null;
        native_watchers: number | null;
        extension_host: Awaited<ReturnType<typeof extHostResourceSnapshot>> | null;
        graph_index: Awaited<ReturnType<typeof graphIndexResourceSnapshot>> | null;
      }>;
    };
    const snapshot = async () => {
      const tracked = openModelUrisRef.current;
      const modelCounts = {
        monaco_models_total: monaco.editor.getModels().length,
        tracked_editor_models: tracked.size,
        live_tracked_editor_models: [...tracked].filter(
          (uri) => monaco.editor.getModel(monaco.Uri.parse(uri)) !== null,
        ).length,
        tracked_listeners: perfCounters.resourceSnapshot().listener,
      };
      let native_pty = null;
      let native_watchers = null;
      let extension_host = null;
      let graph_index = null;
      try {
        native_pty = await ideApi.terminalResourceSnapshot();
      } catch {
        // A native census failure is unknown, never a measured zero.
      }
      try {
        native_watchers = await ideApi.watcherCount();
      } catch {
        // Keep a failed watcher census distinct from an empty registry.
      }
      try {
        extension_host = await extHostResourceSnapshot();
      } catch {
        // The sidecar census is unknown when the host query fails.
      }
      try {
        graph_index = await graphIndexResourceSnapshot();
      } catch {
        // Keep an unavailable graph-index census explicit.
      }
      return { ...modelCounts, native_pty, native_watchers, extension_host, graph_index };
    };
    debugWindow.__agentz_codez_resource_snapshot = snapshot;
    return () => {
      if (debugWindow.__agentz_codez_resource_snapshot === snapshot) {
        delete debugWindow.__agentz_codez_resource_snapshot;
      }
    };
  }, []);

  // ─── Close tab (no dirty prompt — used internally) ─────────────────
  const removeTab = useCallback(
    (path: string) => {
      setTabs((prev) => {
        const idx = prev.findIndex((t) => t.path === path);
        const next = prev.filter((t) => t.path !== path);
        if (activeTabPathRef.current === path) {
          const newActive = next[Math.min(idx, next.length - 1)] || null;
          setActiveTabPath(newActive?.path || null);
        }
        return next;
      });
    },
    [setTabs],
  );

  // ─── Close tab (with dirty prompt) ──────────────────────────────────
  const closeTab = useCallback(
    (path: string) => {
      const tab = tabsRef.current.find((t) => t.path === path);
      if (tab && isRecoveryPath(path)) {
        if (window.confirm("Discard this recovered draft? Copy its text before closing; this does not save the original file.")) removeTab(path);
        return;
      }
      if (tab?.isDirty) {
        const name = tab.name || path;
        const msg = t("ide.unsavedConfirm", { name })
          || `"${name}" has unsaved changes. Save before closing?`;
        // eslint-disable-next-line no-alert
        const answer = window.confirm(msg);
        if (!answer) return; // cancel — keep tab open
        // User clicked OK — save then close
        const dir = projectDirRef.current;
        if (dir && !tab.isReadOnly && !tab.path.startsWith("diff:")) {
          const identity = conflicts.current.snapshot(path)?.tabIdentity;
          void saveFile(path).then(saved => {
            if (saved && conflicts.current.snapshot(path)?.tabIdentity === identity && !conflicts.current.snapshot(path)?.isDirty) removeTab(path);
          });
        } else {
          removeTab(path);
        }
      } else {
        removeTab(path);
      }
    },
    [removeTab, saveFile, t],
  );

  const handleTabClose = useCallback(
    (path: string) => {
      if (isBrowserTab(path)) {
        void (async () => {
          const ok = await requestBrowserClose();
          if (!ok) return;
          onBrowserOpenChange?.(false);
          void browserClose().catch(() => {});
          setActiveTabPath((cur) =>
            isBrowserTab(cur) ? tabsRef.current[0]?.path ?? null : cur,
          );
        })();
        return;
      }
      closeTab(path);
    },
    [closeTab, onBrowserOpenChange, requestBrowserClose],
  );

  // ─── Context menu actions ─────────────────────────────────────────
  const closeAllTabs = useCallback(async () => {
    const dir = projectDirRef.current;
    const generation = editorGeneration.current;
    // Recovered drafts stay open: they cannot be saved here, and closing them would discard data.
    const snapshot = tabsRef.current.filter(tab => !isRecoveryPath(tab.path));
    const identities = new Map(snapshot.map(tab => [tab.path, conflicts.current.snapshot(tab.path)?.tabIdentity]));
    // Save all dirty tabs first (with confirmation)
    const dirty = snapshot.filter((t) => t.isDirty);
    if (dirty.length > 0) {
      // eslint-disable-next-line no-alert
      const msg = t("ide.unsavedBulkConfirm", { count: dirty.length })
        || `${dirty.length} file(s) have unsaved changes. Save all before closing?`;
      // eslint-disable-next-line no-alert
      const save = window.confirm(msg);
      // Cancel keeps every tab open and unchanged; only OK saves, then closes.
      if (!save || !dir) return;
      const saved = await Promise.all(
        dirty
          .filter((t) => !t.isReadOnly && !t.path.startsWith("diff:"))
          .map((t) => saveFile(t.path)),
      );
      if (saved.some(success => !success)) return;
      if (dirty.some(tab => conflicts.current.snapshot(tab.path)?.isDirty)) return;
    }
    if (generation !== editorGeneration.current) return;
    // Keep any tab edited while the save was in flight; the identity check alone cannot see content changes.
    setTabs(previous => previous.filter(tab => !identities.has(tab.path) || conflicts.current.snapshot(tab.path)?.isDirty || conflicts.current.snapshot(tab.path)?.tabIdentity !== identities.get(tab.path)));
    setActiveTabPath(tabsRef.current[0]?.path ?? null);
    scheduleWorkspaceRefresh({ git: true, force: true, delayMs: 0 });
  }, [scheduleWorkspaceRefresh, t, setTabs, saveFile]);

  const closeSavedTabs = useCallback(() => {
    setTabs((prev) => prev.filter((t) => t.isDirty));
    setActiveTabPath((current) => {
      const remaining = tabsRef.current.filter((t) => t.isDirty);
      if (current && remaining.some((t) => t.path === current)) return current;
      return remaining[0]?.path || null;
    });
  }, [setTabs]);

  const closeOtherTabs = useCallback(async (keepPath: string) => {
    const dir = projectDirRef.current;
    const generation = editorGeneration.current;
    // Recovered drafts stay open: they cannot be saved here, and closing them would discard data.
    const snapshot = tabsRef.current.filter(tab => !isRecoveryPath(tab.path));
    const identities = new Map(snapshot.filter(tab => tab.path !== keepPath).map(tab => [tab.path, conflicts.current.snapshot(tab.path)?.tabIdentity]));
    const closingDirty = snapshot.filter((t) => t.path !== keepPath && t.isDirty);
    if (closingDirty.length > 0) {
      // eslint-disable-next-line no-alert
      const msg = t("ide.unsavedBulkConfirm", { count: closingDirty.length })
        || `${closingDirty.length} file(s) have unsaved changes. Save before closing?`;
      // eslint-disable-next-line no-alert
      const save = window.confirm(msg);
      // Cancel keeps every tab open and unchanged; only OK saves, then closes.
      if (!save || !dir) return;
      const saved = await Promise.all(
        closingDirty
          .filter((t) => !t.isReadOnly && !t.path.startsWith("diff:"))
          .map((t) => saveFile(t.path)),
      );
      if (saved.some(success => !success)) return;
      if (closingDirty.some(tab => conflicts.current.snapshot(tab.path)?.isDirty)) return;
    }
    if (generation !== editorGeneration.current) return;
    setTabs(previous => previous.filter(tab => !identities.has(tab.path) || conflicts.current.snapshot(tab.path)?.isDirty || conflicts.current.snapshot(tab.path)?.tabIdentity !== identities.get(tab.path)));
    setActiveTabPath(keepPath);
    scheduleWorkspaceRefresh({ git: true, force: true, delayMs: 0 });
  }, [scheduleWorkspaceRefresh, t, setTabs, saveFile]);

  const handleTabContextMenu = useCallback(
    (e: React.MouseEvent, path: string) => {
      e.preventDefault();
      e.stopPropagation();
      setTabContextMenu({ x: e.clientX, y: e.clientY, targetPath: path });
    },
    [],
  );

  // ─── File tree: multi-select + context menu ──────────────────────────
  const handleFileTreeSelect = useCallback(
    (path: string, opts: { multi: boolean }) => {
      setFileTreeSelection((prev) => {
        if (opts.multi) {
          const next = new Set(prev);
          if (next.has(path)) next.delete(path);
          else next.add(path);
          return next;
        }
        return new Set([path]);
      });
    },
    [],
  );

  const handleFileTreeContextMenu = useCallback(
    (menu: FileTreeContextMenu) => {
      setFileTreeContextMenu(menu);
    },
    [],
  );

  // Dismiss file tree context menu on outside click / escape / scroll
  useEffect(() => {
    if (!fileTreeContextMenu) return;
    const dismiss = () => setFileTreeContextMenu(null);
    const onScroll = (e: Event) => {
      const target = e.target as HTMLElement | null;
      if (target && typeof target.closest === "function" && target.closest(".ide-tab-context-menu")) {
        return;
      }
      dismiss();
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") dismiss(); };
    window.addEventListener("click", dismiss);
    window.addEventListener("contextmenu", dismiss);
    window.addEventListener("keydown", onKey);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      window.removeEventListener("click", dismiss);
      window.removeEventListener("contextmenu", dismiss);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [fileTreeContextMenu]);

  // ─── File tree keyboard shortcuts (Delete / F2) ───────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // Only when the file tree (or a child input inside it) has focus
      const el = fileTreeRef.current;
      if (!el) return;
      if (!el.contains(document.activeElement) && document.activeElement !== el) return;
      // Don't intercept when the user is typing in an inline create/rename input
      if ((document.activeElement as HTMLElement)?.tagName === "INPUT") return;

      if (e.key === "Delete") {
        e.preventDefault();
        el.deleteSelected?.();
      } else if (e.key === "F2") {
        e.preventDefault();
        el.renameActive?.();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Dismiss context menu on outside click / escape / scroll
  useEffect(() => {
    if (!tabContextMenu) return;
    const dismiss = () => setTabContextMenu(null);
    const onScroll = (e: Event) => {
      const target = e.target as HTMLElement | null;
      if (target && typeof target.closest === "function" && target.closest(".ide-tab-context-menu")) {
        return;
      }
      dismiss();
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") dismiss(); };
    window.addEventListener("click", dismiss);
    window.addEventListener("contextmenu", dismiss);
    window.addEventListener("keydown", onKey);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      window.removeEventListener("click", dismiss);
      window.removeEventListener("contextmenu", dismiss);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [tabContextMenu]);

  const { paletteCommands } = useCodezShortcuts({ t, projectDir, activeTabPathRef, saveFile, handleTabClose, closeAllTabs, onOpenFolder, setBottomOpen, setSidebarTab, setSidebarCollapsed, setRemoteDialogOpen, setPaletteMode, setPaletteOpen });

  useEffect(() => {
    const onShowTerminal = () => {
      openBottomPanel("terminal");
    };
    const onOpenExt = () => {
      openExtensionsSidebar();
    };
    const onOpenFile = (e: Event) => {
      const p = (e as CustomEvent<{ path?: string }>).detail?.path;
      if (p) void openFile(p);
    };
    const onOpenPalette = (e: Event) => {
      const mode = (e as CustomEvent<{ mode?: "files" | "commands" }>).detail?.mode;
      setPaletteMode(mode === "files" ? "files" : "commands");
      setPaletteOpen(true);
    };
    window.addEventListener("agentz-show-terminal", onShowTerminal);
    window.addEventListener("agentz-open-extensions", onOpenExt);
    window.addEventListener("agentz-open-file", onOpenFile);
    window.addEventListener("agentz-open-palette", onOpenPalette);
    return () => {
      window.removeEventListener("agentz-show-terminal", onShowTerminal);
      window.removeEventListener("agentz-open-extensions", onOpenExt);
      window.removeEventListener("agentz-open-file", onOpenFile);
      window.removeEventListener("agentz-open-palette", onOpenPalette);
    };
  }, [openBottomPanel, openExtensionsSidebar, openFile, setPaletteMode, setPaletteOpen]);

  useEffect(() => {
    if (!projectDir || !onWorkspacePatch) return;
    const timer = window.setTimeout(() => {
      onWorkspacePatch({
        editor: editorSnapshotFromTabs(tabsRef.current, activeTabPathRef.current),
        layout: {
          sidebar_tab: sidebarTab,
          sidebar_collapsed: sidebarCollapsed,
          sidebar_width: sidebarWidth,
          bottom_open: bottomOpen,
          bottom_tab: bottomTab,
          bottom_height: bottomHeight,
          explorer_expanded_paths: [...explorerExpanded],
        },
      });
    }, 800);
    return () => window.clearTimeout(timer);
  }, [
    projectDir,
    onWorkspacePatch,
    tabs,
    activeTabPath,
    sidebarTab,
    sidebarCollapsed,
    sidebarWidth,
    bottomOpen,
    bottomTab,
    bottomHeight,
    explorerExpanded,
  ]);

  return {
    t, ideRef, sidebarTab, sidebarCollapsed, switchSidebarTab, gitModified, gitAdded, bottomOpen, setBottomOpen, sidebarWidth, fileTree, showHiddenFiles, toggleShowHiddenFiles, fileTreeFocusPath, fileTreeSelection, openFile, scheduleWorkspaceRefresh, handleFileTreeSelect, handleFileTreeContextMenu, fileTreeRef, explorerExpanded, setExplorerExpanded, revealInFile, openDiff, startSidebarResize, headerTabs, activeTabPath, setActiveTabPath, handleTabClose, saveFile, handleTabContextMenu, closeAllTabs, closeSavedTabs, closeOtherTabs, reloadTabFromDisk, tabContextMenu, setTabContextMenu, setRemoteDialogOpen, fileLoading, fileLoadError, setFileLoadError, browserRefreshSignal, agentBrowserAction, requestBrowserClose, activeTab, reveal, handleEditorChange, setTabViewMode, activeTabPathRef, diskConflicts, handleViewConflictDiff, handleAcceptDiskConflict, handleKeepMineConflict, startBottomResize, bottomTab, setBottomTab, bottomHeight, openBottomPanel, openExtensionsSidebar, remoteDialogOpen, paletteOpen, setPaletteOpen, paletteMode, tabs, paletteCommands, fileTreeContextMenu, setFileTreeContextMenu, projectDirRef,
  };
}