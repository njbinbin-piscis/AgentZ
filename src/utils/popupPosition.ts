import {
  useLayoutEffect,
  useState,
  type CSSProperties,
  type RefObject,
} from "react";

export interface FixedPopupOptions {
  gap?: number;
  margin?: number;
  minWidth?: number;
  maxWidth?: number;
  maxHeight?: number;
  align?: "left" | "right";
  matchTriggerWidth?: boolean;
}

export interface ContextMenuPositionOptions {
  margin?: number;
  minWidth?: number;
  maxWidth?: number;
  maxHeight?: number;
}

export function canPortalToBody(): boolean {
  return (
    typeof document !== "undefined" &&
    Boolean(document.body) &&
    typeof (document.body as unknown as { nodeType?: number }).nodeType === "number"
  );
}

export function getActiveFontScale(): number {
  if (typeof document === "undefined" || !document.documentElement) {
    return 1;
  }
  const inlineVal = document.documentElement.style?.getPropertyValue?.("--ui-font-scale")?.trim();
  const parsedInline = inlineVal ? Number(inlineVal) : NaN;
  if (Number.isFinite(parsedInline) && parsedInline >= 0.5 && parsedInline <= 3) {
    return parsedInline;
  }
  if (typeof window !== "undefined" && typeof window.getComputedStyle === "function") {
    try {
      const computedVal = window
        .getComputedStyle(document.documentElement)
        .getPropertyValue("--ui-font-scale")
        ?.trim();
      const parsedComputed = computedVal ? Number(computedVal) : NaN;
      if (Number.isFinite(parsedComputed) && parsedComputed >= 0.5 && parsedComputed <= 3) {
        return parsedComputed;
      }
    } catch {
      // ignore in non-DOM test environments
    }
  }
  return 1;
}

export function getViewportSize(): { vw: number; vh: number } {
  const vw =
    (typeof window !== "undefined" && window.innerWidth) ||
    (typeof document !== "undefined" && document.documentElement?.clientWidth) ||
    800;
  const vh =
    (typeof window !== "undefined" && window.innerHeight) ||
    (typeof document !== "undefined" && document.documentElement?.clientHeight) ||
    600;
  return { vw, vh };
}

export function getTriggerViewportRect(
  trigger: HTMLElement,
  vw: number,
  vh: number,
): {
  top: number;
  bottom: number;
  left: number;
  right: number;
  width: number;
  height: number;
} {
  const rawTr = trigger.getBoundingClientRect();

  const isCounterZoomed =
    typeof trigger.closest === "function" &&
    Boolean(
      trigger.closest(
        ".agentz-editor-wrap, .ide-terminal-session, .agentz-browser-view, .agentz-browser-canvas",
      ),
    );
  const zoomedRoot =
    !isCounterZoomed && typeof trigger.closest === "function"
      ? (trigger.closest(".agentz-app, .agentz-settings-overlay") as HTMLElement | null)
      : null;
  const appRect =
    zoomedRoot && typeof zoomedRoot.getBoundingClientRect === "function"
      ? zoomedRoot.getBoundingClientRect()
      : null;

  const rawScaleX = appRect && appRect.width > 0 ? vw / appRect.width : 1;
  const rawScaleY = appRect && appRect.height > 0 ? vh / appRect.height : 1;
  const useScale =
    appRect !== null &&
    Math.abs(rawScaleX - rawScaleY) < 0.08 &&
    rawScaleX >= 0.5 &&
    rawScaleX <= 3 &&
    rawTr.right <= appRect.width + 2 &&
    rawTr.bottom <= appRect.height + 2;
  const scaleX = useScale ? rawScaleX : 1;
  const scaleY = useScale ? rawScaleY : 1;

  return {
    top: rawTr.top * scaleY,
    bottom: rawTr.bottom * scaleY,
    left: rawTr.left * scaleX,
    right: rawTr.right * scaleX,
    width: rawTr.width * scaleX,
    height: rawTr.height * scaleY,
  };
}

