import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { FileNode } from "./types";

export type PaletteMode = "files" | "commands";

export interface PaletteCommandItem {
  id: string;
  label: string;
  shortcut?: string;
  category?: string;
  run: () => void;
}

export interface CommandPaletteProps {
  open: boolean;
  initialMode: PaletteMode;
  fileTree: FileNode[];
  recentPaths?: string[];
  commands: PaletteCommandItem[];
  onSelectFile: (path: string) => void;
  onClose: () => void;
}

export function flattenFileTreePaths(nodes: FileNode[]): string[] {
  const out: string[] = [];
  const walk = (list: FileNode[]) => {
    for (const node of list) {
      if (node.is_dir) {
        if (node.children) walk(node.children);
      } else {
        out.push(node.path.replace(/\\/g, "/"));
      }
    }
  };
  walk(nodes);
  return out;
}

export function fuzzyScore(candidate: string, query: string): number {
  const q = query.trim().toLowerCase();
  if (!q) return 1;
  const c = candidate.toLowerCase();
  const baseName = c.split("/").pop() ?? c;
  if (baseName === q) return 1000;
  if (baseName.startsWith(q)) return 800 - baseName.length;
  if (baseName.includes(q)) return 600 - baseName.indexOf(q);
  if (c.includes(q)) return 400 - c.indexOf(q);

  let qi = 0;
  let score = 100;
  let lastMatch = -1;
  for (let i = 0; i < c.length && qi < q.length; i++) {
    if (c[i] === q[qi]) {
      if (lastMatch === i - 1) score += 15;
      lastMatch = i;
      qi++;
    }
  }
  return qi === q.length ? score : -1;
}

export default function CommandPalette({
  open,
  initialMode,
  fileTree,
  recentPaths = [],
  commands,
  onSelectFile,
  onClose,
}: CommandPaletteProps) {
  const { t } = useTranslation();
  const [rawQuery, setRawQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setRawQuery(initialMode === "commands" ? "> " : "");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open, initialMode]);

  const effectiveMode: PaletteMode = rawQuery.trimStart().startsWith(">") ? "commands" : "files";
  const cleanQuery =
    effectiveMode === "commands" ? rawQuery.trimStart().replace(/^>\s*/, "") : rawQuery.trim();

  const allFiles = useMemo(() => flattenFileTreePaths(fileTree), [fileTree]);

  const filteredFiles = useMemo(() => {
    if (effectiveMode !== "files") return [];
    const recentOrder = new Map(recentPaths.map((p, idx) => [p.replace(/\\/g, "/"), idx]));
    const scored = allFiles
      .map((path) => {
        const s = fuzzyScore(path, cleanQuery);
        const recentBoost = recentOrder.has(path) ? 50 - (recentOrder.get(path) ?? 0) : 0;
        return { path, score: s < 0 ? -1 : s + (cleanQuery ? 0 : recentBoost) };
      })
      .filter((item) => item.score >= 0)
      .sort((a, b) => b.score - a.score || a.path.localeCompare(b.path));
    return scored.slice(0, 40).map((item) => item.path);
  }, [effectiveMode, allFiles, recentPaths, cleanQuery]);

  const filteredCommands = useMemo(() => {
    if (effectiveMode !== "commands") return [];
    return commands
      .map((cmd) => ({ cmd, score: fuzzyScore(cmd.label, cleanQuery) }))
      .filter((item) => item.score >= 0)
      .sort((a, b) => b.score - a.score)
      .map((item) => item.cmd);
  }, [effectiveMode, commands, cleanQuery]);

  const totalItems =
    effectiveMode === "commands" ? filteredCommands.length : filteredFiles.length;

  useEffect(() => {
    setSelectedIndex(0);
  }, [rawQuery, effectiveMode]);

  if (!open) return null;

  const executeIndex = (idx: number) => {
    if (effectiveMode === "commands") {
      const target = filteredCommands[idx];
      if (target) {
        onClose();
        target.run();
      }
    } else {
      const target = filteredFiles[idx];
      if (target) {
        onClose();
        onSelectFile(target);
      }
    }
  };

  return (
    <div
      className="ide-command-palette-backdrop"
      data-testid="command-palette-backdrop"
      onClick={onClose}
    >
      <div
        className="ide-command-palette"
        data-testid="command-palette-modal"
        role="dialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="ide-command-palette-input-wrap">
          <input
            ref={inputRef}
            type="text"
            data-testid="command-palette-input"
            className="ide-command-palette-input"
            value={rawQuery}
            placeholder={
              effectiveMode === "commands"
                ? t("ide.commandPalettePlaceholder")
                : t("ide.quickOpenPlaceholder")
            }
            onChange={(e) => setRawQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                onClose();
              } else if (e.key === "ArrowDown") {
                e.preventDefault();
                setSelectedIndex((prev) => (totalItems > 0 ? (prev + 1) % totalItems : 0));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setSelectedIndex((prev) =>
                  totalItems > 0 ? (prev - 1 + totalItems) % totalItems : 0,
                );
              } else if (e.key === "Enter") {
                e.preventDefault();
                executeIndex(selectedIndex);
              }
            }}
          />
        </div>
        <div className="ide-command-palette-list" data-testid="command-palette-list">
          {totalItems === 0 ? (
            <div className="ide-command-palette-empty">{t("ide.paletteNoResults")}</div>
          ) : effectiveMode === "commands" ? (
            filteredCommands.map((cmd, idx) => (
              <button
                key={cmd.id}
                type="button"
                data-testid={`palette-cmd-${cmd.id}`}
                className={`ide-command-palette-item ${idx === selectedIndex ? "active" : ""}`}
                onMouseEnter={() => setSelectedIndex(idx)}
                onClick={() => executeIndex(idx)}
              >
                <span className="ide-command-palette-item-label">{cmd.label}</span>
                {cmd.shortcut && (
                  <kbd className="ide-command-palette-item-kbd">{cmd.shortcut}</kbd>
                )}
              </button>
            ))
          ) : (
            filteredFiles.map((filePath, idx) => {
              const name = filePath.split("/").pop() ?? filePath;
              const dir = filePath.includes("/")
                ? filePath.slice(0, filePath.lastIndexOf("/"))
                : "";
              return (
                <button
                  key={filePath}
                  type="button"
                  data-testid={`palette-file-${filePath}`}
                  className={`ide-command-palette-item ${idx === selectedIndex ? "active" : ""}`}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  onClick={() => executeIndex(idx)}
                >
                  <span className="ide-command-palette-item-label">
                    <strong>{name}</strong>
                    {dir && <span className="ide-command-palette-item-dir">{dir}</span>}
                  </span>
                </button>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
