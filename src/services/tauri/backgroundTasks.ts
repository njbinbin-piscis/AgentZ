import { invoke } from "@tauri-apps/api/core";

/** Snapshot of one registered background task (mirrors kernel BgSnapshot). */
export interface BackgroundTask {
  id: string;
  session_id: string;
  workspace_root: string;
  label: string;
  cwd: string;
  pid: number | null;
  status: "running" | "exited" | "killed" | "failed" | string;
  exit_code: number | null;
  error: string | null;
  ready: boolean;
  started_unix_ms: number;
  finished_unix_ms: number | null;
  elapsed_ms: number;
  stdout_total: number;
  stderr_total: number;
}

export interface BackgroundOutput {
  task: BackgroundTask;
  stdout: string;
  stderr: string;
  next_cursor_out: number;
  next_cursor_err: number;
  truncated: boolean;
}

/** Payload of the `background_task` channel on `agentz:chat-event`. */
export interface BackgroundTaskEvent {
  kind: "ready" | "terminal";
  task: BackgroundTask;
}

export function listBackgroundTasks(
  projectDir?: string | null,
  sessionId?: string | null,
): Promise<BackgroundTask[]> {
  return invoke<BackgroundTask[]>("background_list", {
    projectDir: projectDir ?? null,
    sessionId: sessionId ?? null,
  });
}

export function getBackgroundOutput(
  bgId: string,
  cursorOut = 0,
  cursorErr = 0,
): Promise<BackgroundOutput> {
  return invoke<BackgroundOutput>("background_output", {
    bgId,
    cursorOut,
    cursorErr,
  });
}

export function killBackgroundTask(bgId: string): Promise<boolean> {
  return invoke<boolean>("background_kill", { bgId });
}

export function backgroundRunningCount(projectDir?: string | null): Promise<number> {
  return invoke<number>("background_running_count", {
    projectDir: projectDir ?? null,
  });
}