/**
 * Compute viewport-clamped `position: fixed` coordinates for a trigger-anchored popup.
 *
 * Rules:
 * - Always checks whether the right edge (`left + width`) exceeds the window; if so,
 *   calculates horizontal position anchored from `right` (`left: "auto"`).
 * - Always checks whether the bottom edge (`top + height`) exceeds the window; if so,
 *   calculates vertical position anchored from `bottom` (`top: "auto"`).
 * - Accounts for `--ui-font-scale` zoom scaling on `.agentz-app` and scales popup bounds accordingly.
 */
export function computeFixedPopupPosition(
  trigger: HTMLElement | null,
  popup: HTMLElement | null,
  placement: "up" | "down",
  variant: "pill" | "text" | "field" | "inline" = "text",
  options?: FixedPopupOptions,
): CSSProperties {
  if (!trigger || typeof trigger.getBoundingClientRect !== "function") {
    return {
      position: "fixed",
      zIndex: 100100,
    };
  }

  const { vw, vh } = getViewportSize();
  const fontScale = getActiveFontScale();
  const tr = getTriggerViewportRect(trigger, vw, vh);

  const gap = options?.gap ?? 6;
  const margin = options?.margin ?? 8;
  const baseMaxH = options?.maxHeight ?? 280;
  const defaultMaxH = Math.min(
    Math.round(baseMaxH * fontScale),
    Math.max(72, vh - margin * 2),
  );

  const isFieldLike = variant === "field" || variant === "inline";
  const baseMinW =
    options?.minWidth !== undefined
      ? Math.round(options.minWidth * fontScale)
      : options?.matchTriggerWidth
        ? Math.max(120, Math.round(tr.width || 200))
        : isFieldLike
          ? Math.max(120, Math.round(tr.width || 200 * fontScale))
          : Math.round(200 * fontScale);
  const baseMaxW =
    options?.maxWidth !== undefined
      ? Math.round(options.maxWidth * fontScale)
      : options?.matchTriggerWidth
        ? Math.max(baseMinW, Math.round(tr.width || 320))
        : isFieldLike
          ? Math.max(baseMinW, Math.round(520 * fontScale))
          : Math.round(320 * fontScale);

  const minWidth = Math.min(baseMinW, Math.max(100, vw - margin * 2));
  const maxWidth = Math.max(minWidth, Math.min(baseMaxW, vw - margin * 2));

  const rect =
    popup && typeof popup.getBoundingClientRect === "function"
      ? popup.getBoundingClientRect()
      : null;
  const measuredW = popup
    ? Math.max(popup.offsetWidth || 0, popup.scrollWidth || 0, rect?.width || 0)
    : 0;
  const measuredH = popup
    ? Math.max(popup.scrollHeight || 0, popup.offsetHeight || 0, rect?.height || 0)
    : 0;

  const fallbackW = Math.min(maxWidth, Math.max(minWidth, Math.round(240 * fontScale)));
  const fallbackH = Math.min(defaultMaxH, Math.round(220 * fontScale));
  const pw = Math.min(maxWidth, Math.max(minWidth, measuredW || fallbackW));
  const ph = Math.min(defaultMaxH, Math.max(48, measuredH || fallbackH));

  // ─── 1. Horizontal: check if right side exceeds window ───────────────
  const preferRight = options?.align === "right";
  const overflowsRight = preferRight || tr.left + pw > vw - margin;

  let left: number | "auto" = "auto";
  let right: number | "auto" = "auto";
  let resolvedMaxW = maxWidth;

  if (overflowsRight) {
    const rightFromTrigger = Math.max(margin, vw - Math.min(tr.right, vw - margin));
    right = vw - rightFromTrigger - pw >= margin ? rightFromTrigger : margin;
    left = "auto";
    resolvedMaxW = Math.max(minWidth, Math.min(maxWidth, vw - right - margin));
  } else {
    left = Math.max(margin, tr.left);
    right = "auto";
    resolvedMaxW = Math.max(minWidth, Math.min(maxWidth, vw - left - margin));
  }

  const resolvedMinW = Math.min(minWidth, resolvedMaxW);

  // ─── 2. Vertical: check if bottom side exceeds window ────────────────
  const spaceAbove = Math.max(0, tr.top - gap - margin);
  const spaceBelow = Math.max(0, vh - tr.bottom - gap - margin);
  const desiredTop = tr.bottom + gap;
  const overflowsBottom = desiredTop + ph > vh - margin;

  if (placement === "up") {
    // Open upward anchored from bottom; only flip down if trigger is pinned to the very top of the window.
    if (spaceAbove >= ph || spaceAbove >= spaceBelow || spaceAbove >= 72) {
      const bottom = Math.max(margin, vh - tr.top + gap);
      const maxHeight = Math.min(
        defaultMaxH,
        Math.max(72, vh - bottom - margin),
      );
      return {
        position: "fixed",
        top: "auto",
        bottom,
        left,
        right,
        minWidth: resolvedMinW,
        maxWidth: resolvedMaxW,
        maxHeight,
        zIndex: 100100,
      };
    }

    if (overflowsBottom) {
      const bottom = margin;
      const availFromTop = vh - Math.max(margin, desiredTop) - margin;
      const maxHeight =
        availFromTop >= 96
          ? Math.min(defaultMaxH, availFromTop)
          : Math.min(defaultMaxH, Math.max(72, vh - margin * 2));
      return {
        position: "fixed",
        top: "auto",
        bottom,
        left,
        right,
        minWidth: resolvedMinW,
        maxWidth: resolvedMaxW,
        maxHeight,
        zIndex: 100100,
      };
    }

    const top = Math.max(margin, desiredTop);
    const maxHeight = Math.min(defaultMaxH, Math.max(72, vh - top - margin));
    return {
      position: "fixed",
      top,
      bottom: "auto",
      left,
      right,
      minWidth: resolvedMinW,
      maxWidth: resolvedMaxW,
      maxHeight,
      zIndex: 100100,
    };
  }

  // placement === "down": check if bottom side exceeds the window
  if (overflowsBottom) {
    // When bottom exceeds window, always calculate vertical position using `bottom` as the reference.
    if (spaceAbove >= ph || (spaceAbove > spaceBelow && spaceAbove >= 96)) {
      const bottom = Math.max(margin, vh - tr.top + gap);
      const maxHeight = Math.min(
        defaultMaxH,
        Math.max(72, vh - bottom - margin),
      );
      return {
        position: "fixed",
        top: "auto",
        bottom,
        left,
        right,
        minWidth: resolvedMinW,
        maxWidth: resolvedMaxW,
        maxHeight,
        zIndex: 100100,
      };
    }

    // Cannot flip above trigger (e.g. titlebar popover near top of window): anchor to window bottom.
    const bottom = margin;
    const availFromTop = vh - Math.max(margin, desiredTop) - margin;
    const maxHeight =
      availFromTop >= 96
        ? Math.min(defaultMaxH, availFromTop)
        : Math.min(defaultMaxH, Math.max(72, vh - margin * 2));
    return {
      position: "fixed",
      top: "auto",
      bottom,
      left,
      right,
      minWidth: resolvedMinW,
      maxWidth: resolvedMaxW,
      maxHeight,
      zIndex: 100100,
    };
  }

  const top = Math.max(margin, desiredTop);
  const maxHeight = Math.min(defaultMaxH, Math.max(72, vh - top - margin));
  return {
    position: "fixed",
    top,
    bottom: "auto",
    left,
    right,
    minWidth: resolvedMinW,
    maxWidth: resolvedMaxW,
    maxHeight,
    zIndex: 100100,
  };
}

