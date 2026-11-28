/**
 * Declarative `contributes.customEditors` matching for FileViewer / linkRouter.
 */
import type { InstalledExtension } from "../services/tauri/vsix";

export interface CustomEditorContribution {
  viewType: string;
  displayName: string;
  priority?: string;
  selector: Array<{ filenamePattern?: string }>;
  extensionId: string;
}

type Listener = () => void;

function matchPattern(fileName: string, pattern: string): boolean {
  // Supports simple "*.ext" and exact names; full glob is out of scope.
  const name = fileName.replace(/\\/g, "/").split("/").pop() ?? fileName;
  if (pattern.startsWith("*.") && !pattern.slice(2).includes("*") && !pattern.slice(2).includes("?")) {
    return name.toLowerCase().endsWith(pattern.slice(1).toLowerCase());
  }
  if (!/[?*[]/.test(pattern)) {
    return name.toLowerCase() === pattern.toLowerCase();
  }
  const re = new RegExp(
    "^" +
      pattern
        .replace(/[.+^${}()|[\]\\]/g, "\\$&")
        .replace(/\*/g, ".*")
        .replace(/\?/g, ".") +
      "$",
    "i",
  );
  return re.test(name);
}

class CustomEditorRegistry {
  private editors: CustomEditorContribution[] = [];
  private listeners = new Set<Listener>();

  subscribe = (l: Listener): (() => void) => {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  };

  private emit(): void {
    for (const l of this.listeners) l();
  }

  ingest(extensions: InstalledExtension[]): void {
    this.editors = [];
    for (const ext of extensions) {
      if (!ext.enabled) continue;
      const c = (ext.contributes ?? {}) as Record<string, unknown>;
      const arr = Array.isArray(c.customEditors) ? c.customEditors : [];
      for (const raw of arr) {
        if (!raw || typeof raw !== "object") continue;
        const E = raw as Record<string, unknown>;
        const viewType = typeof E.viewType === "string" ? E.viewType : "";
        if (!viewType) continue;
        const selector = Array.isArray(E.selector)
          ? (E.selector as Array<Record<string, unknown>>)
              .map((s) => ({
                filenamePattern: typeof s.filenamePattern === "string" ? s.filenamePattern : undefined,
              }))
              .filter((s) => !!s.filenamePattern)
          : [];
        if (selector.length === 0) continue;
        this.editors.push({
          viewType,
          displayName: typeof E.displayName === "string" ? E.displayName : viewType,
          priority: typeof E.priority === "string" ? E.priority : undefined,
          selector,
          extensionId: ext.id,
        });
      }
    }
    // default priority before option
    this.editors.sort((a, b) => {
      const rank = (p?: string) => (p === "default" ? 0 : p === "option" ? 2 : 1);
      return rank(a.priority) - rank(b.priority);
    });
    this.emit();
  }

  /** Best matching custom editor for a file path, if any. */
  match(filePath: string): CustomEditorContribution | undefined {
    const name = filePath.replace(/\\/g, "/").split("/").pop() ?? filePath;
    for (const ed of this.editors) {
      if (ed.selector.some((s) => s.filenamePattern && matchPattern(name, s.filenamePattern))) {
        return ed;
      }
    }
    return undefined;
  }

  all(): CustomEditorContribution[] {
    return [...this.editors];
  }
}

export const customEditorRegistry = new CustomEditorRegistry();
