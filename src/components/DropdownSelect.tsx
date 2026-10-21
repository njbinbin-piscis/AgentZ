import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";
import "./DropdownSelect.css";

export interface DropdownOption {
  id: string;
  label: string;
  hint?: string;
  title?: string;
}

function ChevronDown() {
  return (
    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
      <path d="M6 9l6 6 6-6" />
    </svg>
  );
}

export function computeFixedPopupPosition(
  trigger: HTMLElement | null,
  popup: HTMLElement | null,
  placement: "up" | "down",
  variant: "pill" | "text" | "field" | "inline" = "text",
): CSSProperties {
  if (!trigger || typeof trigger.getBoundingClientRect !== "function") {
    return {
      position: "fixed",
      zIndex: 100100,
    };
  }

  const vw =
    (typeof window !== "undefined" && window.innerWidth) ||
    (typeof document !== "undefined" && document.documentElement?.clientWidth) ||
    800;
  const vh =
    (typeof window !== "undefined" && window.innerHeight) ||
    (typeof document !== "undefined" && document.documentElement?.clientHeight) ||
    600;
  const rawTr = trigger.getBoundingClientRect();

  // Normalize coordinates if `.agentz-app` applies CSS `zoom` and the engine reports local zoomed rects.
  const appEl =
    typeof trigger.closest === "function"
      ? (trigger.closest(".agentz-app") as HTMLElement | null)
      : null;
  const appRect =
    appEl && typeof appEl.getBoundingClientRect === "function"
      ? appEl.getBoundingClientRect()
      : null;
  const rawScaleX = appRect && appRect.width > 0 ? vw / appRect.width : 1;
  const rawScaleY = appRect && appRect.height > 0 ? vh / appRect.height : 1;
  const useScale =
    Math.abs(rawScaleX - rawScaleY) < 0.08 && rawScaleX >= 0.5 && rawScaleX <= 3;
  const scaleX = useScale ? rawScaleX : 1;
  const scaleY = useScale ? rawScaleY : 1;

  const tr = {
    top: rawTr.top * scaleY,
    bottom: rawTr.bottom * scaleY,
    left: rawTr.left * scaleX,
    right: rawTr.right * scaleX,
    width: rawTr.width * scaleX,
    height: rawTr.height * scaleY,
  };

  const gap = 6;
  const margin = 8;
  const defaultMaxH = 280;

  const spaceAbove = Math.max(0, tr.top - gap - margin);
  const spaceBelow = Math.max(0, vh - tr.bottom - gap - margin);
  const naturalH = Math.min(
    defaultMaxH,
    popup ? popup.scrollHeight || popup.offsetHeight || 220 : 220,
  );

  // Only flip direction when the preferred side cannot fit the menu AND the opposite side has strictly more space.
  const openUp =
    placement === "up"
      ? spaceAbove >= naturalH || spaceAbove >= spaceBelow
      : spaceBelow < naturalH && spaceAbove > spaceBelow;

  const availH = openUp ? spaceAbove : spaceBelow;
  const maxHeight =
    availH >= 48 ? Math.min(defaultMaxH, availH) : Math.min(defaultMaxH, Math.max(72, vh - margin * 2));

  const isFieldLike = variant === "field" || variant === "inline";
  const minWidth = isFieldLike
    ? Math.max(120, Math.min(tr.width || 200, vw - margin * 2))
    : Math.min(200, Math.max(120, vw - margin * 2));
  const maxWidth = isFieldLike
    ? Math.max(minWidth, Math.min(520, vw - margin * 2))
    : Math.max(minWidth, Math.min(320, vw - margin * 2));

  const measuredW = popup
    ? popup.offsetWidth ||
      (typeof popup.getBoundingClientRect === "function"
        ? popup.getBoundingClientRect().width
        : 0) ||
      0
    : 0;
  const pw = Math.min(maxWidth, Math.max(minWidth, measuredW || minWidth));

  let left = tr.left;
  if (left + pw > vw - margin) {
    left = tr.right - pw;
  }
  if (left + pw > vw - margin) {
    left = vw - pw - margin;
  }
  left = Math.max(margin, left);

  if (openUp) {
    const bottom = Math.max(margin, vh - tr.top + gap);
    return {
      position: "fixed",
      top: "auto",
      bottom,
      left,
      right: "auto",
      minWidth,
      maxWidth,
      maxHeight,
      zIndex: 100100,
    };
  }

  const top = Math.max(margin, Math.min(tr.bottom + gap, vh - margin - 48));
  return {
    position: "fixed",
    top,
    bottom: "auto",
    left,
    right: "auto",
    minWidth,
    maxWidth,
    maxHeight,
    zIndex: 100100,
  };
}

