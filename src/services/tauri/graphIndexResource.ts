import { invoke } from "@tauri-apps/api/core";

export interface GraphIndexResourceSnapshot {
  queued_projects: number;
  pending_files: number;
  worker_projects: number;
  phase_entries: number;
  last_built_entries: number;
  last_error_entries: number;
  retiring_projects: number;
}

export const graphIndexResourceSnapshot = () =>
  invoke<GraphIndexResourceSnapshot>("graph_index_resource_snapshot");
