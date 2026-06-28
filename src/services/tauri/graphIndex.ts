/**
 * Background knowledge-graph index (CodeGraph-style, no visualization UI).
 */
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
