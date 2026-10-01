import { invoke } from "@tauri-apps/api/core";
import type { SessionMeta } from "./chat";

export interface WorkzProject {
  path: string;
  name: string;
  kind: "repo" | "dir";
  exists: boolean;
  last_opened: number;
}

export interface WorkzOverview {
  projects: WorkzProject[];
  default_work_dir: string;
  default_is_custom: boolean;
  free_dir: string;
}

export interface ProjectSessions {
  project_dir: string;
  sessions: SessionMeta[];
  error: string | null;
}

export function workzOverview(): Promise<WorkzOverview> {
  return invoke<WorkzOverview>("workz_overview");
}

export function workzProjectAdd(path: string): Promise<WorkzOverview> {
  return invoke<WorkzOverview>("workz_project_add", { path });
}

export function workzProjectRemove(path: string): Promise<WorkzOverview> {
  return invoke<WorkzOverview>("workz_project_remove", { path });
}

export function workzSetDefaultDir(path: string | null): Promise<WorkzOverview> {
  return invoke<WorkzOverview>("workz_set_default_dir", { path });
}

export function workzListAllSessions(
  projectDirs: string[],
  sources?: string[],
): Promise<ProjectSessions[]> {
  return invoke<ProjectSessions[]>("workz_list_all_sessions", {
    projectDirs,
    sources: sources && sources.length > 0 ? sources : null,
  });
}

export function chatSetSessionCwd(
  projectDir: string,
  sessionId: string,
  cwd: string | null,
): Promise<void> {
  return invoke<void>("chat_set_session_cwd", { projectDir, sessionId, cwd });
}
