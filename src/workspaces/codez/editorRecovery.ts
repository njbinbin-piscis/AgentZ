import type { OpenTab } from "./types";

const PREFIX = "agentz-recovery:";
/** Reserved paths must never be forwarded to file, language-server or agent APIs. */
export function isRecoveryPath(path: string): boolean { return path.startsWith(PREFIX); }
export function recoveryOriginalPath(path: string): string | null {
  if (!isRecoveryPath(path)) return null;
  const suffix = path.slice(PREFIX.length); const separator = suffix.indexOf(":");
  if (separator <= 0 || separator === suffix.length - 1) return null;
  try {
    const original = decodeURIComponent(suffix.slice(separator + 1));
    return original && !original.includes("\0") && !isRecoveryPath(original) ? original : null;
  } catch { return null; }
}
function backup(path: string, original: string, content: string): OpenTab {
  return { path, name: `Recovered draft: ${original}`, language: null, content,
    isDirty: true, isReadOnly: true, viewMode: "editor" };
}
export function makeRecoveryTab(original: OpenTab): OpenTab {
  const source = isRecoveryPath(original.path) ? recoveryOriginalPath(original.path) : original.path;
  if (!source || source.includes("\0")) throw new Error("Invalid recovery source path");
  return backup(`${PREFIX}${crypto.randomUUID()}:${encodeURIComponent(source)}`, source, original.content);
}
/** Only persisted dirty text restores a reserved draft. No file IO or inferred disk content. */
export function restoreRecoveryTab(path: string, dirty: string | undefined): OpenTab | null {
  const original = recoveryOriginalPath(path);
  return original && dirty !== undefined ? backup(path, original, dirty) : null;
}
/** Current tabs carry newer intent even when clean or identical in content. */
export function mergeRestoredTabs(current: OpenTab[], restored: OpenTab[]): OpenTab[] {
  const merged = [...current]; const paths = new Set(current.map(tab => tab.path));
  for (const tab of restored) {
    if (paths.has(tab.path)) {
      if (tab.isDirty) { const recovered = makeRecoveryTab(tab); merged.push(recovered); paths.add(recovered.path); }
      continue;
    }
    const recovered = isRecoveryPath(tab.path) ? restoreRecoveryTab(tab.path, tab.isDirty ? tab.content : undefined) : tab;
    if (recovered) { merged.push(recovered); paths.add(recovered.path); }
  }
  return merged;
}
