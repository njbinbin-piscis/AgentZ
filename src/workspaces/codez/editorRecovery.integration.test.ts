import { expect, it, vi } from "vitest";
const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
import { EditorConflictTracker } from "./editorConflict";
import { isRecoveryPath, mergeRestoredTabs, recoveryOriginalPath, restoreRecoveryTab } from "./editorRecovery";
import { editorSnapshotFromTabs } from "../../services/tauri/workspace";
import type { OpenTab } from "./types";

const tab = (content: string, isDirty = false): OpenTab => ({
  path: "src/file.ts", name: "file.ts", language: "typescript", content, isDirty, isReadOnly: false, viewMode: "editor",
});

it("a real completed save stays current while an older restored draft survives separately", () => {
  const tracker = new EditorConflictTracker(); tracker.setScope("project"); tracker.open("src/file.ts", "new saved content", true);
  expect(tracker.settleSave(tracker.beginSave("src/file.ts")!, true).status).toBe("saved");
  const current = tab(tracker.snapshot("src/file.ts")!.content, tracker.snapshot("src/file.ts")!.isDirty);
  const result = mergeRestoredTabs([current], [tab("old recovered draft", true)]);
  expect(result.find(t => t.path === current.path)).toEqual(current);
  const recovered = result.find(t => isRecoveryPath(t.path))!;
  expect(recovered.content).toBe("old recovered draft");
  expect(recovered.isDirty).toBe(true); expect(recovered.isReadOnly).toBe(true);
  expect(recoveryOriginalPath(recovered.path)).toBe(current.path);
});

it("close/reopen keeps the new live identity and never merges old draft into it", () => {
  const tracker = new EditorConflictTracker(); tracker.setScope("project"); tracker.open("src/file.ts", "same");
  const request = tracker.beginRead("src/file.ts")!;
  tracker.close("src/file.ts"); tracker.open("src/file.ts", "same");
  const current = tab("same"); const merged = mergeRestoredTabs([current], [tab("old draft", true)]);
  expect(merged.find(t => t.path === current.path)).toEqual(current);
  expect(merged.filter(t => isRecoveryPath(t.path)).map(t => t.content)).toEqual(["old draft"]);
  expect(tracker.settleRead(request, { kind: "content", content: "late disk" }).status).toBe("stale");
});

it("same-content dirty restoration is retained independently rather than treating equality as consent", () => {
  const current = tab("same");
  const merged = mergeRestoredTabs([current], [tab("same", true)]);
  expect(merged.find(t => t.path === current.path)).toEqual(current);
  expect(merged.some(t => isRecoveryPath(t.path) && t.content === "same" && t.isDirty && t.isReadOnly)).toBe(true);
});

it("real workspace serializer round-trips recovery content without a native file request", () => {
  invoke.mockClear();
  const merged = mergeRestoredTabs([tab("live")], [tab("retained draft", true)]);
  const recovered = merged.find(t => isRecoveryPath(t.path))!;
  const snapshot = editorSnapshotFromTabs(merged, recovered.path);
  expect(snapshot.open_paths).toContain(recovered.path);
  expect(snapshot.dirty_buffers[recovered.path]).toBe("retained draft");
  const restored = restoreRecoveryTab(recovered.path, snapshot.dirty_buffers[recovered.path]);
  expect(restored?.path).toBe(recovered.path); expect(restored?.content).toBe("retained draft");
  expect(restored?.isReadOnly).toBe(true); expect(restored?.isDirty).toBe(true);
  expect(restoreRecoveryTab(recovered.path, undefined)).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});
