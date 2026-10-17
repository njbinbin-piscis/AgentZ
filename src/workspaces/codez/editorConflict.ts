/** In-memory editor operation identities. No IO, host authority or disk write CAS. */
export interface EditorSnapshot {
  project: string; path: string; content: string; isDirty: boolean;
  scopeGeneration: number; tabIdentity: number; localRevision: number;
}
export interface CapturedOperation extends EditorSnapshot {
  kind: "read" | "save";
  requestGeneration: number;
  allowDiscard: boolean;
}
export type ReadResult = { kind: "content"; content: string } | { kind: "deleted" } | { kind: "error" };
export type ReadSettlement = { status: "stale" }
  | { status: "applied"; content: string }
  | { status: "conflict"; localContent: string; diskContent: string }
  | { status: "deleted"; localContent: string; preserveBuffer: boolean }
  | { status: "unavailable"; localContent: string; isDirty: boolean };
export type SaveSettlement = { status: "stale" | "saved" | "failed" };
interface Tab extends EditorSnapshot { readGeneration: number; saveGeneration: number }
function next(value: number): number {
  if (!Number.isSafeInteger(value) || value < 0 || value === Number.MAX_SAFE_INTEGER) throw new Error("Editor identity exhausted");
  return value + 1;
}
/**
 * Caller must synchronously report every local edit and every close/reopen/scope
 * change, before starting/settling async operations. Content equality alone is
 * never an identity proof. Do not seed this from a debounced persisted snapshot.
 */
export class EditorConflictTracker {
  private project: string | null = null;
  private scopeGeneration = 0;
  private tabGeneration = 0;
  private tabs = new Map<string, Tab>();
  setScope(project: string | null): void {
    this.scopeGeneration = next(this.scopeGeneration); this.project = project; this.tabs.clear();
  }
  open(path: string, content: string, isDirty = false): EditorSnapshot {
    if (!this.project || !path) throw new Error("Live project/tab identity required");
    this.tabGeneration = next(this.tabGeneration);
    this.tabs.set(path, { project: this.project, path, content, isDirty, scopeGeneration: this.scopeGeneration, tabIdentity: this.tabGeneration, localRevision: 0, readGeneration: 0, saveGeneration: 0 });
    return this.snapshot(path)!;
  }
  close(path: string): void { this.tabs.delete(path); }
  snapshot(path: string): EditorSnapshot | null {
    const tab = this.tabs.get(path); if (!tab) return null;
    const { project, path: p, content, isDirty, scopeGeneration, tabIdentity, localRevision } = tab;
    return { project, path: p, content, isDirty, scopeGeneration, tabIdentity, localRevision };
  }
  edit(path: string, content: string): EditorSnapshot | null {
    const tab = this.tabs.get(path); if (!tab) return null;
    tab.localRevision = next(tab.localRevision); tab.content = content; tab.isDirty = true;
    return this.snapshot(path);
  }
  /** Only pass allowDiscard after an explicit confirmation; later edits still invalidate it. */
  beginRead(path: string, options: { allowDiscard?: boolean } = {}): CapturedOperation | null { return this.begin(path, "read", options.allowDiscard === true); }
  beginSave(path: string): CapturedOperation | null { return this.begin(path, "save"); }
  private begin(path: string, kind: "read" | "save", allowDiscard = false): CapturedOperation | null {
    const tab = this.tabs.get(path); if (!tab) return null;
    const generation = next(kind === "read" ? tab.readGeneration : tab.saveGeneration);
    if (kind === "read") tab.readGeneration = generation; else tab.saveGeneration = generation;
    return { ...this.snapshot(path)!, kind, requestGeneration: generation, allowDiscard };
  }
  private current(captured: CapturedOperation, kind: "read" | "save"): Tab | null {
    const tab = this.tabs.get(captured.path);
    if (!tab || captured.kind !== kind || captured.project !== this.project || captured.scopeGeneration !== this.scopeGeneration
        || captured.tabIdentity !== tab.tabIdentity || captured.localRevision !== tab.localRevision
        || captured.content !== tab.content || captured.isDirty !== tab.isDirty
        || captured.requestGeneration !== (kind === "read" ? tab.readGeneration : tab.saveGeneration)) return null;
    return tab;
  }
  settleRead(captured: CapturedOperation, result: ReadResult): ReadSettlement {
    const tab = this.current(captured, "read"); if (!tab) return { status: "stale" };
    // Consume this request even on failure/conflict: a duplicated result cannot reapply.
    const generation = next(tab.readGeneration);
    if (result.kind === "error") { tab.readGeneration = generation; return { status: "unavailable", localContent: tab.content, isDirty: tab.isDirty }; }
    if (result.kind === "deleted") { tab.readGeneration = generation; return { status: "deleted", localContent: tab.content, preserveBuffer: tab.isDirty }; }
    if (tab.isDirty && !captured.allowDiscard) { tab.readGeneration = generation; return { status: "conflict", localContent: tab.content, diskContent: result.content }; }
    const revision = next(tab.localRevision); tab.readGeneration = generation; tab.localRevision = revision; tab.content = result.content; tab.isDirty = false;
    return { status: "applied", content: result.content };
  }
  settleSave(captured: CapturedOperation, succeeded: boolean): SaveSettlement {
    const tab = this.current(captured, "save"); if (!tab) return { status: "stale" };
    const generation = next(tab.saveGeneration);
    if (!succeeded) { tab.saveGeneration = generation; return { status: "failed" }; }
    const revision = next(tab.localRevision); const readGeneration = next(tab.readGeneration);
    tab.saveGeneration = generation; tab.localRevision = revision; tab.readGeneration = readGeneration; tab.isDirty = false;
    return { status: "saved" };
  }
}
