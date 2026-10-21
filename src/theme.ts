/** App shell appearance (light / dark). Built-in Monaco themes follow unless a .vsix theme is pinned. */

export type AppearanceTheme = "dark" | "light";

export const UI_FONT_SCALES = [0.875, 1, 1.125, 1.25, 1.375, 1.5] as const;
export type UiFontScale = (typeof UI_FONT_SCALES)[number];

export const DEFAULT_EDITOR_FONT_SIZE = 14;
export const MIN_EDITOR_FONT_SIZE = 10;
export const MAX_EDITOR_FONT_SIZE = 32;

const STORAGE_KEY = "agentz-appearance";
const FONT_SCALE_KEY = "agentz-ui-font-scale";
const EDITOR_FONT_SIZE_KEY = "agentz-editor-font-size";

export function getAppearanceTheme(): AppearanceTheme {
  if (typeof localStorage === "undefined") return "dark";
  const saved = localStorage.getItem(STORAGE_KEY);
  return saved === "light" ? "light" : "dark";
}

export function applyAppearanceTheme(theme: AppearanceTheme): void {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = theme;
  }
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(STORAGE_KEY, theme);
  }
}

export function toggleAppearanceTheme(): AppearanceTheme {
  const next: AppearanceTheme = getAppearanceTheme() === "dark" ? "light" : "dark";
  applyAppearanceTheme(next);
  return next;
}

export function initAppearanceTheme(): AppearanceTheme {
  const theme = getAppearanceTheme();
  applyAppearanceTheme(theme);
  return theme;
}

export function getUiFontScale(): UiFontScale {
  if (typeof localStorage === "undefined") return 1;
  const saved = Number(localStorage.getItem(FONT_SCALE_KEY));
  return (UI_FONT_SCALES as readonly number[]).includes(saved)
    ? (saved as UiFontScale)
    : 1;
}

export function getEditorBaseFontSize(): number {
  if (typeof localStorage === "undefined") return DEFAULT_EDITOR_FONT_SIZE;
  const raw = localStorage.getItem(EDITOR_FONT_SIZE_KEY);
  if (!raw) return DEFAULT_EDITOR_FONT_SIZE;
  const parsed = Number(raw);
  if (!Number.isFinite(parsed)) return DEFAULT_EDITOR_FONT_SIZE;
  return Math.min(MAX_EDITOR_FONT_SIZE, Math.max(MIN_EDITOR_FONT_SIZE, Math.round(parsed)));
}

export function getEffectiveEditorFontSize(
  scale: number = getUiFontScale(),
  baseSize: number = getEditorBaseFontSize(),
): number {
  return Math.min(36, Math.max(MIN_EDITOR_FONT_SIZE, Math.round(baseSize * scale)));
}

export function getEffectiveTerminalFontSize(
  scale: number = getUiFontScale(),
  baseSize: number = getEditorBaseFontSize(),
): number {
  return Math.min(32, Math.max(10, Math.round(Math.max(12, baseSize - 1) * scale)));
}

function syncFontCssVariables(scale: UiFontScale, baseSize: number): void {
  if (typeof document === "undefined") return;
  const effectiveEditorPx = getEffectiveEditorFontSize(scale, baseSize);
  document.documentElement.style.setProperty("--ui-font-scale", String(scale));
  document.documentElement.style.setProperty("--editor-base-font-size", `${baseSize}px`);
  document.documentElement.style.setProperty("--editor-font-size", `${effectiveEditorPx}px`);
}

function applyUiFontScaleNow(scale: UiFontScale): void {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(FONT_SCALE_KEY, String(scale));
  }
  syncFontCssVariables(scale, getEditorBaseFontSize());
}

let fontScaleApplyTimer: ReturnType<typeof setTimeout> | null = null;

/** Apply UI + editor font scale immediately and notify mounted editors/terminals. */
export function applyUiFontScale(scale: UiFontScale): void {
  if (fontScaleApplyTimer) {
    clearTimeout(fontScaleApplyTimer);
    fontScaleApplyTimer = null;
  }
  applyUiFontScaleNow(scale);
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("agentz-font-scale", { detail: scale }));
  }
}

export function setEditorBaseFontSize(size: number): number {
  const clamped = Math.min(
    MAX_EDITOR_FONT_SIZE,
    Math.max(MIN_EDITOR_FONT_SIZE, Math.round(size)),
  );
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(EDITOR_FONT_SIZE_KEY, String(clamped));
  }
  const scale = getUiFontScale();
  syncFontCssVariables(scale, clamped);
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("agentz-font-scale", { detail: scale }));
  }
  return clamped;
}

export function stepEditorBaseFontSize(delta: number): number {
  return setEditorBaseFontSize(getEditorBaseFontSize() + delta);
}

export function initUiFontScale(): UiFontScale {
  const scale = getUiFontScale();
  applyUiFontScaleNow(scale);
  return scale;
}

export function uiFontScaleLabel(scale: UiFontScale): string {
  return `${Math.round(scale * 100)}%`;
}
