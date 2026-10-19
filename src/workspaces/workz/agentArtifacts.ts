import type { GitFileStatus } from "../codez/types";
import { relativizeArtifactPath } from "./artifactPaths";

export interface AgentToolEvent {
  id: string;
  name: string;
  status: "running" | "done" | "error";
  result?: string;
  path?: string;
  input?: unknown;
  textOffset?: number;
}

export interface AgentStep {
  role: "user" | "assistant";
  text: string;
  tools: AgentToolEvent[];
  id?: string;
  turnId?: string;
}

function pathFromToolInput(input: unknown): string | undefined {
  if (!input || typeof input !== "object") return undefined;
  const rec = input as Record<string, unknown>;
  for (const key of ["path", "file", "file_path", "target"]) {
    const v = rec[key];
    if (typeof v === "string" && v.trim()) return v.trim();
  }
  return undefined;
}

/** Extract a file path from tool name + payload, relativized to `roots` when possible. */
export function pathFromToolEvent(
  name: string,
  input: unknown,
  result?: string,
  roots: Array<string | null | undefined> = [],
): string | undefined {
  const fromInput = pathFromToolInput(input);
  if (fromInput) return normalizeArtifactPath(fromInput, roots);

  if (!result) return undefined;
  const n = name.toLowerCase();
  if (n.includes("file") || n.includes("write") || n.includes("read") || n.includes("edit")) {
    const m = result.match(/(?:^|\s)([^\s'"]+\.[a-zA-Z0-9]+)(?:\s|$)/);
    if (m?.[1]) return normalizeArtifactPath(m[1], roots);
  }
  return undefined;
}

export function normalizeArtifactPath(
  raw: string,
  roots: Array<string | null | undefined> = [],
): string {
  return relativizeArtifactPath(raw, roots);
}

export interface ToolStepLike {
  name: string;
  input?: unknown;
  result?: string;
}

/**
 * Order paths newest-first: the last tool invocation that touched a path
 * determines its position, so later-generated artifacts sort to the front
 * and re-generated files bubble back to the top.
 */
function newestFirstByLastTouch(entries: string[]): string[] {
  const lastTouch = new Map<string, number>();
  entries.forEach((p, i) => {
    if (p) lastTouch.set(p, i);
  });
  return [...lastTouch.entries()].sort((a, b) => b[1] - a[1]).map(([p]) => p);
}

/** Agent-produced paths from tool invocations in the current UI step list, newest first. */
export function collectArtifactsFromToolSteps(
  toolSteps: ToolStepLike[],
  roots: Array<string | null | undefined> = [],
): string[] {
  const entries: string[] = [];
  for (const step of toolSteps) {
    const p = pathFromToolEvent(step.name, step.input, step.result, roots);
    if (p) entries.push(p);
  }
  return newestFirstByLastTouch(entries);
}

/** Agent-produced paths from persisted WorkZ step history (not git status), newest first. */
export function collectArtifactsFromSteps(
  steps: AgentStep[],
  roots: Array<string | null | undefined> = [],
): string[] {
  const entries: string[] = [];
  for (const step of steps) {
    for (const tool of step.tools) {
      const p =
        tool.path != null
          ? normalizeArtifactPath(tool.path, roots)
          : pathFromToolEvent(tool.name, tool.input, tool.result, roots);
      if (p) entries.push(p);
    }
  }
  return newestFirstByLastTouch(entries);
}

/** @deprecated Prefer {@link collectArtifactsFromSteps} — git changes belong in the Changes tab. */
export function collectArtifacts(
  steps: AgentStep[],
  _changes?: GitFileStatus[],
  roots: Array<string | null | undefined> = [],
): string[] {
  return collectArtifactsFromSteps(steps, roots);
}
