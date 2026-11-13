import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  browserClickAt,
  browserClose,
  browserCloseTab,
  browserHistoryStep,
  browserMouseClick,
  browserMouseEvent,
  browserSelectAll,
  browserSelectionText,
  browserInsertText,
  browserPressKey,
  browserCurrentUrl,
  browserInspectAt,
  browserListTabs,
  browserNavigate,
  browserPickAt,
  browserScreenshot,
  browserScrollBy,
  browserScrollInfo,
  browserScrollTo,
  browserSetSurface,
  browserSetViewport,
  browserSwitchTab,
  browserState,
  onBrowserChanged,
  type BrowserTabInfo,
  type PickedElement,
  type ScrollInfo,
} from "../../services/tauri/browser";
import "./BrowserPanel.css";
import { rewriteRemoteBrowserUrl } from "../../extensions/remotePorts";
import { useContextMenu } from "../../components/ContextMenu";

const SHOT_POLL_MIN_MS = 1200;
const SHOT_POLL_MAX_MS = 6000;

const EMPTY_SCROLL: ScrollInfo = {
  scroll_x: 0,
  scroll_y: 0,
  scroll_width: 0,
  scroll_height: 0,
  client_width: 0,
  client_height: 0,
};

interface BrowserPanelProps {
  /** Chat session whose browser tab this panel shows. */
  sessionId: string;
  /** Product surface that owns this panel: codez, workz, or botz. */
  surface?: string;
  /** Active project, so localhost opens through a remote port forward. */
  projectDir?: string | null;
  onClose: () => void;
  onSendElementToChat: (el: PickedElement) => void;
  onScreenshotToChat: (base64: string) => void;
  /** When false, hide send-to-chat actions (requires an open project folder). */
  chatEnabled?: boolean;
  /** Increment to force refresh after agent browser actions. */
  refreshSignal?: number;
  /** Latest agent browser action label (from chat tool_start). */
  agentAction?: string | null;
  /** Ask parent to confirm before closing (returns true to proceed). */
  onRequestClose?: () => Promise<boolean>;
}

function rectStyle(el: PickedElement, img: HTMLImageElement): React.CSSProperties | null {
  const rw = el.rect_width ?? 0;
  const rh = el.rect_height ?? 0;
  if (!rw || !rh || !img.naturalWidth) return null;
  const sx = img.clientWidth / img.naturalWidth;
  const sy = img.clientHeight / img.naturalHeight;
  return {
    left: (el.rect_x ?? 0) * sx,
    top: (el.rect_y ?? 0) * sy,
    width: rw * sx,
    height: rh * sy,
  };
}

/**
 * Embedded browser: viewport tracks the panel size 1:1, devtools-style hover
 * highlight in pick mode, Cursor-style inspector sidebar, agent-shared CDP page.
 */
