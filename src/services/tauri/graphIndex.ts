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

    const poll = async () => {
      try {
        const st = await getGraphIndexStatus(projectDir);
        if (cancelled) return;
        setStatus(st);
        const intervalMs = st.phase === "idle" ? 5000 : 800;
        timer = setTimeout(() => void poll(), intervalMs);
      } catch {
        if (!cancelled) timer = setTimeout(() => void poll(), 5000);
      }
    };

    void poll();
    return () => {
      cancelled = true;
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