/**
 * Compute viewport-clamped `position: fixed` coordinates for a right-click context menu at `(x, y)`.
 *
 * Rules:
 * - Checks if `x + width` exceeds the right window edge; if so, anchors via `right` (`left: "auto"`).
 * - Checks if `y + height` exceeds the bottom window edge; if so, anchors via `bottom` (`top: "auto"`).
 * - Scales bounds with `--ui-font-scale`.
 */
export function computeContextMenuPosition(
  point: { x: number; y: number } | null,
  popup: HTMLElement | null,
  options?: ContextMenuPositionOptions,
): CSSProperties {
  if (!point) {
    return {
      position: "fixed",
      zIndex: 100100,
    };
  }

  const { vw, vh } = getViewportSize();
  const fontScale = getActiveFontScale();
  const margin = options?.margin ?? 8;

  const minWidth = Math.min(
    Math.round((options?.minWidth ?? 160) * fontScale),
    Math.max(100, vw - margin * 2),
  );
  const maxWidth = Math.max(
    minWidth,
    Math.min(Math.round((options?.maxWidth ?? 320) * fontScale), vw - margin * 2),
  );
  const defaultMaxH = Math.min(
    Math.round((options?.maxHeight ?? 420) * fontScale),
    Math.max(72, vh - margin * 2),
  );

  const rect =
    popup && typeof popup.getBoundingClientRect === "function"
      ? popup.getBoundingClientRect()
      : null;
  const measuredW = popup
    ? Math.max(popup.offsetWidth || 0, popup.scrollWidth || 0, rect?.width || 0)
    : 0;
  const measuredH = popup
    ? Math.max(popup.scrollHeight || 0, popup.offsetHeight || 0, rect?.height || 0)
    : 0;

  const fallbackW = Math.min(maxWidth, Math.max(minWidth, Math.round(220 * fontScale)));
  const fallbackH = Math.min(defaultMaxH, Math.round(260 * fontScale));
  const pw = Math.min(maxWidth, Math.max(minWidth, measuredW || fallbackW));
  const ph = Math.min(defaultMaxH, Math.max(40, measuredH || fallbackH));

  // 1. Horizontal: check if right side exceeds window
  const overflowsRight = point.x + pw > vw - margin;
  let left: number | "auto" = "auto";
  let right: number | "auto" = "auto";
  let resolvedMaxW = maxWidth;

  if (overflowsRight) {
    const rightFromPoint = Math.max(margin, vw - Math.min(point.x, vw - margin));
    right = vw - rightFromPoint - pw >= margin ? rightFromPoint : margin;
    left = "auto";
    resolvedMaxW = Math.max(minWidth, Math.min(maxWidth, vw - right - margin));
  } else {
    left = Math.max(margin, point.x);
    right = "auto";
    resolvedMaxW = Math.max(minWidth, Math.min(maxWidth, vw - left - margin));
  }

  // 2. Vertical: check if bottom side exceeds window -> anchor from bottom
  const overflowsBottom = point.y + ph > vh - margin;
  if (overflowsBottom) {
    const bottomFromPoint = Math.max(margin, vh - Math.min(point.y, vh - margin));
    const bottom = vh - bottomFromPoint - ph >= margin ? bottomFromPoint : margin;
    const maxHeight = Math.min(defaultMaxH, Math.max(72, vh - bottom - margin));
    return {
      position: "fixed",
      top: "auto",
      bottom,
      left,
      right,
      minWidth: Math.min(minWidth, resolvedMaxW),
      maxWidth: resolvedMaxW,
      maxHeight,
      zIndex: 100100,
    };
  }

  const top = Math.max(margin, point.y);
  const maxHeight = Math.min(defaultMaxH, Math.max(72, vh - top - margin));
  return {
    position: "fixed",
    top,
    bottom: "auto",
    left,
    right,
    minWidth: Math.min(minWidth, resolvedMaxW),
    maxWidth: resolvedMaxW,
    maxHeight,
    zIndex: 100100,
  };
}

