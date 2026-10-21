import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { canPortalToBody, useFixedPopupPosition } from "./utils/popupPosition";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getProjectScope, pickProjectScope, restoreProjectScope, revokeProjectScope } from "./services/tauri/projectScope";
import { isRemoteDir, OPEN_PROJECT_EVENT } from "./extensions/remoteTargets";
import { preflightWorkspaceNavigation } from "./services/workspaceNavigation";
import {
  workspaceLoad,
  workspaceSave,
  workspaceCloseAck,
  type EditorSnapshot,
  type LayoutSnapshot,
  type WorkspaceSnapshot,
} from "./services/tauri/workspace";
import {
  generateDeepRepoWiki,
  generateRepoWiki,
  readRepoMemory,
} from "./services/tauri/repoWiki";
import {
  graphIndexVisualState,
  requestGraphIndex,
  useGraphIndexStatus,
} from "./services/tauri/graphIndex";
import IdeSidePanel from "./components/IdeSidePanel";
import { confirmTerminalCloseOnProjectChange, destroyAllTerminals } from "./workspaces/codez/Terminal";
import { getSettings } from "./services/tauri/settings";
import { setLanguage } from "./i18n";
import { onSettingsRefresh, notifySettingsRefresh } from "./services/settingsRefresh";
import {
  getAppearanceTheme,
  toggleAppearanceTheme,
  type AppearanceTheme,
  getUiFontScale,
  applyUiFontScale,
  UI_FONT_SCALES,
  uiFontScaleLabel,
  type UiFontScale,
  DEFAULT_EDITOR_FONT_SIZE,
  getEditorBaseFontSize,
  getEffectiveEditorFontSize,
  setEditorBaseFontSize,
  stepEditorBaseFontSize,
} from "./theme";
import { syncEditorThemeWithAppearance } from "./workspaces/codez/themeStore";
import CodeZWorkspace from "./workspaces/codez";
import WorkZWorkspace, { type WikiBuildAction } from "./workspaces/workz";
import SettingsPanel from "./workspaces/codez/SettingsPanel";
import { OPEN_SETTINGS_EVENT, SETTINGS_CLOSED_EVENT } from "./workspaces/codez/firstRun";
import { modeShortcutLabel, modeShortcutTarget } from "./utils/modeShortcut";
import ZLogo from "./components/ZLogo";
import { browserClose, browserCloseGuard, type PickedElement } from "./services/tauri/browser";
import type { ChatAttachment } from "./services/tauri/chat";
import { terminalSnippetPut } from "./services/tauri/terminal";
import {
  AssistantBubbleIcon,
  BrowserIcon,
  ChatIcon,
  CloseIcon,
  FolderIcon,
  MoonIcon,
  SettingsIcon,
  StoreIcon,
  SunIcon,
  WikiIcon,
} from "./components/TitleBarIcons";
import ProjectTemplateDialog from "./components/ProjectTemplateDialog";
import { projectHasAgentz } from "./services/tauri/projectTemplates";
import {
  workzOverview,
  workzProjectAdd,
  workzProjectRemove,
  type WorkzProject,
} from "./services/tauri/workz";
import ResourceLibraryPanel, {
  type LibraryInitialState,
} from "./workspaces/codez/ResourceLibraryPanel";
import AssistantMessagesPanel from "./workspaces/codez/AssistantMessagesPanel";
import { getImSettings } from "./services/tauri/gateway";
import ExtensionHostProvider from "./extensions/ui/ExtensionHostProvider";
import { debugController } from "./extensions/debug/debugController";
import { ProjectEdgeProvider } from "./contexts/ProjectEdgeContext";
import ProjectEdgeShell from "./components/ProjectEdgeShell";
import NotificationInbox from "./components/NotificationInbox";
import OnboardingModal, { isOnboardingDone } from "./components/OnboardingModal";
import { composerDbg } from "./utils/composerDebug";
import "./App.css";

const CodeMapPanel = lazy(() =>
  import("./workspaces/codez/CodeMap").then(({ CodeMapPanel }) => ({ default: CodeMapPanel })),
);

type Mode = "codez" | "workz";

/** Normalize folder paths before comparing (slashes, trailing slash). */
function normProjectPath(p: string): string {
  return p.replace(/\\/g, "/").replace(/\/+$/, "");
}

