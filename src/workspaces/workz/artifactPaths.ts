/** Slash-normalize a path and strip any Windows `\\?\` verbatim prefix. */
export function normalizePathSlashes(raw: string): string {
  let p = raw.trim().replace(/\\/g, "/");
  if (p.startsWith("//?/UNC/")) p = `//${p.slice("//?/UNC/".length)}`;
  else if (p.startsWith("//?/")) p = p.slice("//?/".length);
  return p;
}

/** True for `C:/…`, `C:\…`, UNC `//host/…`, or POSIX `/…` paths. */
export function isAbsoluteArtifactPath(raw: string): boolean {
  const p = normalizePathSlashes(raw);
  return /^[A-Za-z]:\//.test(p) || p.startsWith("//") || p.startsWith("/");
}

/**
 * Recover from an already-doubled join such as
 * `D:/projects/AgentZ/D:/projects/AgentZ/MODULE_DEPENDENCY_GRAPH.md`
 * (a root prepended to an already-absolute path): the real path starts at
 * the last embedded drive root.
 */
export function recoverEmbeddedAbsolutePath(raw: string): string {
  const p = normalizePathSlashes(raw);
  const embedded = /\/([A-Za-z]:\/.*)$/.exec(p);
  if (embedded && !/^[A-Za-z]:\//.test(p.slice(0, embedded.index + 1))) {
    return embedded[1];
  }
  // Doubled even when the outer prefix itself looks absolute:
  // `D:/projects/AgentZ/D:/projects/AgentZ/file` → keep from the 2nd drive.
  const drives = [...p.matchAll(/[A-Za-z]:\//g)];
  if (drives.length > 1) {
    const last = drives[drives.length - 1];
    if (last.index > 0 && p[last.index - 1] === "/") return p.slice(last.index);
  }
  return p;
}

function normalizedRoot(root: string): string {
  return normalizePathSlashes(root).replace(/\/+$/, "");
}

/** Case-insensitive (Windows-style) prefix check on a `/` boundary. */
function stripRootPrefix(path: string, root: string): string | null {
  const r = normalizedRoot(root);
  if (!r) return null;
  if (path.toLowerCase() === r.toLowerCase()) return "";
  if (path.toLowerCase().startsWith(`${r.toLowerCase()}/`)) {
    return path.slice(r.length + 1);
  }
  return null;
}

/**
 * Turn a tool-reported path into a project-relative artifact key when it
 * lives under one of `roots`; absolute paths outside every root are kept
 * absolute (never re-joined onto a root), and plain relative paths are
 * cleaned of leading `./` / separators.
 */
export function relativizeArtifactPath(raw: string, roots: Array<string | null | undefined> = []): string {
  let p = recoverEmbeddedAbsolutePath(raw);
  p = p.replace(/^\.\//, "").replace(/\/+$/, (m) => (p === m ? p : ""));
  if (isAbsoluteArtifactPath(p)) {
    for (const root of roots) {
      if (!root?.trim()) continue;
      const rel = stripRootPrefix(p, root);
      if (rel !== null) return rel;
    }
    // A single leading `/` was historically treated as project-relative
    // (tool output often reports `/docs/plan.md`); drive-letter and UNC
    // absolutes outside every root stay absolute so they open as-is.
    if (p.startsWith("/") && !p.startsWith("//")) return p.replace(/^\/+/, "");
    return p;
  }
  // Relative path that still embeds a known root's tail (e.g. root was
  // reported without its drive letter) — fall through as a plain relative.
  return p.replace(/^\/+/, "");
}

/**
 * Relative artifact path → absolute file path inside the active workspace.
 * Absolute artifact paths (the bug behind
 * `D:\projects\AgentZ/D:/projects/AgentZ/…`) are used as-is, and a path
 * already containing the root is relativized first — never double-joined.
 */
export function resolveArtifactFullPath(
  projectDir: string,
  relativePath: string,
  workspaceDir?: string | null,
): string {
  const roots = [workspaceDir, projectDir].filter(
    (r): r is string => Boolean(r?.trim()),
  );
  const rel = relativizeArtifactPath(relativePath, roots);
  if (isAbsoluteArtifactPath(rel)) return rel;
  const root = (workspaceDir?.trim() || projectDir).replace(/[/\\]+$/, "");
  if (!rel) return normalizePathSlashes(root);
  return `${normalizePathSlashes(root)}/${rel}`;
}

export interface ArtifactNode {
  name: string;
  /** Project-relative path used as the artifact key. */
  path: string;
  isDir: boolean;
  children?: ArtifactNode[];
}

/** Turn a flat path list into a folder tree for the artifacts drawer. */
export function buildArtifactTree(paths: string[]): ArtifactNode[] {
  const root: ArtifactNode[] = [];
  const sorted = [...paths].sort((a, b) => a.localeCompare(b));

  for (const full of sorted) {
    const parts = full.split("/").filter(Boolean);
    let level = root;
    let acc = "";
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      acc = acc ? `${acc}/${part}` : part;
      const isDir = i < parts.length - 1;
      let node = level.find((n) => n.name === part);
      if (!node) {
        node = { name: part, path: acc, isDir, children: isDir ? [] : undefined };
        level.push(node);
        level.sort((a, b) => {
          if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
          return a.name.localeCompare(b.name);
        });
      }
      if (isDir) {
        if (!node.children) node.children = [];
        level = node.children;
      }
    }
  }
  return root;
}

export function isPdfPath(name: string): boolean {
  return /\.pdf$/i.test(name);
}

/* ── Artifact list filtering / sorting (progressive filter + sort) ──── */

export type ArtifactSortMode = "time-desc" | "time-asc" | "name-asc" | "name-desc" | "path-asc";

/**
 * Progressive (incremental) filter: every whitespace-separated token must
 * appear in the path, so typing more text only ever narrows the result.
 * Matching is case-insensitive and ignores `/` vs `\` spelling.
 */
export function filterArtifactPaths(paths: string[], query: string): string[] {
  const tokens = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return [...paths];
  return paths.filter((p) => {
    const hay = normalizePathSlashes(p).toLowerCase();
    return tokens.every((tok) => hay.includes(tok.replace(/\\/g, "/")));
  });
}

function artifactBasename(p: string): string {
  const parts = normalizePathSlashes(p).split("/");
  return parts[parts.length - 1] ?? p;
}

/**
 * Sort artifact paths. The input order is generation order (oldest →
 * newest as collected, or newest-first when callers pass it that way), so
 * `time-desc` preserves the incoming order untouched — callers feed
 * newest-first lists and the most recently generated artifact stays on top.
 */
export function sortArtifactPaths(paths: string[], mode: ArtifactSortMode): string[] {
  const list = [...paths];
  switch (mode) {
    case "time-desc":
      return list;
    case "time-asc":
      return list.reverse();
    case "name-asc":
      return list.sort(
        (a, b) =>
          artifactBasename(a).localeCompare(artifactBasename(b)) || a.localeCompare(b),
      );
    case "name-desc":
      return list.sort(
        (a, b) =>
          artifactBasename(b).localeCompare(artifactBasename(a)) || b.localeCompare(a),
      );
    case "path-asc":
      return list.sort((a, b) => a.localeCompare(b));
  }
}
