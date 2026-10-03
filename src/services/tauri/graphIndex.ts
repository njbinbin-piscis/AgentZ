/**
 * Background knowledge-graph index (CodeGraph-style, no visualization UI).
 */
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type IndexPhase = "idle" | "queued" | "building";

export interface IndexBuildStatus {
  phase: IndexPhase;
  pending_files: number;
  last_built_at: string | null;
  last_error: string | null;
  nodes: number;
  edges: number;
}

export interface IndexBuildAck {
  accepted: boolean;
  phase: IndexPhase;
  message: string;
}

export function requestGraphIndex(projectDir: string): Promise<IndexBuildAck> {
  return invoke<IndexBuildAck>("graph_index_rebuild", { projectDir });
}

export function getGraphIndexStatus(projectDir: string): Promise<IndexBuildStatus> {
  return invoke<IndexBuildStatus>("graph_index_status", { projectDir });
}

export type GraphIndexVisualState = "none" | "indexing" | "ready" | "error";

export function graphIndexVisualState(st: IndexBuildStatus): GraphIndexVisualState {
  if (st.last_error) return "error";
  if (st.phase === "queued" || st.phase === "building") return "indexing";
  if (st.nodes > 0) return "ready";
  return "none";
}

function sameStatus(a: IndexBuildStatus, b: IndexBuildStatus): boolean {
  return (
    a.phase === b.phase &&
    a.pending_files === b.pending_files &&
    a.last_built_at === b.last_built_at &&
    a.last_error === b.last_error &&
    a.nodes === b.nodes &&
    a.edges === b.edges
  );
}

/** Poll graph index worker + graph.db stats for title-bar status coloring. */
export function useGraphIndexStatus(
  projectDir: string | null,
  refreshNonce = 0,
): IndexBuildStatus | null {
  const [status, setStatus] = useState<IndexBuildStatus | null>(null);

  useEffect(() => {
    if (!projectDir) {
      setStatus(null);
      return;
    }

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let lastPhase: IndexPhase | null = null;

    const schedule = (intervalMs: number) => {
      if (!cancelled) timer = setTimeout(() => void poll(), intervalMs);
    };

    const poll = async () => {
      timer = undefined;
      // An idle index only needs a heartbeat while the window is on screen.
      if (lastPhase === "idle" && document.hidden) return;
      try {
        const st = await getGraphIndexStatus(projectDir);
        if (cancelled) return;
        lastPhase = st.phase;
        // Skip identical snapshots: this hook lives in App, so every update
        // re-renders the whole tree.
        setStatus((prev) => (prev && sameStatus(prev, st) ? prev : st));
        schedule(st.phase === "idle" ? 5000 : 800);
      } catch {
        schedule(5000);
      }
    };

    const onVisibility = () => {
      if (!document.hidden && timer === undefined) void poll();
    };
    document.addEventListener("visibilitychange", onVisibility);

    void poll();
    return () => {
      cancelled = true;
      document.removeEventListener("visibilitychange", onVisibility);
      if (timer !== undefined) clearTimeout(timer);
    };
  }, [projectDir, refreshNonce]);

  return status;
}

/** Poll until index worker is idle or timeout. */
export async function waitGraphIndexIdle(
  projectDir: string,
  timeoutMs = 120_000,
): Promise<IndexBuildStatus> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const st = await getGraphIndexStatus(projectDir);
    if (st.phase === "idle") return st;
    await new Promise((r) => setTimeout(r, 400));
  }
  return getGraphIndexStatus(projectDir);
}