function canPortalToBody(): boolean {
  return (
    typeof document !== "undefined" &&
    Boolean(document.body) &&
    typeof (document.body as unknown as { nodeType?: number }).nodeType === "number"
  );
}

/** Escape overflow:hidden/clip ancestors and stacking contexts by portaling to document.body and clamping to viewport. */
function useFixedPopupPosition(
  open: boolean,
  triggerRef: RefObject<HTMLElement | null>,
  popupRef: RefObject<HTMLElement | null>,
  placement: "up" | "down",
  variant: "pill" | "text" | "field" | "inline" = "text",
) {
  const [style, setStyle] = useState<CSSProperties>({});

  useLayoutEffect(() => {
    if (!open) {
      setStyle({});
      return;
    }
    const trigger = triggerRef.current;
    const popup = popupRef.current;
    if (!trigger) return;

    const reposition = () => {
      setStyle(
        computeFixedPopupPosition(
          triggerRef.current,
          popupRef.current,
          placement,
          variant,
        ),
      );
    };

    reposition();
    const rafId =
      typeof window !== "undefined" && typeof window.requestAnimationFrame === "function"
        ? window.requestAnimationFrame(reposition)
        : 0;
    let ro: ResizeObserver | null = null;
    if (popup && typeof ResizeObserver !== "undefined") {
      ro = new ResizeObserver(() => reposition());
      ro.observe(popup);
    }
    if (typeof window !== "undefined") {
      window.addEventListener("resize", reposition);
      window.addEventListener("scroll", reposition, true);
    }
    return () => {
      if (typeof window !== "undefined" && typeof window.cancelAnimationFrame === "function" && rafId) {
        window.cancelAnimationFrame(rafId);
      }
      ro?.disconnect();
      if (typeof window !== "undefined") {
        window.removeEventListener("resize", reposition);
        window.removeEventListener("scroll", reposition, true);
      }
    };
  }, [open, placement, variant, triggerRef, popupRef]);

  if (open && Object.keys(style).length === 0 && triggerRef.current) {
    return computeFixedPopupPosition(
      triggerRef.current,
      popupRef.current,
      placement,
      variant,
    );
  }

  return style;
}

export interface DropdownSelectProps {
  value: string;
  options: DropdownOption[];
  onChange: (id: string) => void;
  variant?: "pill" | "text" | "field" | "inline";
  placement?: "up" | "down";
  disabled?: boolean;
  id?: string;
  className?: string;
  placeholder?: string;
  title?: string;
  /** Optional icon before the label (pill/text variants). */
  icon?: ReactNode;
  /** Highlight trigger when a non-empty value is selected. */
  accentWhenSet?: boolean;
}