export default function App() {
  const { t } = useTranslation();
  const [mode, setMode] = useState<Mode>("workz");
  // Cursor-style window switch: Ctrl/Cmd+Shift+N -> IDE (CodeZ), Ctrl/Cmd+Alt+N -> Agents (WorkZ).
  // Both views share the current project, so switching keeps the workspace context.
  useEffect(() => {
    const onModeKey = (event: KeyboardEvent) => {
      const target = modeShortcutTarget(event);
      if (!target) return;
      event.preventDefault();
      setMode(target);
    };
    window.addEventListener("keydown", onModeKey);
    return () => window.removeEventListener("keydown", onModeKey);
  }, []);
  const isMac = typeof navigator !== "undefined" && /mac/i.test(navigator.platform);
  const [projectDir, setProjectDir] = useState<string | null>(null);
  const [inboxTaskRoute, setInboxTaskRoute] = useState<{ projectDir: string; taskId: string; nonce: number } | null>(null);
  const [codezTaskBinding, setCodezTaskBinding] = useState<{
    taskId: string;
    worktreePath: string;
    originProjectDir: string;
  } | null>(null);
  useEffect(() => {
    setCodezTaskBinding(null);
  }, [projectDir]);
  const projectNavigationBusy = useRef(false);
  const currentProjectRef = useRef(projectDir);
  currentProjectRef.current = projectDir;
  const liveEditorSnapshot = useRef<(() => EditorSnapshot) | null>(null);
  const registerNavigationSnapshot = useCallback((read: (() => EditorSnapshot) | null) => {
    liveEditorSnapshot.current = read;
  }, []);
  const [chatOpen, setChatOpen] = useState(true);
  const [chatWidth, setChatWidth] = useState<number>(() => {
    const saved = Number(localStorage.getItem("agentz-chat-width"));
    return Number.isFinite(saved) && saved >= 340 ? Math.min(760, saved) : 380;
  });
  const [chatInsert, setChatInsert] = useState<{ paths: string[]; nonce: number } | null>(null);
  const [chatInsertElement, setChatInsertElement] = useState<{
    element: PickedElement;
    nonce: number;
  } | null>(null);
  const [chatInsertTerminal, setChatInsertTerminal] = useState<{
    snippetId: string;
    text: string;
    nonce: number;
  } | null>(null);
  const [chatAttach, setChatAttach] = useState<{
    attachment: ChatAttachment;
    preview: string | null;
    nonce: number;
  } | null>(null);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  // Inline "open Settings" actions (first-run checklist, setup errors, extensions) dispatch this event.
  useEffect(() => {
    const open = () => setSettingsOpen(true);
    window.addEventListener(OPEN_SETTINGS_EVENT, open);
    window.addEventListener("agentz-open-settings", open);
    return () => {
      window.removeEventListener(OPEN_SETTINGS_EVENT, open);
      window.removeEventListener("agentz-open-settings", open);
    };
  }, []);
  const [libraryOpen, setLibraryOpen] = useState(false);
  const [libraryInitial, setLibraryInitial] = useState<LibraryInitialState | null>(null);
  const [assistantPanelOpen, setAssistantPanelOpen] = useState(false);
  const [templateProjectDir, setTemplateProjectDir] = useState<string | null>(null);
  const [hasImAssistant, setHasImAssistant] = useState(false);
  const [wikiBuildNonce, setWikiBuildNonce] = useState(0);
  const [wikiBuildAction, setWikiBuildAction] = useState<WikiBuildAction>("overview");
  const [wikiBusy, setWikiBusy] = useState(false);
  const [wikiMenuOpen, setWikiMenuOpen] = useState(false);
  const [codeMapOpen, setCodeMapOpen] = useState(false);
  const [graphIndexRefreshNonce, setGraphIndexRefreshNonce] = useState(0);
  const wikiMenuRef = useRef<HTMLDivElement>(null);
  const wikiPopoverRef = useRef<HTMLDivElement>(null);
  const graphIndexStatus = useGraphIndexStatus(projectDir, graphIndexRefreshNonce);
  const graphIndexVisual = graphIndexStatus
    ? graphIndexVisualState(graphIndexStatus)
    : null;
  const [settingsToast, setSettingsToast] = useState<string | null>(null);
  const [ideWikiOpenPath, setIdeWikiOpenPath] = useState<{ path: string; nonce: number } | null>(
    null,
  );
  const [appearance, setAppearance] = useState<AppearanceTheme>(() => getAppearanceTheme());
  const [fontScale, setFontScale] = useState<UiFontScale>(() => getUiFontScale());
  const [editorBaseFontSize, setEditorBaseFontSizeState] = useState<number>(() =>
    getEditorBaseFontSize(),
  );
  const [fontScaleOpen, setFontScaleOpen] = useState(false);
  const fontScaleRef = useRef<HTMLDivElement>(null);
  const fontScalePopoverRef = useRef<HTMLDivElement>(null);
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const projectMenuRef = useRef<HTMLDivElement>(null);
  const projectPopoverRef = useRef<HTMLDivElement>(null);
  const projectPopoverStyle = useFixedPopupPosition(
    projectMenuOpen,
    projectMenuRef,
    projectPopoverRef,
    "down",
    "text",
    { align: "left", minWidth: 260, maxWidth: 340, maxHeight: 340 },
  );
  const wikiPopoverStyle = useFixedPopupPosition(
    wikiMenuOpen,
    wikiMenuRef,
    wikiPopoverRef,
    "down",
    "text",
    { align: "right", minWidth: 210, maxWidth: 320, maxHeight: 340 },
  );
  const fontScalePopoverStyle = useFixedPopupPosition(
    fontScaleOpen,
    fontScaleRef,
    fontScalePopoverRef,
    "down",
    "text",
    { align: "right", minWidth: 132, maxWidth: 220, maxHeight: 320 },
  );
  const [recentProjects, setRecentProjects] = useState<WorkzProject[]>([]);
  const [exitToast, setExitToast] = useState(false);
  const [workspaceRestore, setWorkspaceRestore] = useState<{
    key: number;
    editor: EditorSnapshot;
    layout: LayoutSnapshot;
  } | null>(null);
  const [workspaceReady, setWorkspaceReady] = useState(false);
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const [hasConfiguredModel, setHasConfiguredModel] = useState(false);

  const editorPatchRef = useRef<EditorSnapshot>({
    open_paths: [],
    active_path: null,
    dirty_buffers: {},
  });
  const ideLayoutPatchRef = useRef<
    Pick<
      LayoutSnapshot,
      | "sidebar_tab"
      | "sidebar_collapsed"
      | "sidebar_width"
      | "bottom_open"
      | "bottom_tab"
      | "bottom_height"
      | "explorer_expanded_paths"
    >
  >({
    sidebar_tab: "explorer",
    sidebar_collapsed: false,
    sidebar_width: 260,
    bottom_open: false,
    bottom_tab: "terminal",
    bottom_height: 240,
    explorer_expanded_paths: [],
  });
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const buildWorkspaceSnapshot = useCallback((): WorkspaceSnapshot => {
    return {
      version: 1,
      project_dir: projectDir,
      editor: editorPatchRef.current,
      layout: {
        chat_open: chatOpen,
        chat_width: chatWidth,
        browser_open: browserOpen,
        mode,
        ...ideLayoutPatchRef.current,
      },
    };
  }, [projectDir, chatOpen, chatWidth, browserOpen, mode]);

  const persistWorkspace = useCallback(
    async (snap?: WorkspaceSnapshot) => {
      try {
        await workspaceSave(snap ?? buildWorkspaceSnapshot());
      } catch (e) {
        console.error("workspace save failed:", e);
      }
    },
    [buildWorkspaceSnapshot],
  );

  const scheduleWorkspaceSave = useCallback(() => {
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
    saveTimerRef.current = setTimeout(() => {
      saveTimerRef.current = null;
      void persistWorkspace();
    }, 600);
  }, [persistWorkspace]);

  const handleWorkspacePatch = useCallback(
    (patch: {
      editor: EditorSnapshot;
      layout: Pick<
        LayoutSnapshot,
        "sidebar_tab" | "sidebar_collapsed" | "sidebar_width" | "bottom_open" | "bottom_tab" | "bottom_height"
      >;
    }) => {
      editorPatchRef.current = patch.editor;
      ideLayoutPatchRef.current = patch.layout;
      scheduleWorkspaceSave();
    },
    [scheduleWorkspaceSave],
  );

  // Restore last session on startup.
  useEffect(() => {
    let cancelled = false;
    void workspaceLoad()
      .then(async (snap) => {
        if (cancelled || !snap?.project_dir) {
          setWorkspaceReady(true);
          return;
        }
        const layout = snap.layout ?? ({} as LayoutSnapshot);
        if (layout.mode === "codez" || layout.mode === "workz") setMode(layout.mode);
        if (typeof layout.chat_open === "boolean") setChatOpen(layout.chat_open);
        if (layout.chat_width && layout.chat_width >= 340) setChatWidth(layout.chat_width);
        if (layout.browser_open) setBrowserOpen(true);
        ideLayoutPatchRef.current = {
          sidebar_tab: layout.sidebar_tab || "explorer",
          sidebar_collapsed: layout.sidebar_collapsed ?? false,
          sidebar_width: layout.sidebar_width && layout.sidebar_width >= 220 ? layout.sidebar_width : 260,
          bottom_open: layout.bottom_open ?? false,
          bottom_tab: layout.bottom_tab || "terminal",
          bottom_height: layout.bottom_height && layout.bottom_height >= 120 ? layout.bottom_height : 240,
        };
        editorPatchRef.current = snap.editor ?? editorPatchRef.current;
        setWorkspaceRestore({
          key: 1,
          editor: snap.editor,
          layout: {
            chat_open: layout.chat_open ?? true,
            chat_width: layout.chat_width ?? 380,
            browser_open: layout.browser_open ?? false,
            mode: layout.mode === "workz" ? "workz" : "codez",
            ...ideLayoutPatchRef.current,
          },
        });
        let restoredDir = snap.project_dir;
        if (!isRemoteDir(restoredDir)) {
          try {
            const scope = await restoreProjectScope("workspace");
            restoredDir = scope.project_dir;
            if (cancelled) return;
          } catch {
            // Workspace editing can still restore; durable plan actions require a host scope.
            if (cancelled) return;
          }
        }
        setProjectDir(restoredDir);
        setWorkspaceReady(true);
      })
      .catch(() => setWorkspaceReady(true));
    return () => {
      cancelled = true;
    };
  }, []);

  // Persist layout prefs (mode, chat panel) and save on close.
  useEffect(() => {
    if (!workspaceReady) return;
    scheduleWorkspaceSave();
  }, [workspaceReady, mode, chatOpen, chatWidth, browserOpen, projectDir, scheduleWorkspaceSave]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen("app-before-close", () => {
      void (async () => {
        let cancelledByUser = false;
        try {
          const running = await invoke<number>("chat_running_count").catch(() => 0);
          if (running > 0 && !window.confirm(t("app.exitWhileRunning"))) {
            cancelledByUser = true;
            return;
          }
          setExitToast(true);
          if (running > 0) await invoke("chat_cancel_all").catch(() => {});
          await persistWorkspace();
        } catch (e) {
          console.error("workspace close failed:", e);
          setExitToast(false);
        } finally {
          if (!cancelledByUser) {
            await workspaceCloseAck().catch(() => {});
          }
        }
      })();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [persistWorkspace]);

  useEffect(() => {
    const applyLanguage = () => {
      getSettings()
        .then((s) => {
          if (s.language === "zh" || s.language === "en") {
            setLanguage(s.language);
          }
          const configured = Boolean(
            s.is_configured ||
              (s.secret_references && s.secret_references.some((r) => r.exists)),
          );
          setHasConfiguredModel(configured);
          if (!isOnboardingDone() && !configured) {
            setOnboardingOpen(true);
          }
        })
        .catch(() => {
          // config may not exist yet — keep browser-detected language
        });
    };
    applyLanguage();
    return onSettingsRefresh(applyLanguage);
  }, []);

  useEffect(() => {
    return onSettingsRefresh(() => {
      setSettingsToast(t("settings.savedEffectHint"));
      window.setTimeout(() => setSettingsToast((cur) => (cur === t("settings.savedEffectHint") ? null : cur)), 3200);
    });
  }, [t]);

  // Surface the assistant message panel button only once an IM channel is
  // configured. Re-check whenever settings are saved.
  useEffect(() => {
    const detect = () => {
      getImSettings()
        .then((s) => {
          setHasImAssistant(
            Boolean(
              s.feishu_enabled ||
                s.wecom_enabled ||
                s.dingtalk_enabled ||
                s.telegram_enabled ||
                s.slack_enabled ||
                s.discord_enabled ||
                s.teams_enabled ||
                s.matrix_enabled ||
                s.webhook_enabled ||
                s.wechat_enabled,
            ),
          );
        })
        .catch(() => setHasImAssistant(false));
    };
    detect();
    const off = onSettingsRefresh(detect);
    // The agent's `app_control` tool mutates settings/assistants in the backend;
    // bridge its event to the same in-app refresh open panels already listen to.
    const un = listen("agentz:app-control-updated", () => {
      notifySettingsRefresh();
      detect();
    });
    return () => {
      off();
      void un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    const syncFont = () => {
      setFontScale(getUiFontScale());
      setEditorBaseFontSizeState(getEditorBaseFontSize());
    };
    window.addEventListener("agentz-font-scale", syncFont);
    return () => window.removeEventListener("agentz-font-scale", syncFont);
  }, []);

  useEffect(() => {
    if (!fontScaleOpen) return;
    const close = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        fontScaleRef.current &&
        !fontScaleRef.current.contains(target) &&
        !fontScalePopoverRef.current?.contains(target)
      ) {
        setFontScaleOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [fontScaleOpen]);

  useEffect(() => {
    if (!wikiMenuOpen) return;
    const close = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        wikiMenuRef.current &&
        !wikiMenuRef.current.contains(target) &&
        !wikiPopoverRef.current?.contains(target)
      ) {
        setWikiMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [wikiMenuOpen]);

  const refreshRecentProjects = useCallback(() => {
    workzOverview()
      .then((ov) => setRecentProjects(ov.projects.filter((p) => p.exists)))
      .catch(() => {});
  }, []);

  useEffect(() => {
    refreshRecentProjects();
  }, [refreshRecentProjects, mode]);

  useEffect(() => {
    if (!projectMenuOpen) return;
    refreshRecentProjects();
    const close = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        projectMenuRef.current &&
        !projectMenuRef.current.contains(target) &&
        !projectPopoverRef.current?.contains(target)
      ) {
        setProjectMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [projectMenuOpen, refreshRecentProjects]);

  const switchProject = useCallback(
    async (dir: string, navigationOwned = false) => {
    if (!navigationOwned && projectNavigationBusy.current) return false;
    if (!navigationOwned) projectNavigationBusy.current = true;
    try {
      if (projectDir && normProjectPath(dir) === normProjectPath(projectDir)) return true;
      const readSnapshot = () => {
        if (!liveEditorSnapshot.current) throw new Error("Live editor snapshot unavailable");
        return { ...buildWorkspaceSnapshot(), editor: liveEditorSnapshot.current() };
      };
      const checked = await preflightWorkspaceNavigation({
        readSnapshot,
        isCurrent: () => currentProjectRef.current === projectDir,
        confirmTerminals: () => projectDir ? confirmTerminalCloseOnProjectChange(t) : Promise.resolve(true),
        persist: workspaceSave,
      });
      if (checked.status !== "ready") {
        if (checked.reason === "dirty_buffers") window.alert(t("app.saveBeforeProjectSwitch", { defaultValue: "Save or close your edited files and unsaved drafts before switching projects." }));
        if (checked.reason === "save_failed") window.alert(t("app.projectSwitchSaveFailed", { defaultValue: "Could not preserve the workspace. Project switching was cancelled." }));
        return false;
      }
      const stillCurrent = () => currentProjectRef.current === projectDir && JSON.stringify(readSnapshot()) === JSON.stringify(checked.snapshot);
      if (projectDir) await destroyAllTerminals();
      if (!stillCurrent()) return false;
      // The debug session belongs to the old project: stop it before the switch is visible.
      await debugController.stop();
      if (!stillCurrent()) return false;
      setIdeWikiOpenPath(null);
      setChatInsertElement(null);
      const previousScope = projectDir ? getProjectScope(projectDir) : null;
      if (previousScope) await revokeProjectScope(previousScope.handle);
      if (!stillCurrent()) return false;
      setProjectDir(dir);
      if (!isRemoteDir(dir)) {
        workzProjectAdd(dir)
          .then((ov) => setRecentProjects(ov.projects.filter((p) => p.exists)))
          .catch(() => {});
        try {
          const hasAgentz = await projectHasAgentz(dir);
          if (!hasAgentz) setTemplateProjectDir(dir);
        } catch {
          // Non-fatal — user can apply templates from settings later.
        }
      }
      return true;
    } catch (e) {
      console.error("switchProject failed:", e);
    } finally {
      if (!navigationOwned) projectNavigationBusy.current = false;
    }
    },
    [projectDir, t, buildWorkspaceSnapshot],
  );

  useEffect(() => {
    const onOpen = (e: Event) => {
      const dir = (e as CustomEvent<string>).detail;
      if (dir) void switchProject(dir);
    };
    window.addEventListener(OPEN_PROJECT_EVENT, onOpen);
    return () => window.removeEventListener(OPEN_PROJECT_EVENT, onOpen);
  }, [switchProject]);

  const pickFolder = useCallback(async () => {
    if (projectNavigationBusy.current) return;
    projectNavigationBusy.current = true;
    try {
      const scope = await pickProjectScope();
      if (scope && !await switchProject(scope.project_dir, true)) {
        await revokeProjectScope(scope.handle);
      }
    } catch (e) {
      console.error("pickFolder failed:", e);
    } finally {
      projectNavigationBusy.current = false;
    }
  }, [switchProject]);

  const openProjectInCodeZ = useCallback(
    async (dir: string) => {
      if (!await switchProject(dir)) return;
      setMode("codez");
    },
    [switchProject],
  );

  const openTaskWorktreeInCodeZ = useCallback(
    (opts: { taskId: string; worktreePath: string; filePath?: string | null }) => {
      const origin = projectDir || opts.worktreePath;
      setCodezTaskBinding({
        taskId: opts.taskId,
        worktreePath: opts.worktreePath,
        originProjectDir: origin,
      });
      setMode("codez");
      if (opts.filePath) {
        const cleanRel = opts.filePath.replace(/^[\\/]+/, "");
        setIdeWikiOpenPath({ path: cleanRel, nonce: Date.now() });
      }
    },
    [projectDir],
  );

  const openRecentProject = useCallback(
    async (dir: string) => {
      setProjectMenuOpen(false);
      if (projectNavigationBusy.current) return;
      projectNavigationBusy.current = true;
      let mintedHandle: string | null = null;
      try {
        let targetDir = dir;
        if (!isRemoteDir(dir) && !getProjectScope(dir)) {
          try {
            const scope = await restoreProjectScope("workz_project", dir);
            targetDir = scope.project_dir;
            mintedHandle = scope.handle;
          } catch {
            // Editor/chat still open; durable plan actions require a host scope.
          }
        }
        const ok = await switchProject(targetDir, true);
        if (!ok && mintedHandle) {
          await revokeProjectScope(mintedHandle);
        }
      } catch (e) {
        console.error("openRecentProject failed:", e);
      } finally {
        projectNavigationBusy.current = false;
      }
    },
    [switchProject],
  );

  const removeRecentProject = useCallback((dir: string) => {
    workzProjectRemove(dir)
      .then((ov) => setRecentProjects(ov.projects.filter((p) => p.exists)))
      .catch(() => {});
  }, []);

  const closeProject = useCallback(async () => {
    if (!projectDir || projectNavigationBusy.current) return;
    projectNavigationBusy.current = true;
    try {
      const readSnapshot = () => {
        if (!liveEditorSnapshot.current) throw new Error("Live editor snapshot unavailable");
        return { ...buildWorkspaceSnapshot(), editor: liveEditorSnapshot.current() };
      };
      const checked = await preflightWorkspaceNavigation({
        readSnapshot,
        isCurrent: () => currentProjectRef.current === projectDir,
        confirmTerminals: async () => {
          if (!await confirmTerminalCloseOnProjectChange(t)) return false;
          try {
            const guard = await browserCloseGuard();
            return (guard.can_close && !guard.agent_active) || window.confirm(guard.reason ?? t("browser.closeConfirm"));
          } catch {
            return window.confirm(t("browser.closeConfirm"));
          }
        },
        persist: workspaceSave,
      });
      if (checked.status !== "ready") {
        if (checked.reason === "dirty_buffers") window.alert(t("app.saveBeforeProjectClose", { defaultValue: "Save or close your edited files and unsaved drafts before closing the project." }));
        if (checked.reason === "save_failed") window.alert(t("app.projectCloseSaveFailed", { defaultValue: "Could not preserve the workspace. Project closing was cancelled." }));
        return;
      }
      const stillCurrent = () => currentProjectRef.current === projectDir && JSON.stringify(readSnapshot()) === JSON.stringify(checked.snapshot);
      await destroyAllTerminals();
      if (!stillCurrent()) return;
      await debugController.stop();
      if (!stillCurrent()) return;
      const scope = getProjectScope(projectDir);
      if (scope) await revokeProjectScope(scope.handle);
      if (!stillCurrent()) return;
      setIdeWikiOpenPath(null);
      setBrowserOpen(false);
      setChatInsertElement(null);
      void browserClose().catch(() => {});
      setProjectDir(null);
    } catch (e) {
      console.error("closeProject failed:", e);
    } finally {
      projectNavigationBusy.current = false;
    }
  }, [projectDir, t, buildWorkspaceSnapshot]);

  const handleSendToChat = useCallback((paths: string[]) => {
    if (paths.length === 0) return;
    composerDbg("explorer → sendToChat", {
      paths,
      dirs: paths.map((p) => /[/\\]$/.test(p)),
    });
    setChatOpen(true);
    setChatInsert({ paths, nonce: Date.now() });
  }, []);

  const handleSendTerminalToChat = useCallback(async (text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    setChatOpen(true);
    try {
      const snippetId = await terminalSnippetPut(trimmed);
      setChatInsertTerminal({ snippetId, text: trimmed, nonce: Date.now() });
    } catch (e) {
      console.error("terminalSnippetPut failed:", e);
    }
  }, []);

  const handleSendElementToChat = useCallback(
    (el: PickedElement) => {
      if (!projectDir) return;
      setChatOpen(true);
      setChatInsertElement({ element: el, nonce: Date.now() });
    },
    [projectDir],
  );

  const handleScreenshotToChat = useCallback(
    (base64: string) => {
      if (!projectDir) return;
      setChatOpen(true);
    setChatAttach({
      attachment: {
        media_type: "image/png",
        data: base64,
        filename: `browser-${Date.now()}.png`,
        path: null,
      },
      preview: `data:image/png;base64,${base64}`,
      nonce: Date.now(),
    });
    },
    [projectDir],
  );

  const runWikiInIde = useCallback(
    async (action: WikiBuildAction) => {
      if (!projectDir || wikiBusy) return;
      setWikiMenuOpen(false);
      setWikiBusy(true);
      try {
        if (action === "graph") {
          await requestGraphIndex(projectDir);
          setGraphIndexRefreshNonce((n) => n + 1);
          setSettingsToast(t("agent.repoWikiGraphQueued"));
          const queuedMsg = t("agent.repoWikiGraphQueued");
          window.setTimeout(
            () => setSettingsToast((cur) => (cur === queuedMsg ? null : cur)),
            3200,
          );
        } else if (action === "deep") {
          const res = await generateDeepRepoWiki(projectDir);
          setIdeWikiOpenPath({ path: res.path, nonce: Date.now() });
        } else if (action === "memory") {
          const res = await readRepoMemory(projectDir);
          setIdeWikiOpenPath({ path: res.path, nonce: Date.now() });
        } else {
          const res = await generateRepoWiki(projectDir);
          setIdeWikiOpenPath({ path: res.path, nonce: Date.now() });
        }
      } catch (e) {
        console.error("Wiki generation failed:", e);
      } finally {
        setWikiBusy(false);
      }
    },
    [projectDir, wikiBusy, t],
  );

  const triggerWikiInWorkz = useCallback(
    (action: WikiBuildAction) => {
      if (!projectDir || wikiBusy) return;
      setWikiMenuOpen(false);
      setWikiBuildAction(action);
      setWikiBuildNonce((n) => n + 1);
    },
    [projectDir, wikiBusy],
  );

  const handleWikiAction = useCallback(
    (action: WikiBuildAction) => {
      if (mode === "workz") {
        triggerWikiInWorkz(action);
      } else {
        void runWikiInIde(action);
      }
    },
    [mode, runWikiInIde, triggerWikiInWorkz],
  );

  useEffect(() => {
    const onOpenAnchor = (e: Event) => {
      const detail = (e as CustomEvent<{ path?: string }>).detail;
      if (detail?.path && mode !== "workz") {
        setIdeWikiOpenPath({ path: detail.path, nonce: Date.now() });
      }
    };
    window.addEventListener("agentz:open-source-anchor", onOpenAnchor);
    return () => window.removeEventListener("agentz:open-source-anchor", onOpenAnchor);
  }, [mode]);

  const wikiTitle = useMemo(() => {
    if (!graphIndexStatus || wikiBusy) return t("agent.repoWikiHint");
    switch (graphIndexVisual) {
      case "indexing":
        return t("agent.repoWikiGraphStatusIndexing");
      case "ready":
        return t("agent.repoWikiGraphStatusReady", { nodes: graphIndexStatus.nodes });
      case "error":
        return graphIndexStatus.last_error ?? t("agent.repoWikiGraphError");
      default:
        return t("agent.repoWikiHint");
    }
  }, [graphIndexStatus, graphIndexVisual, wikiBusy, t]);

  const graphMenuStatusLabel = useMemo(() => {
    if (!graphIndexStatus) return null;
    switch (graphIndexVisual) {
      case "indexing":
        return t("agent.repoWikiGraphIndexing");
      case "ready":
        return t("agent.repoWikiGraphReadyShort", { nodes: graphIndexStatus.nodes });
      case "error":
        return t("agent.repoWikiGraphError");
      default:
        return t("agent.repoWikiGraphEmpty");
    }
  }, [graphIndexStatus, graphIndexVisual, t]);

  const handleThemeToggle = useCallback(() => {
    const next = toggleAppearanceTheme();
    setAppearance(next);
    syncEditorThemeWithAppearance(next);
  }, []);

  // Drag the divider between the editor and the chat panel. The chat sits on
  // the right, so dragging the handle left widens it.
  const startChatResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const startX = e.clientX;
      const startW = chatWidth;
      const onMove = (ev: MouseEvent) => {
        const delta = startX - ev.clientX;
        setChatWidth(Math.min(760, Math.max(340, startW + delta)));
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
    [chatWidth],
  );

  useEffect(() => {
    localStorage.setItem("agentz-chat-width", String(chatWidth));
  }, [chatWidth]);

  return (
    <ProjectEdgeProvider>
    <div className="agentz-app">
      <header className="agentz-titlebar">
        <div className="agentz-brand" aria-label={`AgentZ v${__APP_VERSION__}`}>
          <span className="agentz-brand-text">Agent</span>
          <ZLogo size={22} className="agentz-brand-z" />
          <span className="agentz-brand-ver" title={`v${__APP_VERSION__}`}>
            v{__APP_VERSION__}
          </span>
        </div>

        <div className="agentz-project-menu" ref={projectMenuRef}>
          <button
            type="button"
            className={`agentz-titlebar-icon agentz-titlebar-folder ${projectMenuOpen ? "active" : ""}`}
            onClick={() => {
              if (recentProjects.length === 0) {
                void pickFolder();
              } else {
                setProjectMenuOpen((v) => !v);
              }
            }}
            title={projectDir ? t("app.changeFolder") : t("app.openFolder")}
            aria-label={projectDir ? t("app.changeFolder") : t("app.openFolder")}
            aria-expanded={projectMenuOpen}
          >
            <FolderIcon />
          </button>
          {projectMenuOpen &&
            (() => {
              const node = (
                <div
                  ref={projectPopoverRef}
                  className="agentz-project-popover is-fixed"
                  role="menu"
                  style={projectPopoverStyle}
                >
                  <button
                    type="button"
                    className="agentz-project-popover-pick"
                    role="menuitem"
                    onClick={() => {
                      setProjectMenuOpen(false);
                      void pickFolder();
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
                          const isCurrent = Boolean(
                            projectDir && normProjectPath(p.path) === normProjectPath(projectDir),
                          );
                          return (
                            <div
                              key={p.path}
                              className={`agentz-project-popover-row ${isCurrent ? "active" : ""}`}
                            >
                              <button
                                type="button"
                                className="agentz-project-popover-item"
                                role="menuitem"
                                title={p.path}
                                onClick={() => void openRecentProject(p.path)}
                              >
                                <strong>{p.name}</strong>
                                <span>{p.path}</span>
                              </button>
                              <button
                                type="button"
                                className="agentz-project-popover-remove"
                                title={t("app.removeRecentProject")}
                                aria-label={t("app.removeRecentProject")}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  removeRecentProject(p.path);
                                }}
                              >
                                ×
                              </button>
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
          onClick={() => {
            if (recentProjects.length > 0) setProjectMenuOpen((v) => !v);
            else void pickFolder();
          }}
        >
          {projectDir ?? t("app.noProjectOpen")}
        </span>
        {projectDir && (
          <button
            type="button"
            className="agentz-titlebar-icon agentz-titlebar-close-project"
            onClick={() => void closeProject()}
            title={t("app.closeFolder")}
            aria-label={t("app.closeFolder")}
          >
            <CloseIcon size={16} />
          </button>
        )}

        <div className="agentz-titlebar-actions">
          {mode === "codez" && (
            <button
              type="button"
              className={`agentz-titlebar-icon ${browserOpen ? "active" : ""}`}
              onClick={() => projectDir && setBrowserOpen((v) => !v)}
              disabled={!projectDir}
              title={projectDir ? t("app.browserTitle") : t("app.browserNeedsProject")}
              aria-label={projectDir ? t("app.browserTitle") : t("app.browserNeedsProject")}
            >
              <BrowserIcon />
            </button>
          )}
          {mode === "codez" && (
            <button
              type="button"
              className={`agentz-titlebar-icon ${chatOpen ? "active" : ""}`}
              onClick={() => setChatOpen((v) => !v)}
              title={t("app.chatTitle")}
              aria-label={t("app.chatTitle")}
            >
              <ChatIcon />
            </button>
          )}
          {hasImAssistant && (
            <button
              type="button"
              className={`agentz-titlebar-icon ${assistantPanelOpen ? "active" : ""}`}
              onClick={() => setAssistantPanelOpen((v) => !v)}
              title={t("assistantPanel.title")}
              aria-label={t("assistantPanel.title")}
            >
              <AssistantBubbleIcon />
            </button>
          )}
          <div className="agentz-wiki-menu" ref={wikiMenuRef}>
            <button
              type="button"
              className={[
                "agentz-titlebar-icon",
                wikiMenuOpen ? "active" : "",
                wikiBusy ? "loading" : "",
                !wikiBusy && graphIndexVisual === "indexing" ? "graph-indexing" : "",
                !wikiBusy && graphIndexVisual === "ready" ? "graph-ready" : "",
                !wikiBusy && graphIndexVisual === "error" ? "graph-error" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              onClick={() => setWikiMenuOpen((v) => !v)}
              disabled={!projectDir || wikiBusy}
              title={wikiTitle}
              aria-label={t("agent.repoWiki")}
              aria-expanded={wikiMenuOpen}
              aria-haspopup="menu"
            >
              <WikiIcon />
            </button>
            {wikiMenuOpen &&
              (() => {
                const node = (
                  <div
                    ref={wikiPopoverRef}
                    className="agentz-wiki-popover is-fixed"
                    role="menu"
                    style={wikiPopoverStyle}
                  >
                    <button
                      type="button"
                      role="menuitem"
                      onClick={() => {
                        setWikiMenuOpen(false);
                        setCodeMapOpen(true);
                      }}
                    >
                      {t("agent.codeMapOpen")}
                      <span>{t("agent.codeMapOpenHint")}</span>
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      onClick={() => handleWikiAction("overview")}
                    >
                      {t("agent.repoWikiOverview")}
                      <span>{t("agent.repoWikiOverviewHint")}</span>
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      onClick={() => handleWikiAction("deep")}
                    >
                      {t("agent.repoWikiDeep")}
                      <span>{t("agent.repoWikiDeepHint")}</span>
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      onClick={() => handleWikiAction("memory")}
                    >
                      {t("agent.repoWikiMemory")}
                      <span>{t("agent.repoWikiMemoryHint")}</span>
                    </button>
                    <button
                      type="button"
                      role="menuitem"
                      className={[
                        graphIndexVisual === "indexing" ? "graph-indexing" : "",
                        graphIndexVisual === "ready" ? "graph-ready" : "",
                        graphIndexVisual === "error" ? "graph-error" : "",
                      ]
                        .filter(Boolean)
                        .join(" ")}
                      onClick={() => handleWikiAction("graph")}
                    >
                      <span className="agentz-wiki-menu-row">
                        {t("agent.repoWikiGraph")}
                        {graphMenuStatusLabel && (
                          <span className="agentz-wiki-graph-status">{graphMenuStatusLabel}</span>
                        )}
                      </span>
                      <span>{t("agent.repoWikiGraphHint")}</span>
                    </button>
                  </div>
                );
                return canPortalToBody() ? createPortal(node, document.body) : node;
              })()}
          </div>
          <div className="agentz-font-scale-menu" ref={fontScaleRef}>
            <button
              type="button"
              className={`agentz-titlebar-icon${fontScaleOpen ? " active" : ""}`}
              title={t("app.fontScaleTitle")}
              aria-label={t("app.fontScale")}
              aria-expanded={fontScaleOpen}
              aria-haspopup="menu"
              onClick={() => setFontScaleOpen((v) => !v)}
            >
              Aa
            </button>
            {fontScaleOpen &&
              (() => {
                const node = (
                  <div
                    ref={fontScalePopoverRef}
                    className="agentz-font-scale-popover is-fixed"
                    role="menu"
                    style={fontScalePopoverStyle}
                  >
                    {UI_FONT_SCALES.map((s) => (
                      <button
                        key={s}
                        type="button"
                        role="menuitemradio"
                        aria-checked={fontScale === s}
                        className={fontScale === s ? "active" : ""}
                        onClick={() => {
                          applyUiFontScale(s);
                          setFontScale(s);
                          setFontScaleOpen(false);
                        }}
                      >
                        {uiFontScaleLabel(s)}
                      </button>
                    ))}
                    <div className="agentz-font-scale-divider" />
                    <div className="agentz-font-scale-editor-label">{t("app.editorFontSize")}</div>
                    <div className="agentz-font-scale-stepper">
                      <button
                        type="button"
                        data-testid="editor-font-decrease"
                        onClick={() => setEditorBaseFontSizeState(stepEditorBaseFontSize(-1))}
                      >
                        −
                      </button>
                      <button
                        type="button"
                        className="agentz-font-scale-readout"
                        data-testid="editor-font-reset"
                        onClick={() =>
                          setEditorBaseFontSizeState(setEditorBaseFontSize(DEFAULT_EDITOR_FONT_SIZE))
                        }
                      >
                        {getEffectiveEditorFontSize(fontScale, editorBaseFontSize)}px
                      </button>
                      <button
                        type="button"
                        data-testid="editor-font-increase"
                        onClick={() => setEditorBaseFontSizeState(stepEditorBaseFontSize(1))}
                      >
                        +
                      </button>
                    </div>
                  </div>
                );
                return canPortalToBody() ? createPortal(node, document.body) : node;
              })()}
          </div>
          <button
            type="button"
            className="agentz-titlebar-icon"
            onClick={handleThemeToggle}
            title={
              appearance === "dark" ? t("app.themeSwitchLight") : t("app.themeSwitchDark")
            }
            aria-label={
              appearance === "dark" ? t("app.themeSwitchLight") : t("app.themeSwitchDark")
            }
          >
            {appearance === "dark" ? <SunIcon /> : <MoonIcon />}
          </button>
          <button
            type="button"
            className="agentz-titlebar-icon"
            onClick={() => {
              setLibraryInitial(null);
              setLibraryOpen(true);
            }}
            title={t("library.title")}
            aria-label={t("library.title")}
          >
            <StoreIcon />
          </button>
          <button
            type="button"
            className="agentz-titlebar-icon"
            onClick={() => setSettingsOpen(true)}
            title={t("app.settingsTitle")}
            aria-label={t("app.settingsTitle")}
          >
            <SettingsIcon />
          </button>
          <NotificationInbox projectDir={projectDir} onOpenTask={(taskId) => {
            if (!projectDir) return;
            setInboxTaskRoute({ projectDir, taskId, nonce: Date.now() });
            setMode("workz");
          }} />
        </div>

        <div className="agentz-mode-switch">
          <button
            className={mode === "workz" ? "active" : ""}
            onClick={() => setMode("workz")}
            title={`${t("app.modeWorkZTitle")} (${modeShortcutLabel("workz", isMac)})`}
          >
            {t("app.modeWorkZ")}
          </button>
          <button
            className={mode === "codez" ? "active" : ""}
            onClick={() => setMode("codez")}
            title={`${t("app.modeCodeZTitle")} (${modeShortcutLabel("codez", isMac)})`}
          >
            {t("app.modeCodeZ")}
          </button>
        </div>
      </header>

      {codeMapOpen && projectDir && (
        <Suspense fallback={null}>
          <CodeMapPanel
          projectDir={projectDir}
          onClose={() => setCodeMapOpen(false)}
          onOpenFile={(rel) => {
            setCodeMapOpen(false);
            setMode("codez");
            setIdeWikiOpenPath({
              path: `${projectDir.replace(/[\\/]+$/, "")}/${rel}`,
              nonce: Date.now(),
            });
          }}
          onAskAgent={(rel) => {
            setCodeMapOpen(false);
            setMode("codez");
            handleSendToChat([`${projectDir.replace(/[\\/]+$/, "")}/${rel}`]);
          }}
          />
        </Suspense>
      )}

      <main className="agentz-main">
        <div className="agentz-pane" hidden={mode !== "codez"}>
          <div className="agentz-ide-split">
            <div className="agentz-ide-main">
              {codezTaskBinding && (
                <div className="agentz-worktree-handoff-bar" data-testid="worktree-handoff-bar">
                  <span className="agentz-worktree-handoff-label">
                    {t("agent.handoffBanner", {
                      id: codezTaskBinding.taskId.slice(0, 8),
                      path: codezTaskBinding.worktreePath,
                    })}
                  </span>
                  <button
                    type="button"
                    className="agentz-worktree-handoff-return"
                    data-testid="worktree-handoff-return"
                    onClick={() => {
                      const origin = codezTaskBinding.originProjectDir;
                      const taskId = codezTaskBinding.taskId;
                      setCodezTaskBinding(null);
                      setMode("workz");
                      if (origin && taskId) {
                        setInboxTaskRoute({ projectDir: origin, taskId, nonce: Date.now() });
                      }
                    }}
                  >
                    {t("agent.returnToWorkZ")}
                  </button>
                </div>
              )}
              <CodeZWorkspace
                projectDir={codezTaskBinding?.worktreePath ?? projectDir}
                onOpenFolder={pickFolder}
                recentProjects={recentProjects}
                onOpenRecentProject={(dir) => void openRecentProject(dir)}
                onRemoveRecentProject={removeRecentProject}
                onSendToChat={handleSendToChat}
                onSendTerminalToChat={handleSendTerminalToChat}
                openPathRequest={ideWikiOpenPath}
                onOpenPathRequestHandled={() => setIdeWikiOpenPath(null)}
                browserOpen={browserOpen}
                onBrowserOpenChange={setBrowserOpen}
                onSendElementToChat={handleSendElementToChat}
                onScreenshotToChat={handleScreenshotToChat}
                workspaceRestore={workspaceRestore}
                onWorkspacePatch={handleWorkspacePatch}
                onRegisterNavigationSnapshot={registerNavigationSnapshot}
              />
            </div>
            <div
              className="agentz-ide-chat-resize"
              onMouseDown={startChatResize}
              hidden={!chatOpen}
              role="separator"
              aria-orientation="vertical"
            />
            {/* Kept mounted while in IDE mode so toggling chat visibility never
                discards the active session — only show/hide. */}
            <div className="agentz-ide-chat" style={{ width: chatWidth }} hidden={!chatOpen}>
              <IdeSidePanel
                projectDir={codezTaskBinding?.worktreePath ?? projectDir}
                onOpenFolder={pickFolder}
                insertRequest={chatInsert}
                insertElementRequest={chatInsertElement}
                insertTerminalRequest={chatInsertTerminal}
                attachRequest={chatAttach}
                onAttachRequestHandled={() => setChatAttach(null)}
              />
            </div>
          </div>
        </div>
        <div className="agentz-pane" hidden={mode !== "workz"}>
          <WorkZWorkspace
            projectDir={projectDir}
            openTaskRequest={inboxTaskRoute}
            onOpenFolder={pickFolder}
            onOpenProjectInCodeZ={(dir) => void openProjectInCodeZ(dir)}
            onOpenTaskWorktreeInCodeZ={openTaskWorktreeInCodeZ}
            wikiBuildNonce={wikiBuildNonce}
            wikiBuildAction={wikiBuildAction}
            onWikiBusyChange={setWikiBusy}
            onOpenLibrary={(initial) => {
              setLibraryInitial(initial ?? null);
              setLibraryOpen(true);
            }}
          />
        </div>
      </main>

      {settingsOpen && (
        <SettingsPanel
          onClose={() => {
            setSettingsOpen(false);
            window.dispatchEvent(new CustomEvent(SETTINGS_CLOSED_EVENT));
          }}
          projectDir={projectDir}
        />
      )}
      {libraryOpen && (
        <ResourceLibraryPanel
          initial={libraryInitial}
          onClose={() => {
            setLibraryOpen(false);
            setLibraryInitial(null);
          }}
        />
      )}
      {assistantPanelOpen && (
        <AssistantMessagesPanel onClose={() => setAssistantPanelOpen(false)} />
      )}
      {templateProjectDir && (
        <ProjectTemplateDialog
          projectDir={templateProjectDir}
          onDone={() => setTemplateProjectDir(null)}
          onSkip={() => setTemplateProjectDir(null)}
        />
      )}
      <OnboardingModal
        open={onboardingOpen}
        hasConfiguredModel={hasConfiguredModel}
        currentMode={mode}
        onOpenSettings={() => setSettingsOpen(true)}
        onSelectMode={(nextMode) => setMode(nextMode)}
        onFinish={() => setOnboardingOpen(false)}
        onDismiss={() => setOnboardingOpen(false)}
      />

      {/* Project-scoped extension host — must live outside CodeZ/WorkZ panes so
          workspace auto-restore always boots extensions (not only after manual
          folder pick or switching back to CodeZ). */}
      <ExtensionHostProvider projectDir={projectDir} ready={workspaceReady} />

      {(exitToast || settingsToast) && (
        <div className="agentz-exit-toast" role="status" aria-live="polite">
          {exitToast ? t("app.exiting") : settingsToast}
        </div>
      )}

      <ProjectEdgeShell projectDir={projectDir} />
    </div>
    </ProjectEdgeProvider>
  );
}