export function useFixedPopupPosition(
  open: boolean,
  triggerRef: RefObject<HTMLElement | null>,
  popupRef: RefObject<HTMLElement | null>,
  placement: "up" | "down",
  variant: "pill" | "text" | "field" | "inline" = "text",
  options?: FixedPopupOptions,
): CSSProperties {
  const [style, setStyle] = useState<CSSProperties>({});

  const gap = options?.gap;
  const margin = options?.margin;
  const minWidth = options?.minWidth;
  const maxWidth = options?.maxWidth;
  const maxHeight = options?.maxHeight;
  const align = options?.align;
  const matchTriggerWidth = options?.matchTriggerWidth;

  useLayoutEffect(() => {
    if (!open) {
      setStyle({});
      return;
    }
    const trigger = triggerRef.current;
    const popup = popupRef.current;
    if (!trigger) return;

    const opts: FixedPopupOptions = {
      gap,
      margin,
      minWidth,
      maxWidth,
      maxHeight,
      align,
      matchTriggerWidth,
    };

    const reposition = () => {
      setStyle(
        computeFixedPopupPosition(
          triggerRef.current,
          popupRef.current,
          placement,
          variant,
          opts,
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
      window.addEventListener("agentz-font-scale", reposition);
    }
    return () => {
      if (
        typeof window !== "undefined" &&
        typeof window.cancelAnimationFrame === "function" &&
        rafId
      ) {
        window.cancelAnimationFrame(rafId);
      }
      ro?.disconnect();
      if (typeof window !== "undefined") {
        window.removeEventListener("resize", reposition);
        window.removeEventListener("scroll", reposition, true);
        window.removeEventListener("agentz-font-scale", reposition);
      }
    };
  }, [
    open,
    placement,
    variant,
    gap,
    margin,
    minWidth,
    maxWidth,
    maxHeight,
    align,
    matchTriggerWidth,
    triggerRef,
    popupRef,
  ]);

  if (open && Object.keys(style).length === 0 && triggerRef.current) {
    return computeFixedPopupPosition(
      triggerRef.current,
      popupRef.current,
      placement,
      variant,
      options,
    );
  }

  return style;
}

export function useContextMenuPosition(
  point: { x: number; y: number } | null,
  popupRef: RefObject<HTMLElement | null>,
  options?: ContextMenuPositionOptions,
): CSSProperties {
  const [style, setStyle] = useState<CSSProperties>({});
  const open = point !== null;
  const x = point?.x ?? 0;
  const y = point?.y ?? 0;
  const margin = options?.margin;
  const minWidth = options?.minWidth;
  const maxWidth = options?.maxWidth;
  const maxHeight = options?.maxHeight;

  useLayoutEffect(() => {
    if (!open) {
      setStyle({});
      return;
    }
    const popup = popupRef.current;
    const opts: ContextMenuPositionOptions = {
      margin,
      minWidth,
      maxWidth,
      maxHeight,
    };

    const reposition = () => {
      setStyle(computeContextMenuPosition({ x, y }, popupRef.current, opts));
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
      window.addEventListener("agentz-font-scale", reposition);
    }
    return () => {
      if (
        typeof window !== "undefined" &&
        typeof window.cancelAnimationFrame === "function" &&
        rafId
      ) {
        window.cancelAnimationFrame(rafId);
      }
      ro?.disconnect();
      if (typeof window !== "undefined") {
        window.removeEventListener("resize", reposition);
        window.removeEventListener("agentz-font-scale", reposition);
      }
    };
  }, [open, x, y, margin, minWidth, maxWidth, maxHeight, popupRef]);

  if (open && Object.keys(style).length === 0) {
    return computeContextMenuPosition({ x, y }, popupRef.current, options);
  }

  return style;
}