/** Theme-aware single-select menu (replaces native `<select>` popup styling). */
export default function DropdownSelect({
  value,
  options,
  onChange,
  variant = "field",
  placement,
  disabled,
  id,
  className,
  placeholder,
  title,
  icon,
  accentWhenSet,
}: DropdownSelectProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popupRef = useRef<HTMLDivElement>(null);
  const resolvedPlacement = placement ?? (variant === "pill" || variant === "text" ? "up" : "down");
  const popupStyle = useFixedPopupPosition(
    open,
    triggerRef,
    popupRef,
    resolvedPlacement,
    variant,
  );
  const selected = options.find((o) => o.id === value);
  const displayLabel = selected?.label ?? placeholder ?? options[0]?.label ?? "—";

  useEffect(() => {
    if (!open || typeof document === "undefined") return;
    const onDoc = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (
        target &&
        !rootRef.current?.contains(target) &&
        !popupRef.current?.contains(target)
      ) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const popupClass = resolvedPlacement === "up" ? "popup-up" : "popup-down";

  const popupNode = open ? (
    <div
      ref={popupRef}
      className={`agentz-dropdown-popup ${variant} is-fixed`}
      style={popupStyle}
      role="listbox"
      title=""
    >
      {options.length === 0 ? (
        <div className="agentz-dropdown-empty">{placeholder ?? "—"}</div>
      ) : (
        options.map((opt) => (
          <button
            key={opt.id || "__empty"}
            type="button"
            role="option"
            title={opt.title ?? opt.hint}
            aria-selected={opt.id === value}
            className={opt.id === value ? "active" : ""}
            onClick={() => {
              onChange(opt.id);
              setOpen(false);
            }}
          >
            <span className="agentz-dropdown-option-label">{opt.label}</span>
            {opt.hint && <span className="agentz-dropdown-option-hint">{opt.hint}</span>}
          </button>
        ))
      )}
    </div>
  ) : null;

  return (
    <div
      className={`agentz-dropdown ${variant} ${popupClass}${className ? ` ${className}` : ""}`}
      ref={rootRef}
    >
      <button
        ref={triggerRef}
        type="button"
        id={id}
        className={`agentz-dropdown-trigger${accentWhenSet && value ? " has-value-accent" : ""}`}
        disabled={disabled}
        title={title ?? selected?.title ?? selected?.hint}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        {icon && <span className="agentz-dropdown-icon">{icon}</span>}
        <span className="agentz-dropdown-label">{displayLabel}</span>
        {(variant === "pill" || variant === "text") && (
          <span className="agentz-dropdown-chevron">
            <ChevronDown />
          </span>
        )}
      </button>
      {popupNode && canPortalToBody()
        ? createPortal(popupNode, document.body)
        : popupNode}
    </div>
  );
}

export interface DropdownMultiSelectProps {
  options: DropdownOption[];
  selected: string[];
  onChange: (ids: string[]) => void;
  label: string;
  disabled?: boolean;
  emptyHint?: string;
  onEmptyHintClick?: () => void;
  /** When "toast", hints are also rendered inline (clamped) without spawning an obscuring toast. */
  hintPresentation?: "inline" | "toast";
  onHintToast?: (message: string) => void;
}

/** Multi-select skills-style menu (CodeZ / WorkZ composer). */
export function DropdownMultiSelect({
  options,
  selected,
  onChange,
  label,
  disabled,
  emptyHint,
  onEmptyHintClick,
  icon,
}: DropdownMultiSelectProps & { icon?: ReactNode }) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const popupRef = useRef<HTMLDivElement>(null);
  const popupStyle = useFixedPopupPosition(open, triggerRef, popupRef, "up", "text");

  useEffect(() => {
    if (!open || typeof document === "undefined") return;
    const onDoc = (e: MouseEvent) => {
      const target = e.target as Node | null;
      if (
        target &&
        !rootRef.current?.contains(target) &&
        !popupRef.current?.contains(target)
      ) {
        setOpen(false);
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const toggle = (id: string) => {
    onChange(selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id]);
  };

  const count = selected.length;

  const popupNode = open ? (
    <div
      ref={popupRef}
      className="agentz-dropdown-popup text is-fixed"
      style={popupStyle}
      role="listbox"
      aria-multiselectable="true"
      title=""
    >
      {options.length === 0 ? (
        onEmptyHintClick ? (
          <button
            type="button"
            className="agentz-dropdown-empty agentz-dropdown-empty-action"
            onClick={() => {
              setOpen(false);
              onEmptyHintClick();
            }}
          >
            {emptyHint ?? "—"}
          </button>
        ) : (
          <div className="agentz-dropdown-empty">{emptyHint ?? "—"}</div>
        )
      ) : (
        options.map((opt) => {
          const active = selected.includes(opt.id);
          return (
            <button
              key={opt.id}
              type="button"
              role="option"
              title={opt.title ?? opt.hint}
              aria-selected={active}
              className={active ? "active" : ""}
              onClick={() => toggle(opt.id)}
            >
              <span className="agentz-dropdown-option-head">
                <span className="agentz-dropdown-check" aria-hidden>
                  {active ? "✓" : ""}
                </span>
                <span className="agentz-dropdown-option-label">{opt.label}</span>
              </span>
              {opt.hint && (
                <span className="agentz-dropdown-option-hint">{opt.hint}</span>
              )}
            </button>
          );
        })
      )}
    </div>
  ) : null;

  return (
    <div className="agentz-dropdown text popup-up" ref={rootRef}>
      <button
        ref={triggerRef}
        type="button"
        className={`agentz-dropdown-trigger${count > 0 ? " has-value-accent" : ""}`}
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        title={label}
        onClick={() => setOpen((v) => !v)}
      >
        {icon && <span className="agentz-dropdown-icon">{icon}</span>}
        <span className="agentz-dropdown-label">{count > 0 ? `${label} · ${count}` : label}</span>
        <span className="agentz-dropdown-chevron">
          <ChevronDown />
        </span>
      </button>
      {popupNode && canPortalToBody()
        ? createPortal(popupNode, document.body)
        : popupNode}
    </div>
  );
}