export default function BrowserPanel({
  sessionId,
  surface = "codez",
  projectDir = null,
  onClose,
  onSendElementToChat,
  onScreenshotToChat,
  chatEnabled = true,
  refreshSignal = 0,
  agentAction = null,
  onRequestClose,
}: BrowserPanelProps) {
  const { t } = useTranslation();
  const pageMenu = useContextMenu();
  const sessionIdRef = useRef(sessionId);
  sessionIdRef.current = sessionId;
  const [address, setAddress] = useState("");
  const [shot, setShot] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pickMode, setPickMode] = useState(false);
  const [hovered, setHovered] = useState<PickedElement | null>(null);
  const [selected, setSelected] = useState<PickedElement | null>(null);
  const [scroll, setScroll] = useState<ScrollInfo>(EMPTY_SCROLL);
  const [pageZoom, setPageZoom] = useState(1);
  const scrollRef = useRef<ScrollInfo>(EMPTY_SCROLL);
  scrollRef.current = scroll;

  const viewRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLDivElement>(null);
  const imgRef = useRef<HTMLImageElement>(null);
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const resizeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inspectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const viewportReady = useRef(false);
  const lastViewportRef = useRef<{ w: number; h: number } | null>(null);
  const syncingViewportRef = useRef(false);
  // Accumulated wheel deltas, flushed to the page on a short timer so a burst
  // of wheel events becomes one scroll + one screenshot refresh.
  const wheelAccum = useRef({ dx: 0, dy: 0 });
  const wheelTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [tabs, setTabs] = useState<BrowserTabInfo[]>([]);
  const lastShotRef = useRef<string | null>(null);
  const refreshTabs = useCallback(async () => {
    try {
      setTabs(await browserListTabs(sessionId));
    } catch {
      setTabs([]);
    }
  }, [sessionId]);

  useEffect(() => {
    void browserSetSurface(sessionId, surface)
      .catch(() => {})
      .then(() => refreshTabs());
  }, [sessionId, surface, refreshTabs]);

  /** Resolves `true` when the page image changed since the previous capture. */
  const refreshShot = useCallback(async (): Promise<boolean> => {
    try {
      const b64 = await browserScreenshot(sessionIdRef.current);
      setError(null);
      if (b64 === lastShotRef.current) return false;
      lastShotRef.current = b64;
      setShot(b64);
      return true;
    } catch {
      // Browser not launched yet — ignore until first navigate.
      return false;
    }
  }, []);

  const refreshScrollInfo = useCallback(async () => {
    try {
      setScroll(await browserScrollInfo(sessionIdRef.current));
    } catch {
      // Page not ready yet.
    }
  }, []);

  const flushWheel = useCallback(async () => {
    const { dx, dy } = wheelAccum.current;
    wheelAccum.current = { dx: 0, dy: 0 };
    if (dx === 0 && dy === 0) return;
    try {
      const info = await browserScrollBy(sessionIdRef.current, dx, dy);
      setScroll(info);
      await refreshShot();
    } catch (e) {
      setError(String(e));
    }
  }, [refreshShot]);
  const flushWheelRef = useRef(flushWheel);
  flushWheelRef.current = flushWheel;
  const pickModeRef = useRef(pickMode);
  pickModeRef.current = pickMode;

  const measureViewport = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    // Canvas fills the view; synthetic scrollbars are absolute overlays and do
    // not shrink clientWidth/Height. Do not subtract gutter here — doing so
    // oscillates with refreshScrollInfo and can spin CPU / crash WebKit.
    const w = Math.max(320, canvas.clientWidth);
    const h = Math.max(200, canvas.clientHeight);
    if (w < 8 || h < 8) return null;
    return { w, h };
  }, []);

  const syncViewport = useCallback(async () => {
    if (syncingViewportRef.current) return;
    const dims = measureViewport();
    if (!dims) return;
    const { w, h } = dims;
    const last = lastViewportRef.current;
    if (last && last.w === w && last.h === h) return;

    syncingViewportRef.current = true;
    try {
      await browserSetViewport(sessionIdRef.current, w, h);
      lastViewportRef.current = { w, h };
      viewportReady.current = true;
      await refreshShot();
      await refreshScrollInfo();
    } catch (e) {
      if (viewportReady.current) setError(String(e));
    } finally {
      syncingViewportRef.current = false;
    }
  }, [measureViewport, refreshShot, refreshScrollInfo]);

  const syncAddressFromBackend = useCallback(async () => {
    try {
      const st = await browserState(sessionIdRef.current);
      if (st.url) setAddress(st.url);
    } catch {
      try {
        const url = await browserCurrentUrl(sessionIdRef.current);
        if (url) setAddress(url);
      } catch {
        /* browser not launched */
      }
    }
  }, []);

  useEffect(() => {
    void syncAddressFromBackend();
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void onBrowserChanged((payload) => {
      const mine = sessionIdRef.current;
      if (payload.session_id && mine && payload.session_id !== mine) return;
      void refreshShot();
      void refreshTabs();
      void syncAddressFromBackend();
      void refreshScrollInfo();
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refreshShot, refreshTabs, refreshScrollInfo, syncAddressFromBackend]);

  // Switching chats shows that session's page, not the one we were just looking at.
  useEffect(() => {
    lastShotRef.current = null;
    setShot(null);
    setAddress("");
    setScroll(EMPTY_SCROLL);
    setSelected(null);
    setHovered(null);
    viewportReady.current = false;
    lastViewportRef.current = null;
    void syncAddressFromBackend();
    void syncViewport();
  }, [sessionId, syncAddressFromBackend, syncViewport]);

  useEffect(() => {
    if (refreshSignal > 0) {
      void refreshShot();
      void syncAddressFromBackend();
    }
  }, [refreshSignal, refreshShot, syncAddressFromBackend]);

  useEffect(() => {
    const canvas = canvasRef.current;
    const view = viewRef.current;
    if (!canvas && !view) return;

    // Wait two frames so flex layout (sidebar, chat panel) has settled before
    // the first measurement — avoids a transient wide width that relayouts the page.
    let raf2 = 0;
    const raf1 = requestAnimationFrame(() => {
      raf2 = requestAnimationFrame(() => void syncViewport());
    });

    const onResize = () => {
      if (resizeTimer.current) clearTimeout(resizeTimer.current);
      resizeTimer.current = setTimeout(() => void syncViewport(), 250);
    };
    const ro = new ResizeObserver(onResize);
    if (canvas) ro.observe(canvas);
    window.addEventListener("resize", onResize);

    const onFontScale = () => {
      lastViewportRef.current = null;
      if (resizeTimer.current) clearTimeout(resizeTimer.current);
      resizeTimer.current = setTimeout(() => void syncViewport(), 400);
    };
    window.addEventListener("agentz-font-scale", onFontScale);

    // Catch page changes that emit no browser event (JS, animations). Back off
    // while the image is static and skip while the panel is not on screen.
    let pollDelay = SHOT_POLL_MIN_MS;
    let pollDisposed = false;
    const pollShot = async () => {
      const visible = !document.hidden && !!canvas && canvas.clientWidth > 0;
      if (visible && viewportReady.current && !syncingViewportRef.current) {
        const changed = await refreshShot();
        pollDelay = changed ? SHOT_POLL_MIN_MS : Math.min(pollDelay * 2, SHOT_POLL_MAX_MS);
      }
      if (!pollDisposed) pollRef.current = setTimeout(() => void pollShot(), pollDelay);
    };
    pollRef.current = setTimeout(() => void pollShot(), pollDelay);

    return () => {
      pollDisposed = true;
      cancelAnimationFrame(raf1);
      cancelAnimationFrame(raf2);
      ro.disconnect();
      window.removeEventListener("resize", onResize);
      window.removeEventListener("agentz-font-scale", onFontScale);
      if (resizeTimer.current) clearTimeout(resizeTimer.current);
      if (pollRef.current) clearTimeout(pollRef.current);
      pollRef.current = null;
      if (wheelTimer.current) clearTimeout(wheelTimer.current);
      wheelTimer.current = null;
    };
  }, [syncViewport, refreshShot]);

  // Native, non-passive wheel listener so we can preventDefault and forward the
  // delta to the page (React's synthetic onWheel is passive — can't cancel).
  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) {
        e.preventDefault();
        const delta = e.deltaY > 0 ? -0.1 : 0.1;
        setPageZoom((z) => {
          const next = Math.round((z + delta) * 10) / 10;
          return Math.min(2, Math.max(0.5, next));
        });
        return;
      }
      if (pickModeRef.current) return;
      e.preventDefault();
      wheelAccum.current.dx += e.deltaX;
      wheelAccum.current.dy += e.deltaY;
      if (wheelTimer.current) return;
      wheelTimer.current = setTimeout(() => {
        wheelTimer.current = null;
        void flushWheelRef.current();
      }, 40);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || e.key !== "0") return;
      e.preventDefault();
      setPageZoom(1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Inspector sidebar steals horizontal space — re-sync viewport after layout.
  useEffect(() => {
    const id = requestAnimationFrame(() => {
      void syncViewport();
    });
    return () => cancelAnimationFrame(id);
  }, [selected, syncViewport]);

  const navigate = useCallback(async () => {
    const url = address.trim();
    if (!url) return;
    setLoading(true);
    setError(null);
    try {
      await syncViewport();
      const opened = await rewriteRemoteBrowserUrl(projectDir, url);
      const finalUrl = await browserNavigate(sessionIdRef.current, opened);
      setAddress(finalUrl || url);
      await refreshTabs();
      await syncViewport();
      await refreshShot();
      await refreshScrollInfo();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [address, projectDir, refreshShot, refreshScrollInfo, refreshTabs, syncViewport]);

  /** Screen position -> page pixel; `clamp` keeps a drag that left the image on its edge. */
  const clientToPage = (clientX: number, clientY: number, clamp = false) => {
    const img = imgRef.current;
    if (!img || !img.naturalWidth) return null;
    const rect = img.getBoundingClientRect();
    let fx = (clientX - rect.left) / rect.width;
    let fy = (clientY - rect.top) / rect.height;
    if (clamp) {
      fx = Math.min(1, Math.max(0, fx));
      fy = Math.min(1, Math.max(0, fy));
    }
    return { x: Math.round(fx * img.naturalWidth), y: Math.round(fy * img.naturalHeight) };
  };
  const toPageCoords = (e: React.MouseEvent<HTMLDivElement>) => clientToPage(e.clientX, e.clientY);

  const scrollTo = useCallback(
    async (x: number, y: number) => {
      try {
        const info = await browserScrollTo(sessionIdRef.current, x, y);
        setScroll(info);
        await refreshShot();
      } catch (e) {
        setError(String(e));
      }
    },
    [refreshShot],
  );

  // Drag a synthetic scrollbar thumb. Maps the pointer position within the
  // track to an absolute page scroll offset, throttled to one call per frame.
  const startScrollDrag = useCallback(
    (axis: "x" | "y") => (e: React.PointerEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      const track = e.currentTarget.parentElement;
      if (!track) return;
      track.setPointerCapture?.(e.pointerId);

      let raf = 0;
      let pending: { x: number; y: number } | null = null;
      const flush = () => {
        raf = 0;
        if (pending) {
          void scrollTo(pending.x, pending.y);
          pending = null;
        }
      };
      const move = (ev: PointerEvent) => {
        const rect = track.getBoundingClientRect();
        const info = scrollRef.current;
        if (axis === "y") {
          const frac = Math.min(1, Math.max(0, (ev.clientY - rect.top) / rect.height));
          const max = Math.max(0, info.scroll_height - info.client_height);
          pending = { x: info.scroll_x, y: frac * max };
        } else {
          const frac = Math.min(1, Math.max(0, (ev.clientX - rect.left) / rect.width));
          const max = Math.max(0, info.scroll_width - info.client_width);
          pending = { x: frac * max, y: info.scroll_y };
        }
        if (!raf) raf = requestAnimationFrame(flush);
      };
      const up = (ev: PointerEvent) => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        track.releasePointerCapture?.(ev.pointerId);
        if (raf) cancelAnimationFrame(raf);
        flush();
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      move(e.nativeEvent);
    },
    [scrollTo],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!pickMode) return;
      const pt = toPageCoords(e);
      if (!pt) return;
      if (inspectTimer.current) clearTimeout(inspectTimer.current);
      inspectTimer.current = setTimeout(() => {
        void browserInspectAt(sessionIdRef.current, pt.x, pt.y)
          .then((el) => setHovered(el))
          .catch(() => setHovered(null));
      }, 60);
    },
    [pickMode],
  );

  const handleMouseLeave = useCallback(() => {
    if (inspectTimer.current) clearTimeout(inspectTimer.current);
    setHovered(null);
  }, []);

  // Text selection by dragging: remember where the press happened, and on
  // release send one press-drag-release to the page. The click that follows a
  // drag must not be forwarded, or it would clear the new selection.
  const justDraggedRef = useRef(false);

  // The page sees the press once the pointer has moved a few pixels, then every
  // move (coalesced to the latest one) and the release, so the selection grows
  // under the cursor. Listeners live on `window`: releasing outside the panel
  // still ends the drag instead of leaving the page's button held down.
  const handleMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      justDraggedRef.current = false;
      if (pickMode || e.button !== 0) return;
      const startPt = clientToPage(e.clientX, e.clientY);
      if (!startPt) return;
      const sid = sessionIdRef.current;
      const origin = { x: e.clientX, y: e.clientY };
      let started = false;
      let inflight = false;
      let pendingMove: { x: number; y: number } | null = null;
      let chain: Promise<unknown> = Promise.resolve();

      const send = (phase: "down" | "move" | "up", pt: { x: number; y: number }) => {
        chain = chain
          .then(() => browserMouseEvent(sid, phase, pt.x, pt.y))
          .catch((err) => setError(String(err)));
        return chain;
      };
      const pumpMove = () => {
        if (inflight || !pendingMove) return;
        const pt = pendingMove;
        pendingMove = null;
        inflight = true;
        void send("move", pt).then(() => {
          inflight = false;
          void refreshShot();
          pumpMove();
        });
      };
      const onMove = (ev: MouseEvent) => {
        if (!started) {
          if (Math.hypot(ev.clientX - origin.x, ev.clientY - origin.y) < 5) return;
          started = true;
          justDraggedRef.current = true;
          void send("down", startPt);
        }
        const pt = clientToPage(ev.clientX, ev.clientY, true);
        if (!pt) return;
        pendingMove = pt;
        pumpMove();
      };
      const onUp = (ev: MouseEvent) => {
        window.removeEventListener("mousemove", onMove);
        window.removeEventListener("mouseup", onUp);
        if (!started) return;
        pendingMove = null;
        const pt = clientToPage(ev.clientX, ev.clientY, true) ?? startPt;
        void send("up", pt).then(() => setTimeout(() => void refreshShot(), 80));
      };
      window.addEventListener("mousemove", onMove);
      window.addEventListener("mouseup", onUp);
    },
    [pickMode, refreshShot],
  );

  const handleCanvasClick = useCallback(
    async (e: React.MouseEvent<HTMLDivElement>) => {
      if (justDraggedRef.current) {
        justDraggedRef.current = false;
        return;
      }
      const pt = toPageCoords(e);
      if (!pt) return;
      try {
        if (pickMode) {
          const el = await browserPickAt(sessionIdRef.current, pt.x, pt.y);
          if (el) setSelected(el);
        } else if (e.detail >= 2) {
          // Double / triple click: select the word / paragraph.
          await browserMouseClick(sessionIdRef.current, pt.x, pt.y, Math.min(3, e.detail));
          setTimeout(() => void refreshShot(), 120);
        } else {
          await browserClickAt(sessionIdRef.current, pt.x, pt.y);
          setTimeout(() => {
            void refreshShot();
            void refreshScrollInfo();
          }, 350);
        }
      } catch (err) {
        setError(String(err));
      }
    },
    [pickMode, refreshShot, refreshScrollInfo],
  );

  // Forward typing to the focused page element. Printable characters go through
  // insertText (IME and shortcuts untouched); navigation/editing keys are real key events.
  const refreshSoon = useCallback(() => {
    setTimeout(() => {
      void refreshShot();
      void refreshScrollInfo();
    }, 150);
  }, [refreshShot, refreshScrollInfo]);

  const handleCanvasKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (pickMode || e.nativeEvent.isComposing) return;
      const mods = (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0);
      const printable = e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey;
      if (printable) {
        e.preventDefault();
        void browserInsertText(sessionIdRef.current, e.key).then(refreshSoon, (err) => setError(String(err)));
        return;
      }
      if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.length === 1) {
        const k = e.key.toLowerCase();
        if (k === "c") {
          // Copy the page's selection to the system clipboard.
          e.preventDefault();
          void browserSelectionText(sessionIdRef.current)
            .then((text) => (text ? navigator.clipboard.writeText(text) : undefined))
            .catch((err) => setError(String(err)));
        } else if (k === "a") {
          e.preventDefault();
          void browserSelectAll(sessionIdRef.current).then(refreshSoon, (err) => setError(String(err)));
        }
        return; // paste (Ctrl+V) still reaches onPaste
      }
      if (e.key === "Shift" || e.key === "Control" || e.key === "Alt" || e.key === "Meta") return;
      e.preventDefault();
      void browserPressKey(sessionIdRef.current, e.key, mods).then(refreshSoon, () => {});
    },
    [pickMode, refreshSoon],
  );

  const handleCanvasPaste = useCallback(
    (e: React.ClipboardEvent<HTMLDivElement>) => {
      const text = e.clipboardData.getData("text/plain");
      if (!text) return;
      e.preventDefault();
      void browserInsertText(sessionIdRef.current, text).then(refreshSoon, (err) => setError(String(err)));
    },
    [refreshSoon],
  );

  const historyStep = useCallback(
    (delta: number) => {
      void browserHistoryStep(sessionIdRef.current, delta)
        .then(() => new Promise((r) => setTimeout(r, 400)))
        .then(() => Promise.all([refreshShot(), syncAddressFromBackend(), refreshScrollInfo()]))
        .catch((err) => setError(String(err)));
    },
    [refreshShot, syncAddressFromBackend, refreshScrollInfo],
  );

  const screenshotToChat = useCallback(async () => {
    try {
      const b64 = await browserScreenshot(sessionIdRef.current);
      onScreenshotToChat(b64);
    } catch (e) {
      setError(String(e));
    }
  }, [onScreenshotToChat]);

  const onCanvasContextMenu = useCallback(
    (e: React.MouseEvent) => {
      const items = [
        { id: "back", label: t("browser.back", { defaultValue: "Back" }), onSelect: () => historyStep(-1) },
        { id: "forward", label: t("browser.forward", { defaultValue: "Forward" }), onSelect: () => historyStep(1) },
        {
          id: "reload",
          label: t("contextMenu.reloadPage"),
          onSelect: () => void syncViewport().then(refreshShot),
        },
        { id: "sep", label: "", separator: true },
        {
          id: "copy-url",
          label: t("contextMenu.copyPageUrl"),
          disabled: !address,
          onSelect: () => void navigator.clipboard?.writeText(address),
        },
        ...(chatEnabled
          ? [
              {
                id: "screenshot",
                label: t("browser.screenshotToChat"),
                onSelect: () => void screenshotToChat(),
              },
            ]
          : []),
      ];
      pageMenu.openWith(() => items)(e);
    },
    [t, historyStep, syncViewport, refreshShot, address, chatEnabled, screenshotToChat, pageMenu],
  );

  const close = useCallback(() => {
    void (async () => {
      if (onRequestClose) {
        const ok = await onRequestClose();
        if (!ok) return;
      }
      void browserClose(sessionIdRef.current).catch(() => {});
      setPickMode(false);
      setHovered(null);
      setSelected(null);
      onClose();
    })();
  }, [onClose, onRequestClose]);

  const togglePick = () => {
    setPickMode((v) => {
      if (v) {
        setHovered(null);
      } else {
        setSelected(null);
      }
      return !v;
    });
  };

  const hoverBox = hovered && imgRef.current ? rectStyle(hovered, imgRef.current) : null;
  const selectBox = selected && imgRef.current ? rectStyle(selected, imgRef.current) : null;

  // Synthetic scrollbar geometry (screenshot view has no native scrollbar).
  const MIN_THUMB = 24;
  const showVScroll = scroll.scroll_height > scroll.client_height + 1 && scroll.client_height > 0;
  const showHScroll = scroll.scroll_width > scroll.client_width + 1 && scroll.client_width > 0;
  const vThumb = showVScroll
    ? {
        height: `max(${MIN_THUMB}px, ${(scroll.client_height / scroll.scroll_height) * 100}%)`,
        top: `${(scroll.scroll_y / scroll.scroll_height) * 100}%`,
      }
    : null;
  const hThumb = showHScroll
    ? {
        width: `max(${MIN_THUMB}px, ${(scroll.client_width / scroll.scroll_width) * 100}%)`,
        left: `${(scroll.scroll_x / scroll.scroll_width) * 100}%`,
      }
    : null;

  return (
    <div className="agentz-browser">
      {tabs.length > 0 && (
        <div className="agentz-browser-tabs" role="tablist">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              role="tab"
              tabIndex={0}
              aria-selected={tab.active}
              className={`agentz-browser-tab${tab.active ? " active" : ""}`}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  e.currentTarget.click();
                }
              }}
              title={t("browser.tabTitleHint", {
                surface: tab.surface,
                url: tab.url || tab.id,
              })}
              onClick={() => {
                if (tab.active) return;
                void browserSwitchTab(sessionId, tab.id)
                  .then(() => refreshTabs())
                  .then(() => refreshShot())
                  .then(() => syncAddressFromBackend())
                  .catch(() => void refreshTabs());
              }}
            >
              <span className="agentz-browser-tab-surface">{tab.surface}</span>
              <span className="agentz-browser-tab-url">{tab.url || tab.id}</span>
              <button
                type="button"
                className="agentz-browser-tab-close"
                aria-label={t("browser.closeTab")}
                title={t("browser.closeTab")}
                onClick={(e) => {
                  e.stopPropagation();
                  void browserCloseTab(sessionId, tab.id)
                    .catch((err) => setError(String(err)))
                    .then(() => refreshTabs())
                    .then(() => refreshShot())
                    .then(() => syncAddressFromBackend());
                }}
              >
                ✕
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="agentz-browser-toolbar">
        <button className="agentz-browser-btn" onClick={() => historyStep(-1)} title={t("browser.back", { defaultValue: "Back" })}>
          ←
        </button>
        <button className="agentz-browser-btn" onClick={() => historyStep(1)} title={t("browser.forward", { defaultValue: "Forward" })}>
          →
        </button>
        <button
          className="agentz-browser-btn"
          onClick={() => void syncViewport().then(refreshShot)}
          title={t("browser.reload")}
        >
          ⟳
        </button>
        <input
          className="agentz-browser-address"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void navigate();
          }}
          placeholder={t("browser.addressPlaceholder")}
          spellCheck={false}
        />
        {agentAction ? (
          <span className="agentz-browser-agent-badge" title={agentAction}>
            {t("browser.agentAction", { action: agentAction })}
          </span>
        ) : null}
        <button
          className={`agentz-browser-btn${pickMode ? " active" : ""}`}
          onClick={togglePick}
          title={t("browser.pickElement")}
        >
          ⌖
        </button>
        <button
          className="agentz-browser-btn"
          onClick={() => void screenshotToChat()}
          disabled={!chatEnabled}
          title={chatEnabled ? t("browser.screenshotToChat") : t("browser.needsProject")}
        >
          ⎙
        </button>
        <button
          className="agentz-browser-btn"
          onClick={() => setPageZoom(1)}
          title={t("browser.zoomReset")}
        >
          {t("browser.zoomLevel", { percent: Math.round(pageZoom * 100) })}
        </button>
        <button className="agentz-browser-btn" onClick={close} title={t("browser.close")}>
          ✕
        </button>
      </div>

      {error && <div className="agentz-browser-error">{error}</div>}
      {pickMode && <div className="agentz-browser-pickhint">{t("browser.pickHint")}</div>}

      <div className="agentz-browser-body">
        <div className="agentz-browser-view" ref={viewRef}>
          <div
            className="agentz-browser-canvas"
            ref={canvasRef}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseLeave={handleMouseLeave}
            tabIndex={0}
            onKeyDown={handleCanvasKeyDown}
            onPaste={handleCanvasPaste}
            onContextMenu={onCanvasContextMenu}
            onClick={(e) => {
              e.currentTarget.focus();
              void handleCanvasClick(e);
            }}
          >
            {loading && (
              <div className="agentz-browser-loading-overlay">{t("browser.loading")}</div>
            )}
            {shot ? (
              <>
                <img
                  ref={imgRef}
                  className={`agentz-browser-shot${pickMode ? " pick" : ""}`}
                  src={`data:image/png;base64,${shot}`}
                  alt="page"
                  draggable={false}
                  style={{
                    ...(scroll.client_width > 0
                      ? { width: scroll.client_width, height: scroll.client_height }
                      : {}),
                    ...(pageZoom !== 1
                      ? { transform: `scale(${pageZoom})`, transformOrigin: "top left" }
                      : {}),
                  }}
                />
                {pickMode && hoverBox && (
                  <div className="agentz-browser-highlight hover" style={hoverBox} />
                )}
                {selectBox && (
                  <div className="agentz-browser-highlight selected" style={selectBox} />
                )}
              </>
            ) : (
              <div className="agentz-browser-empty">{t("browser.empty")}</div>
            )}
          </div>

          {vThumb && (
            <div className="agentz-browser-scrollbar vertical">
              <div
                className="agentz-browser-scrollthumb"
                style={vThumb}
                onPointerDown={startScrollDrag("y")}
              />
            </div>
          )}
          {hThumb && (
            <div className="agentz-browser-scrollbar horizontal">
              <div
                className="agentz-browser-scrollthumb"
                style={hThumb}
                onPointerDown={startScrollDrag("x")}
              />
            </div>
          )}
        </div>

        {selected && (
          <aside className="agentz-browser-inspector">
            <div className="agentz-browser-inspector-head">
              <span>{t("browser.inspector")}</span>
              <button type="button" className="agentz-browser-inspector-close" onClick={() => setSelected(null)}>
                ✕
              </button>
            </div>
            <div className="agentz-browser-inspector-body">
              <div className="agentz-browser-inspector-row">
                <span className="label">{t("browser.component")}</span>
                <code>
                  {selected.tag}
                  {selected.id ? `#${selected.id}` : ""}
                </code>
              </div>
              {selected.react_component ? (
                <div className="agentz-browser-inspector-row">
                  <span className="label">{t("browser.reactComponent")}</span>
                  <code>{selected.react_component}</code>
                </div>
              ) : null}
              {selected.dom_path ? (
                <div className="agentz-browser-inspector-row stack">
                  <span className="label">{t("browser.domPath")}</span>
                  <pre>{selected.dom_path}</pre>
                </div>
              ) : null}
              <div className="agentz-browser-inspector-row">
                <span className="label">{t("browser.selector")}</span>
                <code>{selected.selector}</code>
              </div>
              {selected.class_name ? (
                <div className="agentz-browser-inspector-row">
                  <span className="label">{t("browser.classes")}</span>
                  <code>{selected.class_name}</code>
                </div>
              ) : null}
              <div className="agentz-browser-inspector-row">
                <span className="label">{t("browser.dimensions")}</span>
                <code>
                  {Math.round(selected.rect_width ?? 0)} × {Math.round(selected.rect_height ?? 0)} px
                </code>
              </div>
              {selected.text ? (
                <div className="agentz-browser-inspector-row stack">
                  <span className="label">{t("browser.text")}</span>
                  <pre>{selected.text}</pre>
                </div>
              ) : null}
            </div>
            <button
              type="button"
              className="agentz-browser-inspector-send"
              onClick={() => onSendElementToChat(selected)}
              disabled={!chatEnabled}
              title={chatEnabled ? undefined : t("browser.needsProject")}
            >
              {t("browser.sendToChat")}
            </button>
          </aside>
        )}
      </div>
      {pageMenu.element}
    </div>
  );
}
