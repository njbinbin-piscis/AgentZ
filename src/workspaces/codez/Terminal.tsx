import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import i18n from "../../i18n";
import { Terminal as XTerm } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { confirmDialog } from "../../services/tauri/confirm";
import { ideApi, onTerminalOutput } from "../../services/tauri/ide";
import { acquireTerminal, terminalBackendId } from "./terminalLifecycle";
import { cssVar } from "./useAppTheme";
import { getEffectiveTerminalFontSize } from "../../theme";
import { perfCounters } from "../../utils/perfCounters";

export interface TerminalTab {
  id: string;
  /** 1-based tab index — label is derived at render time via i18n. */
  seq: number;
}

/** Build an xterm theme from the app's CSS theme tokens so the terminal
 *  matches light/dark. Reads computed custom properties at call time. */
function buildXtermTheme(): Record<string, string> {
  const isLight = document.documentElement.dataset.theme === "light";
  return {
    background: cssVar("--bg-primary", isLight ? "#ffffff" : "#14141c"),
    foreground: cssVar("--text-primary", isLight ? "#1a1a24" : "#e8e8f0"),
    cursor: cssVar("--accent", "#7c6af7"),
    cursorAccent: cssVar("--bg-primary", isLight ? "#ffffff" : "#14141c"),
    selectionBackground: isLight ? "rgba(107, 88, 232, 0.22)" : "rgba(124, 106, 247, 0.35)",
  };
}

/** Stable unique id; title is assigned separately from panel-local sequence. */
function newTerminalId(panelId: string, seq: number): string {
  return `ide-term-${panelId}-${seq}`;
}

interface TerminalSessionProps {
  terminalId: string;
  projectDir: string;
  active: boolean;
  panelVisible: boolean;
  onSendSelectionToChat?: (text: string) => void;
}

// Dock keyboard selection keeps focus on its tab until the user enters the pane.
function dockTabOwnsFocus(container: HTMLDivElement | null): boolean {
  const focused = document.activeElement;
  return Boolean(focused?.closest('[role="tablist"]') && container?.closest(".ide-bottom-panel")?.contains(focused));
}

