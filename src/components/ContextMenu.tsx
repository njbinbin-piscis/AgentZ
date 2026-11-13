import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { canPortalToBody, useContextMenuPosition } from "../utils/popupPosition";
import "./ContextMenu.css";

export interface ContextMenuItem {
  id: string;
  label: string;
  /** Render a divider instead of an actionable row. */
  separator?: boolean;
  disabled?: boolean;
  /** Destructive action — styled red, kept last by convention. */
  danger?: boolean;
  /** Small right-aligned hint (shortcut, count…). */
  hint?: string;
  onSelect?: () => void;
}

export interface ContextMenuState {
  x: number;
  y: number;
  items: ContextMenuItem[];
}

/**
 * Shared right-click menu: viewport-clamped positioning, keyboard navigation
 * (arrows / Home / End / Enter / Escape), outside-click + scroll dismissal,
 * danger styling. Portaled to <body> so panel overflow never clips it.
 */
export default function ContextMenu({
  state,
  onClose,
}: {
  state: ContextMenuState | null;
  onClose: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const style = useContextMenuPosition(state ? { x: state.x, y: state.y } : null, menuRef, {
    minWidth: 180,
    maxWidth: 320,
    maxHeight: 420,
  });
  const [active, setActive] = useState(-1);
  const actionable = useMemo(
    () =>
      (state?.items ?? [])
        .map((item, index) => ({ item, index }))
        .filter(({ item }) => !item.separator && !item.disabled && item.onSelect),
    [state],
  );

  useEffect(() => {
    setActive(-1);
  }, [state]);

  useEffect(() => {
    if (!state) return;
    // Focus the menu so arrow keys work without stealing a permanent tab stop.
    menuRef.current?.focus({ preventScroll: true });
    const close = (e: Event) => {
      if (e.target instanceof Node && menuRef.current?.contains(e.target)) return;
      // Do NOT preventDefault here: a right-click elsewhere must keep flowing
      // to the target's own handler (component menu or the app-wide guard),
      // which decides what opens next. This listener only closes.
      onClose();
    };
    const onBlurWindow = () => onClose();
    document.addEventListener("mousedown", close, true);
    document.addEventListener("contextmenu", close, true);
    window.addEventListener("scroll", close, true);
    window.addEventListener("resize", close);
    window.addEventListener("blur", onBlurWindow);
    return () => {
      document.removeEventListener("mousedown", close, true);
      document.removeEventListener("contextmenu", close, true);
      window.removeEventListener("scroll", close, true);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", onBlurWindow);
    };
  }, [state, onClose]);

  const move = useCallback(
    (delta: number) => {
      if (actionable.length === 0) return;
      setActive((cur) => {
        const pos = actionable.findIndex(({ index }) => index === cur);
        const next = pos < 0 ? (delta > 0 ? 0 : actionable.length - 1) : (pos + delta + actionable.length) % actionable.length;
        return actionable[next].index;
      });
    },
    [actionable],
  );

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        onClose();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        move(1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        move(-1);
      } else if (e.key === "Home" || e.key === "End") {
        e.preventDefault();
        if (actionable.length > 0) setActive(actionable[e.key === "Home" ? 0 : actionable.length - 1].index);
      } else if (e.key === "Enter" || e.key === " ") {
        const entry = actionable.find(({ index }) => index === active);
        if (entry) {
          e.preventDefault();
          onClose();
          entry.item.onSelect?.();
        }
      }
    },
    [actionable, active, move, onClose],
  );

  if (!state) return null;
  const node = (
    <div
      ref={menuRef}
      className="agentz-context-menu"
      style={style}
      role="menu"
      tabIndex={-1}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      {state.items.map((item) =>
        item.separator ? (
          <div key={item.id} className="agentz-context-menu-sep" role="separator" />
        ) : (
          <button
            key={item.id}
            type="button"
            role="menuitem"
            className={`agentz-context-menu-item${item.danger ? " is-danger" : ""}${active === state.items.indexOf(item) ? " is-active" : ""}`}
            disabled={item.disabled}
            tabIndex={-1}
            onMouseEnter={() => setActive(state.items.indexOf(item))}
            onClick={() => {
              onClose();
              item.onSelect?.();
            }}
          >
            <span className="agentz-context-menu-label">{item.label}</span>
            {item.hint && <span className="agentz-context-menu-hint">{item.hint}</span>}
          </button>
        ),
      )}
    </div>
  );
  return canPortalToBody() ? createPortal(node, document.body) : node;
}

/** Open-state helper: `const menu = useContextMenu(); <div onContextMenu={menu.openWith(items)} />`.
 *  `openWith` also accepts a factory `(e) => items` so callers can build items
 *  lazily at right-click time (e.g. reading the live text selection) instead of
 *  during render — keeps render pure and DOM-free. */
export function useContextMenu() {
  const [state, setState] = useState<ContextMenuState | null>(null);
  const close = useCallback(() => setState(null), []);
  const openWith = useCallback(
    (items: ContextMenuItem[] | ((e: React.MouseEvent) => ContextMenuItem[])) =>
      (e: React.MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        const resolved = typeof items === "function" ? items(e) : items;
        setState({ x: e.clientX, y: e.clientY, items: resolved });
      },
    [],
  );
  const element = <ContextMenu state={state} onClose={close} />;
  return { state, openWith, close, element };
}
