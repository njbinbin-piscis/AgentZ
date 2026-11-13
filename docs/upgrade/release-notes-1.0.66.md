# AgentZ 1.0.66 release notes

Release date: 2026-11-13. See the 1.0.66 entry in `CHANGELOG.md`.

## Added

- **App-wide right-click menus; the browser context menu is gone.** Right-clicking anywhere no longer shows Chromium's Back / Reload / Save-as / Inspect menu. A shared `ContextMenu` component (viewport-clamped, full keyboard navigation, danger styling, outside-click/scroll dismissal) provides semantic menus per surface:
  - Chat messages: copy message / selection; assistant messages also offer fork-from-here and restore-to-checkpoint.
  - Code blocks: copy code, copy selection, copy as Markdown (language fence included).
  - Tool calls & file-edit cards: copy name / input / output, open file, copy path / diff.
  - Session lists (CodeZ & WorkZ) and project groups: open, copy title/path, delete/remove.
  - Collaboration board cards: copy title / todo ID / owner.
  - Terminal: copy, paste (into the PTY), select all, send selection to chat, clear.
  - Embedded browser canvas: back, forward, reload, copy page URL, screenshot to chat.
  - Inputs / links / images: cut/copy/paste/select-all, open/copy link, copy image (data-URI images are copied as PNG to the clipboard instead of a giant base64 string).
  - Monaco keeps its native editor menu; surfaces can opt out via `data-native-context-menu`.
  - Menu items are built lazily at right-click time (live selection), keeping render pure.
  - See `docs/context-menu-audit.md` for the full surface audit.

## Fixed

- **Changes-panel icon buttons showed literal "?".** The toolbar refresh button and the per-file undo/keep buttons had their unicode glyphs corrupted to ASCII question marks in an earlier commit. All three are now inline SVG icons (refresh / undo / keep-check) with aria-labels, hover/disabled styling, and a regression test that fails on any "?" placeholder.
- **File-edit card "Open File" actually opens the file.** The `agentz-open-file` event previously had no listener (clicking the card was a no-op); it is now bridged to the editor's real open-file path.
- **Terminal right-click without a selection** no longer falls through to the browser menu — a proper menu (paste / select all / clear) is shown.
- **Theme storage access is defensive** (`readStorage`): when localStorage is absent or non-functional (privacy modes, embedded webviews, minimal test stubs) the app falls back to defaults instead of crashing the render.
