import { invoke } from "@tauri-apps/api/core";

export interface CodeMapFile {
  id: string;
  path: string;
  name: string;
  group: string;
  layer: string;
  loc: number;
  churn: number;
  imports: number;
  imported_by: number;
}

export interface CodeMapEdge {
  from: string;
  to: string;
}

export interface CodeMapGroup {
  name: string;
  files: number;
  loc: number;
  churn: number;
  imported_by: number;
  layer: string;
}

export interface CodeMapGroupEdge {
  from: string;
  to: string;
  weight: number;
}

export interface CodeMapData {
  ready: boolean;
  generated_at: string | null;
  churn_days: number;
  churn_available: boolean;
  files: CodeMapFile[];
  edges: CodeMapEdge[];
  edges_truncated: boolean;
  groups: CodeMapGroup[];
  group_edges: CodeMapGroupEdge[];
}

export function fetchCodeMap(projectDir: string): Promise<CodeMapData> {
  return invoke<CodeMapData>("code_map_data", { projectDir });
}

export interface CallGraphNode {
  id: number;
  name: string;
  kind: string;
  file: string;
  start_line: number;
  container: string | null;
  center: boolean;
}

export interface CallGraphData {
  nodes: CallGraphNode[];
  edges: { from: number; to: number }[];
  truncated: boolean;
}

export function findSymbols(projectDir: string, query: string): Promise<CallGraphNode[]> {
  return invoke<CallGraphNode[]>("symbol_find", { projectDir, query });
}

export function fetchCallGraph(
  projectDir: string,
  symbolId: number,
  depth = 2,
): Promise<CallGraphData> {
  return invoke<CallGraphData>("symbol_call_graph", { projectDir, symbolId, depth });
}
