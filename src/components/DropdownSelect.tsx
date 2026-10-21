import {
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import {
  canPortalToBody,
  computeContextMenuPosition,
  computeFixedPopupPosition,
  useContextMenuPosition,
  useFixedPopupPosition,
} from "../utils/popupPosition";
import "./DropdownSelect.css";

export {
  canPortalToBody,
  computeContextMenuPosition,
  computeFixedPopupPosition,
  useContextMenuPosition,
  useFixedPopupPosition,
};

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
