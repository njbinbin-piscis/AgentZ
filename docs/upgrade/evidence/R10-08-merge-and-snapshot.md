# R10-08 Evidence: Worktree Auto-Commit Merge & Non-Git Task Snapshot Rollback

- **Task ID**: `R10-08` (`r10-08-merge-and-snapshot`)
- **Date**: 2026-10-19
- **Status**: Completed

## 1. Changes Implemented

1. **Auto-Commit Uncommitted Worktree Changes on Merge (`crates/agentz-task-state/src/merge.rs`, `src-tauri/src/commands/agent_task.rs`, `src/services/tauri/agentTask.ts`, `src/workspaces/workz/AgentTaskReview.tsx`)**:
   - Added `commit_uncommitted_task_changes(project, task_id)` in `agentz-task-state` and `auto_commit_uncommitted` support in `agent_task_merge_preview`.
   - Enabled the merge button in `AgentTaskReview.tsx` when uncommitted files exist (`t("agent.mergeAutoCommit")`), staging and committing non-runtime changes on the task branch before running `merge-tree` preflight and executing the merge.
2. **Non-Git Folder Task Snapshot Rollback (`src-tauri/src/commands/agent_task.rs`, `src/services/tauri/agentTask.ts`, `src/workspaces/workz/index.tsx`)**:
   - Added `rollback_session_journal_changes` and Tauri command `agent_task_rollback_snapshot` to replay `FileJournal::undo_turn` across all turns of a task in reverse order, restoring modified files and removing newly created files even in non-Git folders.
   - Mounted `[恢复本任务快照]` (`[data-testid="workz-rollback-snapshot-btn"]`) in the `WorkZ` bottom bar when running without a Git worktree.

## 2. Verification Commands & Outputs

- `.\node_modules\.bin\vitest.cmd run src/workspaces/workz/AgentTaskReview.test.tsx`:
  - `18 passed (18)` (including auto-commit merge verification when untracked/dirty files exist).
- `cargo test --manifest-path crates/agentz-task-state/Cargo.toml merge::tests::preflight_rejects_dirty_wrong_target_stale_review_and_conflict_without_mutating_source` (verifies `commit_uncommitted_task_changes`).
