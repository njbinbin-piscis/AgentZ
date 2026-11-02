// Breakpoint model + Monaco glyph-margin integration. Breakpoints are keyed by
// absolute file path and rendered as red dots in the editor gutter; clicking a
// gutter line toggles one. The debug controller reads these to send DAP
// `setBreakpoints` requests.

import type * as monaco from "monaco-editor";
import { debugStore } from "./debugStore";

type Listener = () => void;

class BreakpointStore {
  private map = new Map<string, Set<number>>();
  private listeners = new Set<Listener>();

  subscribe(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }
  private emit(): void {
    for (const l of this.listeners) l();
  }
  get(path: string): number[] {
    return [...(this.map.get(path) ?? [])].sort((a, b) => a - b);
  }
  all(): Map<string, Set<number>> {
    return this.map;
  }
  toggle(path: string, line: number): void {
    const set = this.map.get(path) ?? new Set<number>();
    if (set.has(line)) set.delete(line);
    else set.add(line);
    if (set.size === 0) this.map.delete(path);
    else this.map.set(path, set);
    this.emit();
  }
  has(path: string, line: number): boolean {
    return this.map.get(path)?.has(line) ?? false;
  }
}

export const breakpointStore = new BreakpointStore();

/** Absolute path of the file most recently shown in an editor (for `${file}` in launch configs). */
export const activeEditorFile: { path: string | null } = { path: null };

function samePath(a: string, b: string): boolean {
  const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
  const x = norm(a);
  const y = norm(b);
  return /^[a-z]:\//i.test(x) ? x.toLowerCase() === y.toLowerCase() : x === y;
}

/** Highlights the line the debugger is paused on, when it is in `path`. */
function attachDebugLine(
  editor: monaco.editor.IStandaloneCodeEditor,
  monacoNs: typeof monaco,
  path: string,
): () => void {
  const collection = editor.createDecorationsCollection([]);
  const render = () => {
    const snap = debugStore.getSnapshot();
    const top = snap.active && !snap.running ? snap.frames[0] : undefined;
    if (top?.source && samePath(top.source, path)) {
      collection.set([
        {
          range: new monacoNs.Range(top.line, 1, top.line, 1),
          options: { isWholeLine: true, className: "agentz-debug-line" },
        },
      ]);
    } else {
      collection.clear();
    }
  };
  const unsubscribe = debugStore.subscribe(render);
  render();
  return () => {
    unsubscribe();
    collection.clear();
  };
}

/**
 * Attach breakpoint toggling + rendering to a Monaco editor for `path`.
 * Returns a disposer. Requires `glyphMargin: true` in editor options.
 */
export function attachBreakpointGutter(
  editor: monaco.editor.IStandaloneCodeEditor,
  monacoNs: typeof monaco,
  path: string,
): monaco.IDisposable {
  activeEditorFile.path = path;
  const disposeDebugLine = attachDebugLine(editor, monacoNs, path);
  const collection = editor.createDecorationsCollection([]);

  const render = () => {
    const lines = breakpointStore.get(path);
    collection.set(
      lines.map((line) => ({
        range: new monacoNs.Range(line, 1, line, 1),
        options: {
          isWholeLine: false,
          glyphMarginClassName: "agentz-bp-glyph",
          glyphMarginHoverMessage: { value: "Breakpoint" },
        },
      })),
    );
  };

  const mouseSub = editor.onMouseDown((e) => {
    if (e.target.type === monacoNs.editor.MouseTargetType.GUTTER_GLYPH_MARGIN) {
      const line = e.target.position?.lineNumber;
      if (line) breakpointStore.toggle(path, line);
    }
  });
  const storeSub = breakpointStore.subscribe(render);
  render();

  return {
    dispose: () => {
      mouseSub.dispose();
      storeSub();
      disposeDebugLine();
      collection.clear();
    },
  };
}
