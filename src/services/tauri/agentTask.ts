/**
 * Tauri IPC — Agent task isolation (M4).
 *
 * Wraps `src-tauri/src/commands/agent_task.rs`. Agent-mode tasks can run inside
 * an isolated git worktree + branch; the resulting diff is reviewed and then
 * merged back, opened as a PR, or discarded. Worktree isolation is separate
 * from filesystem, process and network permission enforcement.
 */
import { invoke } from "@tauri-apps/api/core";

export interface AgentTaskInfo {
  id: string;
  branch: string;
  worktree_path: string;
  base: string;
  base_sha: string;
  environment_id: string;
}

export interface AgentTaskChange {
  path: string;
  status: string;
}

export interface AgentTaskFileDiff {
  path: string;
  original: string;
  modified: string;
}

export interface TaskChangeSet {
  head_sha: string;
  base_sha: string;
  excluded_runtime_paths: string[];
  has_uncommitted: boolean;
  changes: Array<{
    path: string;
    old_path: string | null;
    status: string;
    binary: boolean;
    sources: string[];
  }>;
}

export interface TaskFileView {
  path: string;
  original: string | null;
  modified: string | null;
  binary: boolean;
  review_version: string;
}

export interface PrResult {
  ok: boolean;
  url: string | null;
  message: string;
}

export interface TaskArchive {
  schema_version: number;
  archive_id: string;
  environment: {
    task_id: string;
    environment_id: string;
    project_root: string;
    workspace_root: string;
  };
  head_sha: string;
  tree_sha: string;
  snapshot_commit: string;
  snapshot_ref: string;
  omitted_paths: string[];
  created_at: string;
}

export interface TaskCleanupReceipt {
  archive_id: string;
  task_id: string;
  retained_root: string;
  state: string;
  branch_action: string;
}

export interface TaskMergePreview {
  task_head: string;
  target_head: string;
  target_branch: string;
}
export interface TaskMergeReceipt {
  transaction_id: string;
  after_head: string | null;
  state: string;
  recovery_ref: string;
}

export const agentTaskApi = {
  archiveSnapshot: (projectDir: string, taskId: string) =>
    invoke<TaskArchive>("agent_task_archive_snapshot", { projectDir, taskId }),
  restoreSnapshot: (projectDir: string, archiveId: string) =>
    invoke<{ archive: TaskArchive; workspace_root: string }>("agent_task_restore_snapshot", {
      projectDir,
      archiveId,
    }),
  fileView: (projectDir: string, taskId: string, path: string) =>
    invoke<TaskFileView>("agent_task_file_view", { projectDir, taskId, path }),
  changeSet: (projectDir: string, taskId: string) =>
    invoke<TaskChangeSet>("agent_task_change_set", { projectDir, taskId }),
  create: (
    projectDir: string,
    taskId: string,
    base?: string,
    startMode?: "commit" | "working",
    allowLocalExclusion?: boolean,
  ) =>
    invoke<AgentTaskInfo>("agent_task_create", {
      projectDir,
      taskId,
      base,
      startMode,
      allowLocalExclusion,
    }),

  list: (projectDir: string) => invoke<AgentTaskInfo[]>("agent_task_list", { projectDir }),

  get: (projectDir: string, taskId: string) =>
    invoke<AgentTaskInfo | null>("agent_task_get", { projectDir, taskId }),

  changedFiles: (projectDir: string, branch: string, base: string) =>
    invoke<AgentTaskChange[]>("agent_task_changed_files", {
      projectDir,
      branch,
      base,
    }),

  fileDiff: (projectDir: string, branch: string, base: string, path: string) =>
    invoke<AgentTaskFileDiff>("agent_task_file_diff", {
      projectDir,
      branch,
      base,
      path,
    }),

  mergePreview: (
    projectDir: string,
    taskId: string,
    base: string,
    expectedTaskHead: string,
    autoCommitUncommitted?: boolean,
  ) =>
    invoke<TaskMergePreview>("agent_task_merge_preview", {
      projectDir,
      taskId,
      base,
      expectedTaskHead,
      autoCommitUncommitted: autoCommitUncommitted ?? false,
    }),
  rollbackSnapshot: (projectDir: string, taskId: string) =>
    invoke<string[]>("agent_task_rollback_snapshot", {
      projectDir,
      taskId,
    }),
  merge: (
    projectDir: string,
    taskId: string,
    base: string,
    expectedTaskHead: string,
    expectedTargetHead: string,
  ) =>
    invoke<TaskMergeReceipt>("agent_task_merge", {
      projectDir,
      taskId,
      base,
      expectedTaskHead,
      expectedTargetHead,
    }),

  discard: (projectDir: string, worktreePath: string, branch: string, taskId: string) =>
    invoke<TaskCleanupReceipt>("agent_task_discard", { projectDir, worktreePath, branch, taskId }),

  openPr: (projectDir: string, branch: string, base: string, title: string, body?: string) =>
    invoke<PrResult>("agent_task_open_pr", {
      projectDir,
      branch,
      base,
      title,
      body,
    }),
};
