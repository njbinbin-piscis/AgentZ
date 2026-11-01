/**
 * Embedded browser IPC — drives a headless Chromium over CDP in the Rust host.
 * Each chat session has its own tab; pass that session's id so the panel and
 * the agent tool stay on the same page and do not touch other sessions.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Tauri event emitted after agent browser tool actions. */
export const BROWSER_CHANGED = "browser-changed";

export interface BrowserChangedPayload {
  action: string;
  url?: string | null;
  selector?: string | null;
  ref?: string | null;
  session_id?: string | null;
}

export interface BrowserState {
  open: boolean;
  url: string;
  viewport_width: number;
  viewport_height: number;
  snapshot_generation?: number;
}

export interface BrowserCloseGuard {
  can_close: boolean;
  agent_active: boolean;
  reason?: string | null;
}

export interface PickedElement {
  selector: string;
  tag: string;
  text: string;
  html: string;
  id?: string;
  class_name?: string;
  rect_x?: number;
  rect_y?: number;
  rect_width?: number;
  rect_height?: number;
  dom_path?: string;
  react_component?: string;
}

/** Navigate this session's page; returns the final URL. */
export function browserNavigate(sessionId: string, url: string): Promise<string> {
  return invoke<string>("browser_navigate", { sessionId, url });
}

/** Resize the Chromium viewport to match the panel (CSS pixels). */
export function browserSetViewport(
  sessionId: string,
  width: number,
  height: number,
): Promise<[number, number]> {
  return invoke<[number, number]>("browser_set_viewport", { sessionId, width, height });
}

/** Page scroll geometry, used to drive the synthetic scrollbars. */
export interface ScrollInfo {
  scroll_x: number;
  scroll_y: number;
  scroll_width: number;
  scroll_height: number;
  client_width: number;
  client_height: number;
}

/** Scroll this session's page by a wheel delta (CSS pixels); returns new geometry. */
export function browserScrollBy(sessionId: string, x: number, y: number): Promise<ScrollInfo> {
  return invoke<ScrollInfo>("browser_scroll_by", { sessionId, x, y });
}

/** Scroll this session's page to an absolute offset (CSS pixels). */
export function browserScrollTo(sessionId: string, x: number, y: number): Promise<ScrollInfo> {
  return invoke<ScrollInfo>("browser_scroll_to", { sessionId, x, y });
}

/** Read the page's current scroll position + content extents. */
export function browserScrollInfo(sessionId: string): Promise<ScrollInfo> {
  return invoke<ScrollInfo>("browser_scroll_info", { sessionId });
}

/** Capture this session's page as a base64-encoded PNG (no data: prefix). */
export function browserScreenshot(sessionId: string): Promise<string> {
  return invoke<string>("browser_screenshot", { sessionId });
}

/** Forward a click at viewport coordinates (CSS pixels). */
export function browserClickAt(sessionId: string, x: number, y: number): Promise<boolean> {
  return invoke<boolean>("browser_click_at", { sessionId, x, y });
}

/** Identify the element at viewport coordinates for pick / inspect. */
export function browserPickAt(sessionId: string, x: number, y: number): Promise<PickedElement | null> {
  return invoke<PickedElement | null>("browser_pick_at", { sessionId, x, y });
}

/** Hover-inspect: element + bounding rect at viewport coordinates. */
export function browserInspectAt(sessionId: string, x: number, y: number): Promise<PickedElement | null> {
  return invoke<PickedElement | null>("browser_inspect_at", { sessionId, x, y });
}

export function browserCurrentUrl(sessionId: string): Promise<string> {
  return invoke<string>("browser_current_url", { sessionId });
}

export function browserIsOpen(): Promise<boolean> {
  return invoke<boolean>("browser_is_open");
}

export function browserClose(sessionId: string): Promise<void> {
  return invoke<void>("browser_close", { sessionId });
}

/** Close every session's page and the shared Chromium process. */
export function browserCloseAll(): Promise<void> {
  return invoke<void>("browser_close", { sessionId: null });
}

export function browserState(sessionId: string): Promise<BrowserState> {
  return invoke<BrowserState>("browser_state", { sessionId });
}

export function browserCloseGuard(): Promise<BrowserCloseGuard> {
  return invoke<BrowserCloseGuard>("browser_close_guard");
}

/** Subscribe to agent-driven browser mutations (immediate panel refresh). */
export function onBrowserChanged(
  handler: (payload: BrowserChangedPayload) => void,
): Promise<UnlistenFn> {
  return listen<BrowserChangedPayload>(BROWSER_CHANGED, (ev) => handler(ev.payload));
}

/** Compact chat token for a picked browser element (stored in session history). */
export function browserElementPlaceholder(el: PickedElement): string {
  return `@browser-element(${el.selector})`;
}

/** Short label shown in composer / message chips (Cursor-style). */
export function browserElementChipLabel(el: PickedElement): string {
  if (el.react_component?.trim()) return el.react_component.trim();
  const tag = el.tag?.trim();
  if (tag) return `<${tag}>`;
  return el.selector;
}

/** Type text into the focused page element (typing, IME, paste). */
export function browserInsertText(sessionId: string, text: string): Promise<void> {
  return invoke<void>("browser_insert_text", { sessionId, text });
}

/** Press a non-printable key; modifiers: 1 Alt, 2 Ctrl, 4 Meta, 8 Shift. */
export function browserPressKey(sessionId: string, key: string, modifiers = 0): Promise<void> {
  return invoke<void>("browser_press_key", { sessionId, key, modifiers });
}

/** Go back (-1) or forward (+1) in page history. */
export function browserHistoryStep(sessionId: string, delta: number): Promise<void> {
  return invoke<void>("browser_history_step", { sessionId, delta });
}
