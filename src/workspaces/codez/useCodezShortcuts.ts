import { useEffect, useMemo, type MutableRefObject } from "react";
import type { TFunction } from "i18next";
import type { PaletteCommandItem, PaletteMode } from "./CommandPalette";
import type { BottomTab } from "./BottomPanel";
import { extensionService } from "../../extensions/extensionService";
import { useExtensionUi } from "../../extensions/ui/useExtensionUi";

export type SidebarTab = "explorer" | "search" | "git" | "extensions";

export interface UseCodezShortcutsOptions {
  t: TFunction;
  projectDir: string | null;
  activeTabPathRef: MutableRefObject<string | null>;
  saveFile: (path: string) => Promise<boolean>;
  handleTabClose: (path: string) => void;
  closeAllTabs: () => Promise<void>;
  onOpenFolder: () => void;
  setBottomOpen: (updater: (prev: boolean) => boolean) => void;
  setBottomTab?: (tab: BottomTab) => void;
  setSidebarTab: (tab: SidebarTab) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  setRemoteDialogOpen: (open: boolean) => void;
  setPaletteMode: (mode: PaletteMode) => void;
  setPaletteOpen: (open: boolean) => void;
}

export function useCodezShortcuts({
  t,
  projectDir,
  activeTabPathRef,
  saveFile,
  handleTabClose,
  closeAllTabs,
  onOpenFolder,
  setBottomOpen,
  setSidebarTab,
  setSidebarCollapsed,
  setRemoteDialogOpen,
  setPaletteMode,
  setPaletteOpen,
}: UseCodezShortcutsOptions): { paletteCommands: PaletteCommandItem[] } {
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "p") {
        e.preventDefault();
        setPaletteMode(e.shiftKey ? "commands" : "files");
        setPaletteOpen(true);
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        const active = activeTabPathRef.current;
        if (!active || active.startsWith("diff:")) return;
        saveFile(active);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [activeTabPathRef, saveFile, setPaletteMode, setPaletteOpen]);

  const { running: extRunning } = useExtensionUi();

  const paletteCommands = useMemo<PaletteCommandItem[]>(
    () => {
      const builtins: PaletteCommandItem[] = [
        {
          id: "toggle-terminal",
          label: t("ide.cmdToggleTerminal"),
          shortcut: "Ctrl+`",
          run: () => {
            if (projectDir) setBottomOpen((v) => !v);
          },
        },
        {
          id: "focus-explorer",
          label: t("ide.cmdFocusExplorer"),
          run: () => {
            setSidebarTab("explorer");
            setSidebarCollapsed(false);
          },
        },
        {
          id: "focus-search",
          label: t("ide.cmdFocusSearch"),
          shortcut: "Ctrl+Shift+F",
          run: () => {
            setSidebarTab("search");
            setSidebarCollapsed(false);
          },
        },
        {
          id: "focus-git",
          label: t("ide.cmdFocusGit"),
          run: () => {
            setSidebarTab("git");
            setSidebarCollapsed(false);
          },
        },
        {
          id: "focus-extensions",
          label: t("ide.cmdFocusExtensions"),
          run: () => {
            setSidebarTab("extensions");
            setSidebarCollapsed(false);
          },
        },
        {
          id: "save-active-file",
          label: t("ide.cmdSaveActiveFile"),
          shortcut: "Ctrl+S",
          run: () => {
            const active = activeTabPathRef.current;
            if (active && !active.startsWith("diff:")) void saveFile(active);
          },
        },
        {
          id: "close-active-tab",
          label: t("ide.cmdCloseActiveTab"),
          shortcut: "Ctrl+W",
          run: () => {
            const active = activeTabPathRef.current;
            if (active) handleTabClose(active);
          },
        },
        {
          id: "close-all-tabs",
          label: t("ide.cmdCloseAllTabs"),
          run: () => {
            void closeAllTabs();
          },
        },
        {
          id: "open-folder",
          label: t("ide.cmdOpenFolder"),
          run: () => onOpenFolder(),
        },
        {
          id: "connect-remote",
          label: t("ide.cmdConnectRemote"),
          run: () => setRemoteDialogOpen(true),
        },
      ];
      if (!extRunning) return builtins;
      const extCmds: PaletteCommandItem[] = extensionService
        .listContributedCommands()
        .map((c) => ({
          id: `ext:${c.command}`,
          label: c.category && !c.title.startsWith(c.category) ? `${c.category}: ${c.title}` : c.title,
          run: () => {
            void extensionService.executeCommand(c.command);
          },
        }));
      return [...builtins, ...extCmds];
    },
    [
      t,
      projectDir,
      activeTabPathRef,
      saveFile,
      handleTabClose,
      closeAllTabs,
      onOpenFolder,
      setBottomOpen,
      setSidebarTab,
      setSidebarCollapsed,
      setRemoteDialogOpen,
      extRunning,
    ],
  );

  return { paletteCommands };
}