/** One PTY-backed xterm instance. Stays mounted while its tab exists. */
function TerminalSession({
  terminalId,
  projectDir,
  active,
  panelVisible,
  onSendSelectionToChat,
}: TerminalSessionProps) {
  const { t } = useTranslation();
  const termRef = useRef<XTerm | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const unlistenRef = useRef<(() => void) | null>(null);
  const initializedRef = useRef(false);
  const backendIdRef = useRef<string | null>(null);
  const readyRef = useRef(false);
  const pendingExtWritesRef = useRef<string[]>([]);
  const [activatedProject, setActivatedProject] = useState<string | null>(null);
  const selectionCallbackRef = useRef(onSendSelectionToChat);
  selectionCallbackRef.current = onSendSelectionToChat;
  const visibilityRef = useRef({ active, panelVisible });
  visibilityRef.current = { active, panelVisible };
  const activationProjectRef = useRef(projectDir);
  useEffect(() => {
    const changed = activationProjectRef.current !== projectDir;
    activationProjectRef.current = projectDir;
    if (panelVisible && projectDir) setActivatedProject(projectDir);
    else if (changed) setActivatedProject(null);
  }, [panelVisible, projectDir]);
  const [selectionMenu, setSelectionMenu] = useState<{ x: number; y: number } | null>(null);

  const sendSelection = useCallback(() => {
    const text = termRef.current?.getSelection() ?? "";
    if (!text.trim() || !onSendSelectionToChat) return;
    onSendSelectionToChat(text);
    setSelectionMenu(null);
  }, [onSendSelectionToChat]);

  const fitTerminal = useCallback(() => {
    if (!containerRef.current || !fitRef.current || !termRef.current) return;
    try {
      const prevCols = termRef.current.cols;
      const prevRows = termRef.current.rows;
      fitRef.current.fit();
      const { cols, rows } = termRef.current;
      if (cols <= 0 || rows <= 0) return;
      if (cols === prevCols && rows === prevRows) return;
      if (backendIdRef.current) ideApi.terminalResize(backendIdRef.current, cols, rows).catch(() => {});
    } catch {
      // Container may still be laying out.
    }
  }, []);

  // Only spawn a PTY once the terminal panel is shown — avoids background
  // sessions that block project close/switch (confirm dialog) on folder open.
  useEffect(() => {
    if (!projectDir || activatedProject !== projectDir || initializedRef.current || !containerRef.current) return;
    initializedRef.current = true;

    const term = new XTerm({
      cursorBlink: true,
      fontSize: getEffectiveTerminalFontSize(),
      fontFamily: 'Consolas, "Courier New", monospace',
      lineHeight: 1.2,
      scrollback: 5000,
      theme: buildXtermTheme(),
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon());
    term.open(containerRef.current);

    termRef.current = term;
    fitRef.current = fit;

    const backendId = `ide-session-${crypto.randomUUID()}`;
    backendIdRef.current = backendId;
    let cancelled = false;
    let lease: ReturnType<typeof acquireTerminal> | undefined;

    void (async () => {
      const unlisten = perfCounters.trackListener(await onTerminalOutput((evt) => {
        if (!cancelled && evt.id === backendId) term.write(evt.data);
      }));
      if (cancelled) {
        unlisten();
        term.dispose();
        return;
      }
      unlistenRef.current = unlisten;

      term.onData((data) => {
        ideApi.terminalWrite(backendId, data).catch(() => {});
      });

      term.onResize(({ cols, rows }) => {
        ideApi.terminalResize(backendId, cols, rows).catch(() => {});
      });

      term.attachCustomKeyEventHandler((event) => {
        if (
          event.type === "keydown" &&
          event.ctrlKey &&
          event.shiftKey &&
          event.key.toLowerCase() === "l"
        ) {
          const sel = term.getSelection();
          if (sel.trim() && selectionCallbackRef.current) {
            selectionCallbackRef.current(sel);
            return false;
          }
        }
        return true;
      });

      lease = acquireTerminal(terminalId,
        async () => {
          await ideApi.terminalCreate(backendId, projectDir, term.cols || 80, term.rows || 24);
          perfCounters.resourceOpened("terminal");
        },
        async () => {
          await ideApi.terminalDestroy(backendId);
          perfCounters.resourceClosed("terminal");
        }, backendId);
      await lease.ready;

      if (cancelled) {
        await lease.dispose().catch(() => {});
        return;
      }
      readyRef.current = true;
      if (pendingExtWritesRef.current.length > 0) {
        const queued = pendingExtWritesRef.current.splice(0);
        for (const chunk of queued) {
          ideApi.terminalWrite(backendId, chunk).catch(() => {});
        }
      }
      requestAnimationFrame(() => {
        if (cancelled) return;
        fitTerminal();
        if (visibilityRef.current.active && visibilityRef.current.panelVisible && !dockTabOwnsFocus(containerRef.current)) term.focus();
      });
    })().catch((error: unknown) => {
      if (!cancelled) term.write(`\r\n${String(error)}\r\n`);
    });

    return () => {
      cancelled = true;
      readyRef.current = false;
      unlistenRef.current?.();
      unlistenRef.current = null;
      void lease?.dispose().catch(() => {});
      termRef.current?.dispose();
      termRef.current = null;
      backendIdRef.current = null;
      fitRef.current = null;
      initializedRef.current = false;
    };
  }, [terminalId, projectDir, activatedProject, fitTerminal]);

  useEffect(() => {
    const onExtSend = (e: Event) => {
      if (!visibilityRef.current.active) return;
      const detail = (e as CustomEvent<{ text?: string; addNewLine?: boolean; data?: string }>).detail;
      const chunk =
        typeof detail?.data === "string"
          ? detail.data
          : typeof detail?.text === "string"
            ? detail.addNewLine !== false
              ? `${detail.text}\r`
              : detail.text
            : "";
      if (!chunk) return;
      if (readyRef.current && backendIdRef.current) {
        ideApi.terminalWrite(backendIdRef.current, chunk).catch(() => {});
      } else {
        pendingExtWritesRef.current.push(chunk);
      }
    };
    window.addEventListener("agentz-ext-terminal-send", onExtSend);
    return () => window.removeEventListener("agentz-ext-terminal-send", onExtSend);
  }, []);

  useEffect(() => {
    if (!active || !panelVisible) return;
    const timer = window.setTimeout(() => {
      fitTerminal();
      if (!dockTabOwnsFocus(containerRef.current)) termRef.current?.focus();
    }, 60);
    return () => window.clearTimeout(timer);
  }, [active, panelVisible, fitTerminal]);

  // Re-theme the live terminal when the app appearance toggles light/dark.
  useEffect(() => {
    const apply = () => {
      if (termRef.current) termRef.current.options.theme = buildXtermTheme();
    };
    const observer = new MutationObserver(apply);
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  }, []);

  const fontScalePauseRef = useRef(0);
  useEffect(() => {
    const onFontScale = () => {
      fontScalePauseRef.current = performance.now() + 400;
      if (termRef.current) {
        termRef.current.options.fontSize = getEffectiveTerminalFontSize();
        requestAnimationFrame(() => fitTerminal());
      }
    };
    window.addEventListener("agentz-font-scale", onFontScale);
    return () => window.removeEventListener("agentz-font-scale", onFontScale);
  }, [fitTerminal]);

  useEffect(() => {
    if (!active || !panelVisible || !containerRef.current) return;
    let raf = 0;
    const ro = new ResizeObserver(() => {
      if (performance.now() < fontScalePauseRef.current) return;
      if (raf) cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        raf = 0;
        fitTerminal();
      });
    });
    ro.observe(containerRef.current);
    return () => {
      if (raf) cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, [active, panelVisible, fitTerminal]);

  useEffect(() => {
    if (!selectionMenu) return;
    const close = () => setSelectionMenu(null);
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [selectionMenu]);

  return (
    <>
      <div
        className="ide-terminal-session"
        ref={containerRef}
        style={{ display: active ? "block" : "none" }}
        onContextMenu={(e) => {
          const sel = termRef.current?.getSelection() ?? "";
          if (!sel.trim() || !onSendSelectionToChat) return;
          e.preventDefault();
          setSelectionMenu({ x: e.clientX, y: e.clientY });
        }}
      />
      {selectionMenu && (
        <div
          className="ide-tab-context-menu"
          style={{ position: "fixed", left: selectionMenu.x, top: selectionMenu.y, zIndex: 1000 }}
        >
          <button type="button" onClick={sendSelection}>
            {t("ide.terminalSendSelectionToChat")}
            <span style={{ opacity: 0.5, fontSize: 11, marginLeft: 8 }}>
              {t("ide.terminalSendSelectionShortcut")}
            </span>
          </button>
        </div>
      )}
    </>
  );
}

interface TerminalPanelProps {
  projectDir: string;
  /** Panel visibility — hiding does not destroy PTY sessions. */
  visible: boolean;
  onHide: () => void;
  height?: number;
  /** When embedded in the unified BottomPanel, drop the standalone panel chrome
   *  (outer height/border/close) and fill the host container instead. */
  embedded?: boolean;
  onSendSelectionToChat?: (text: string) => void;
}

export default function TerminalPanel({
  projectDir,
  visible,
  onHide,
  height = 200,
  embedded = false,
  onSendSelectionToChat,
}: TerminalPanelProps) {
  const { t } = useTranslation();
  const panelIdRef = useRef(`p-${Math.random().toString(36).slice(2, 9)}`);
  const seqRef = useRef(0);
  const [tabs, setTabs] = useState<TerminalTab[]>([]);
  const [activeTabId, setActiveTabId] = useState("");
  const projectEpochRef = useRef({ projectDir });
  if (projectEpochRef.current.projectDir !== projectDir) projectEpochRef.current = { projectDir };

  const allocTab = useCallback((): TerminalTab => {
    seqRef.current += 1;
    const seq = seqRef.current;
    return {
      id: newTerminalId(panelIdRef.current, seq),
      seq,
    };
  }, []);

  // Initialize / reset tabs when project opens or changes.
  useEffect(() => {
    const first = allocTab();
    setTabs([first]);
    setActiveTabId(first.id);
  }, [projectDir, allocTab]);

  const addTab = useCallback(() => {
    const tab = allocTab();
    setTabs((prev) => [...prev, tab]);
    setActiveTabId(tab.id);
  }, [allocTab]);

  const closeTab = useCallback(
    async (id: string) => {
      const epoch = projectEpochRef.current;
      try {
        const backendId = terminalBackendId(id);
        const alive = backendId ? await ideApi.terminalIsAlive(backendId) : false;
        if (alive) {
          const ok = await confirmDialog(t("ide.terminalCloseTabConfirm"));
          if (!ok) return;
        }
      } catch {
        const ok = await confirmDialog(t("ide.terminalCloseTabConfirm"));
        if (!ok) return;
      }
      if (projectEpochRef.current !== epoch) return;
      setTabs((prev) => {
        if (projectEpochRef.current !== epoch || prev.length <= 1 || !prev.some(tab => tab.id === id)) return prev;
        const next = prev.filter((tab) => tab.id !== id);
        if (activeTabId === id) {
          const idx = prev.findIndex((tab) => tab.id === id);
          const fallback = next[Math.min(idx, next.length - 1)];
          setActiveTabId(fallback.id);
        }
        return next;
      });
    },
    [activeTabId, t],
  );

  const canAddTab = Boolean(projectDir);
  const singleTab = tabs.length <= 1;

  const className = embedded
    ? "ide-terminal-panel is-embedded"
    : `ide-terminal-panel${visible ? "" : " is-hidden"}`;

  return (
    <div
      className={className}
      style={!embedded && visible ? { height } : undefined}
      aria-hidden={!visible}
    >
      <div className="ide-terminal-header">
        <div className="ide-terminal-tabs">
          {tabs.map((tab) => {
            const label = t("ide.terminalTab", { n: tab.seq });
            return (
            <div
              key={tab.id}
              className={`ide-terminal-tab${tab.id === activeTabId ? " active" : ""}`}
              onClick={() => setActiveTabId(tab.id)}
              title={label}
            >
              <span className="ide-terminal-tab-label">{label}</span>
              <button
                type="button"
                className="ide-terminal-tab-close"
                title={t("ide.terminalCloseTab")}
                disabled={singleTab}
                onClick={(e) => {
                  e.stopPropagation();
                  void closeTab(tab.id);
                }}
              >
                ×
              </button>
            </div>
            );
          })}
          <button
            type="button"
            className="ide-terminal-tab-add"
            onClick={addTab}
            disabled={!canAddTab}
            title={canAddTab ? t("ide.terminalNewTab") : t("ide.terminalNeedProject")}
          >
            +
          </button>
        </div>
        {!embedded && (
          <button type="button" className="ide-terminal-panel-close" onClick={onHide} title={t("ide.terminalHidePanel")}>
            ✕
          </button>
        )}
      </div>
      <div className="ide-terminal-body">
        {tabs.map((tab) => (
          <TerminalSession
            key={tab.id}
            terminalId={tab.id}
            projectDir={projectDir}
            active={tab.id === activeTabId}
            panelVisible={visible}
            onSendSelectionToChat={onSendSelectionToChat}
          />
        ))}
      </div>
    </div>
  );
}

/** Ask user before switching project if terminals are active; returns false if cancelled. */
export async function confirmTerminalCloseOnProjectChange(
  t: (key: string, opts?: { count: number }) => string,
): Promise<boolean> {
  try {
    const count = await ideApi.terminalCount();
    if (count <= 0) return true;
    return confirmDialog(t("ide.terminalCloseProjectConfirm", { count }));
  } catch {
    return false;
  }
}

/** Destroy all backend PTY sessions (after user confirmed project change). */
export async function destroyAllTerminals(): Promise<void> {
  try {
    await ideApi.terminalDestroyAll();
  } catch (error) {
    // Fail closed: the caller must not switch or close the project while a shell may still run.
    // Tell the user why the switch was cancelled, then keep the rejection so the caller aborts.
    if (typeof window !== "undefined" && typeof window.alert === "function") {
      window.alert(i18n.t("ide.terminalStopFailedAlert"));
    }
    throw error;
  }
}
