// Window-mode shortcuts modelled on Cursor's Agents window / IDE switch:
//   Ctrl/Cmd+Shift+N      -> IDE (CodeZ, editor-centric)
//   Ctrl/Cmd+Alt+N        -> Agents (WorkZ, task-centric)
//   Ctrl/Cmd+Shift+Alt+N  -> Bots (BotZ, agent-centric)
// Pure so it can be tested without a DOM.

export type AppModeTarget = "codez" | "workz" | "botz";

export interface ModeKeyEvent {
  key: string;
  code?: string;
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

/** Returns the mode a key event asks for, or null when it is not a mode shortcut. */
export function modeShortcutTarget(event: ModeKeyEvent): AppModeTarget | null {
  if (!(event.ctrlKey || event.metaKey)) return null;
  // Option on macOS rewrites `key`, so fall back to the physical key.
  if (event.key.toLowerCase() !== "n" && event.code !== "KeyN") return null;
  if (event.shiftKey && event.altKey) return "botz";
  if (event.shiftKey) return "codez";
  if (event.altKey) return "workz";
  return null;
}

/** Human-readable shortcut label for tooltips, platform-aware. */
export function modeShortcutLabel(target: AppModeTarget, isMac: boolean): string {
  const mod = isMac ? "⌘" : "Ctrl+";
  if (target === "codez") return isMac ? `${mod}⇧N` : `${mod}Shift+N`;
  if (target === "botz") return isMac ? `${mod}⇧⌥N` : `${mod}Shift+Alt+N`;
  return isMac ? `${mod}⌥N` : `${mod}Alt+N`;
}
