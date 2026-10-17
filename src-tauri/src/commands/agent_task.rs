//! Agent task isolation (M4) — run autonomous Agent-mode tasks inside an
//! isolated git worktree + branch, then review the resulting diff and either
//! merge it back, open a PR, or discard it. A separate checkout reduces
//! interference; it is not a filesystem/process security sandbox.
//!
//! Worktrees live under `<project>/../.agentz-worktrees/task-<id>` (a sibling of
//! the project, matching the kernel's `.koi-worktrees` convention) on a fresh
//! `workz/task-<id>` branch.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command;
use tokio::time::timeout;

use crate::commands::data_scope::require_project_dir;
use crate::task_environment::{self, TaskEnvironment};

/// Metadata describing one isolated agent task worktree.
#[derive(Debug, Clone, Serialize)]
pub struct AgentTaskInfo {
    /// Short task id (the suffix used in the branch / worktree names).
    pub id: String,
    /// Branch the worktree is checked out on (`workz/task-<id>`).
    pub branch: String,
    /// Absolute path to the worktree directory.
    pub worktree_path: String,
    /// Base branch the task forked from (for diff / merge).
    pub base: String,
    pub base_sha: String,
    pub environment_id: String,
}

fn task_info(record: &TaskEnvironment) -> AgentTaskInfo {
    AgentTaskInfo {
        id: record.task_id.clone(),
        branch: record.branch.clone().unwrap_or_default(),
        worktree_path: record.workspace_root.clone(),
        base: record.base_ref.clone(),
        base_sha: record.base_sha.clone(),
        environment_id: record.environment_id.clone(),
    }
}

#[tauri::command]
pub async fn agent_task_get(
    project_dir: Option<String>,
    task_id: String,
) -> Result<Option<AgentTaskInfo>, String> {
    let project = PathBuf::from(require_project_dir(project_dir.as_deref())?);
    let record = task_environment::load(&project, &task_id)?;
    if let Some(record) = &record {
        validate_task_environment(&project, record).await?;
    }
    Ok(record.as_ref().map(task_info))
}

pub async fn validate_task_environment(
    project: &Path,
    record: &TaskEnvironment,
) -> Result<(), String> {
    let workspace = PathBuf::from(&record.workspace_root);
    let workspace_root = task_environment::canonical_root(&workspace)?;
    let project_git = run_git(
        project,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await?;
    let workspace_git = run_git(
        &workspace,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await?;
    let branch = current_branch(&workspace).await?;
    let worktrees = run_git(project, &["worktree", "list", "--porcelain", "-z"]).await?;
    let snapshot = task_environment::ManagedSnapshot {
        workspace_root: workspace_root.clone(),
        project_git_dir: task_environment::canonical_root(Path::new(project_git.trim()))?,
        workspace_git_dir: task_environment::canonical_root(Path::new(workspace_git.trim()))?,
        registered: task_environment::registered_worktree(&worktrees, &workspace_root, &branch),
        branch,
    };
    task_environment::validate_managed(record, &snapshot)
}

pub async fn resolve_task_workspace(
    project: &Path,
    task_id: Option<&str>,
    requested: Option<&str>,
    require_binding: bool,
) -> Result<String, String> {
    let record = task_id
        .map(|id| task_environment::load(project, id))
        .transpose()?
        .flatten();
    if let Some(record) = record {
        validate_task_environment(project, &record).await?;
        let requested_root = requested
            .filter(|p| !p.trim().is_empty())
            .map(|root| task_environment::canonical_root(Path::new(root)))
            .transpose()?;
        task_environment::validate_requested_workspace(&record, requested_root.as_deref())?;
        return Ok(record.workspace_root);
    }
    if require_binding {
        return Err(
            "isolated task has no registered environment; explicit recovery/binding required"
                .into(),
        );
    }
    Ok(requested
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| project.to_string_lossy().to_string()))
}

/// A single file changed on the task branch relative to its base.
#[derive(Debug, Clone, Serialize)]
pub struct AgentTaskChange {
    pub path: String,
    /// One of: modified, added, deleted, renamed, copied, type_changed, unknown.
    pub status: String,
}

/// Side-by-side content for reviewing one changed file.
#[derive(Debug, Clone, Serialize)]
pub struct AgentTaskFileDiff {
    pub path: String,
    pub original: String,
    pub modified: String,
}

/// Result of attempting to open a pull request.
#[derive(Debug, Clone, Serialize)]
pub struct PrResult {
    pub ok: bool,
    /// PR URL when `gh` succeeded.
    pub url: Option<String>,
    /// Human-readable status / fallback instructions.
    pub message: String,
}

fn worktrees_root(project: &Path) -> PathBuf {
    project
        .parent()
        .unwrap_or(project)
        .join(".agentz-worktrees")
}

/// Save a recoverable source snapshot; this command never removes the task.
#[tauri::command]
pub async fn agent_task_archive_snapshot(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: Option<String>,
    task_id: String,
) -> Result<agentz_task_state::archive::Archive, String> {
    let _lease = state.environment_access.execution().await;
    let project = PathBuf::from(require_project_dir(project_dir.as_deref())?);
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    tokio::task::spawn_blocking(move || {
        let _worker_lease = _lease;
        agentz_task_state::archive::create(&project, &task_id, &id, &created_at)
    })
    .await
    .map_err(|e| format!("archive worker failed: {e}"))?
}

#[derive(Debug, Serialize)]
pub struct ArchiveRestoreResult {
    pub archive: agentz_task_state::archive::Archive,
    pub workspace_root: String,
}

/// Restore to a fresh host-owned checkout, without retargeting the original session.
#[tauri::command]
pub async fn agent_task_restore_snapshot(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: Option<String>,
    archive_id: String,
) -> Result<ArchiveRestoreResult, String> {
    let _lease = state.environment_access.execution().await;
    let project = PathBuf::from(require_project_dir(project_dir.as_deref())?);
    tokio::task::spawn_blocking(move || {
        let _worker_lease = _lease;
        agentz_task_state::archive::read(&project, &archive_id)?;
        let root = worktrees_root(&project);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let destination = root.join(format!("restored-{}", uuid::Uuid::new_v4()));
        let archive = agentz_task_state::archive::restore(&project, &archive_id, &destination)?;
        Ok(ArchiveRestoreResult {
            archive,
            workspace_root: task_environment::canonical_root(&destination)?,
        })
    })
    .await
    .map_err(|e| format!("restore worker failed: {e}"))?
}

fn branch_for(task_id: &str) -> String {
    let safe: String = task_id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("workz/task-{safe}")
}

async fn run_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = timeout(
        Duration::from_secs(60),
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "git command timed out".to_string())?
    .map_err(|e| format!("git command failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git error: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

async fn current_branch(project: &Path) -> Result<String, String> {
    let out = run_git(project, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
    Ok(out.trim().to_string())
}

fn status_char_to_string(c: char) -> String {
    match c {
        'M' => "modified",
        'A' => "added",
        'D' => "deleted",
        'R' => "renamed",
        'C' => "copied",
        'T' => "type_changed",
        _ => "unknown",
    }
    .to_string()
}

/// Create (or reuse) an isolated worktree + branch for an agent task.
#[tauri::command]
pub async fn agent_task_create(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: Option<String>,
    task_id: String,
    base: Option<String>,
    start_mode: Option<String>,
    allow_local_exclusion: Option<bool>,
) -> Result<AgentTaskInfo, String> {
    let lease = state.environment_access.execution().await;
    let project = require_project_dir(project_dir.as_deref())?;
    let project = PathBuf::from(project);
    if task_id.is_empty()
        || task_id
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '-' && c != '_')
    {
        return Err("task identity must contain only letters, digits, '-' or '_'".into());
    }
    if let Some(record) = task_environment::load(&project, &task_id)? {
        validate_task_environment(&project, &record).await?;
        return Ok(task_info(&record));
    }
    let id_safe = task_id;
    let branch = branch_for(&id_safe);
    let wt_dir = worktrees_root(&project).join(format!("task-{id_safe}"));
    let source = project.clone();
    let target = wt_dir.clone();
    let start_branch = branch.clone();
    let nonce = uuid::Uuid::new_v4().to_string();
    let mode = start_mode.unwrap_or_else(|| "commit".into());
    let start = tokio::task::spawn_blocking(move || {
        let _worker_lease = lease;
        agentz_task_state::start::create(
            &source,
            &target,
            &start_branch,
            agentz_task_state::start::Options {
                base: base.as_deref(),
                mode: &mode,
                allow_local_exclusion: allow_local_exclusion.unwrap_or(false),
                nonce: &nonce,
            },
        )
    })
    .await
    .map_err(|e| format!("task creation worker failed: {e}"))??;
    let base = start.base_ref;
    let base_sha = start.base_sha;
    let record = TaskEnvironment {
        schema_version: 1,
        task_id: id_safe,
        environment_id: uuid::Uuid::new_v4().to_string(),
        project_root: task_environment::canonical_root(&project)?,
        workspace_root: task_environment::canonical_root(&wt_dir)?,
        kind: "managed-worktree".into(),
        branch: Some(branch),
        base_ref: base,
        base_sha,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    task_environment::register(&project, &record).map(|saved| task_info(&saved))
}

/// List existing AgentZ agent task worktrees.
#[tauri::command]
pub async fn agent_task_list(project_dir: Option<String>) -> Result<Vec<AgentTaskInfo>, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    let project = PathBuf::from(project);
    if !project.join(".git").exists() {
        return Ok(vec![]);
    }
    let out = run_git(&project, &["worktree", "list", "--porcelain"])
        .await
        .unwrap_or_default();

    let mut tasks = Vec::new();
    let mut cur_path: Option<String> = None;
    for line in out.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            cur_path = Some(p.trim().to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            let branch = b.trim().trim_start_matches("refs/heads/").to_string();
            if branch.starts_with("workz/task-") {
                if let Some(path) = cur_path.clone() {
                    let id = branch.trim_start_matches("workz/task-").to_string();
                    if let Some(record) = task_environment::load(&project, &id)? {
                        if task_environment::canonical_root(Path::new(&path))?
                            != record.workspace_root
                        {
                            continue;
                        }
                    }
                    tasks.push(AgentTaskInfo {
                        id,
                        branch,
                        worktree_path: path,
                        base: String::new(),
                        base_sha: String::new(),
                        environment_id: String::new(),
                    });
                }
            }
        } else if line.is_empty() {
            cur_path = None;
        }
    }
    Ok(tasks)
}

/// List files changed on `branch` relative to `base`.
/// Complete review API: caller supplies identity, never a replacement root/base.
#[tauri::command]
pub async fn agent_task_change_set(
    project_dir: Option<String>,
    task_id: String,
) -> Result<agentz_task_state::changes::ChangeSet, agentz_task_state::changes::ChangeError> {
    use agentz_task_state::changes::ChangeError;
    let project = PathBuf::from(
        require_project_dir(project_dir.as_deref()).map_err(ChangeError::environment)?,
    );
    let record = task_environment::load(&project, &task_id)
        .map_err(ChangeError::environment)?
        .ok_or_else(|| {
            ChangeError::environment(
                "task environment missing; explicit binding required".to_string(),
            )
        })?;
    validate_task_environment(&project, &record)
        .await
        .map_err(ChangeError::environment)?;
    tokio::task::spawn_blocking(move || {
        agentz_task_state::changes::collect(Path::new(&record.workspace_root), &record.base_sha)
    })
    .await
    .map_err(|e| ChangeError::worker(format!("change collection failed: {e}")))?
}

/// Legacy branch-only review; replaced by the complete task API in AZ-013.
#[tauri::command]
pub async fn agent_task_file_view(
    project_dir: Option<String>,
    task_id: String,
    path: String,
) -> Result<agentz_task_state::changes::FileView, agentz_task_state::changes::ChangeError> {
    use agentz_task_state::changes::ChangeError;
    let project = PathBuf::from(
        require_project_dir(project_dir.as_deref()).map_err(ChangeError::environment)?,
    );
    let record = task_environment::load(&project, &task_id)
        .map_err(ChangeError::environment)?
        .ok_or_else(|| {
            ChangeError::environment("task environment missing; explicit binding required".into())
        })?;
    validate_task_environment(&project, &record)
        .await
        .map_err(ChangeError::environment)?;
    tokio::task::spawn_blocking(move || {
        agentz_task_state::changes::file_view(
            Path::new(&record.workspace_root),
            &record.base_sha,
            &path,
        )
    })
    .await
    .map_err(|e| ChangeError::worker(e.to_string()))?
}

/// Legacy branch-only review.
#[tauri::command]
pub async fn agent_task_changed_files(
    project_dir: Option<String>,
    branch: String,
    base: String,
) -> Result<Vec<AgentTaskChange>, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    let project = PathBuf::from(project);
    let range = format!("{base}...{branch}");
    let out = run_git(&project, &["diff", "--name-status", &range])
        .await
        .unwrap_or_default();

    let mut changes = Vec::new();
    for line in out.lines() {
        let mut parts = line.split('\t');
        let Some(status) = parts.next() else { continue };
        let Some(path) = parts.next_back() else {
            continue;
        };
        let c = status.chars().next().unwrap_or('?');
        changes.push(AgentTaskChange {
            path: path.to_string(),
            status: status_char_to_string(c),
        });
    }
    Ok(changes)
}

/// Side-by-side content for one changed file (base vs branch).
#[tauri::command]
pub async fn agent_task_file_diff(
    project_dir: Option<String>,
    branch: String,
    base: String,
    path: String,
) -> Result<AgentTaskFileDiff, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    let project = PathBuf::from(project);
    let original = run_git(&project, &["show", &format!("{base}:{path}")])
        .await
        .unwrap_or_default();
    let modified = run_git(&project, &["show", &format!("{branch}:{path}")])
        .await
        .unwrap_or_default();
    Ok(AgentTaskFileDiff {
        path,
        original,
        modified,
    })
}

async fn merge_repository_key(
    project: &Path,
) -> Result<String, agentz_task_state::merge::MergeError> {
    use agentz_task_state::merge::MergeError;
    let directory = run_git(
        project,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .await
    .map_err(|e| MergeError::new("identity", e))?;
    task_environment::canonical_root(Path::new(directory.trim()))
        .map_err(|e| MergeError::new("identity", e))
}

/// Preview the exact reviewed task HEAD without changing any checkout.
#[tauri::command]
pub async fn agent_task_merge_preview(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: Option<String>,
    task_id: String,
    base: String,
    expected_task_head: String,
) -> Result<agentz_task_state::merge::Preview, agentz_task_state::merge::MergeError> {
    use agentz_task_state::merge::MergeError;
    let project = PathBuf::from(
        require_project_dir(project_dir.as_deref()).map_err(|e| MergeError::new("identity", e))?,
    );
    let key = merge_repository_key(&project).await?;
    let repository_lease = state
        .repository_access
        .transaction(&key)
        .await
        .map_err(|e| MergeError::new("repository_lock", e))?;
    let lease = state.environment_access.execution().await;
    tokio::task::spawn_blocking(move || {
        let _repository_lease = repository_lease;
        let _worker_lease = lease;
        let preview =
            agentz_task_state::merge::preview(&project, &task_id, &base, &expected_task_head)?;
        if preview.repository != key {
            return Err(MergeError::new(
                "repository_changed",
                "Git repository changed while queued",
            ));
        }
        Ok(preview)
    })
    .await
    .map_err(|e| MergeError::new("worker", e.to_string()))?
}

/// Versioned merge transaction. Missing legacy identity/version parameters fail closed.
#[tauri::command]
pub async fn agent_task_merge(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: Option<String>,
    task_id: Option<String>,
    base: String,
    expected_task_head: Option<String>,
    expected_target_head: Option<String>,
) -> Result<agentz_task_state::merge::Receipt, agentz_task_state::merge::MergeError> {
    use agentz_task_state::merge::MergeError;
    let required = |value: Option<String>| {
        value.filter(|s| !s.is_empty()).ok_or_else(|| {
            MergeError::new(
                "identity",
                "stable task identity and reviewed source/target versions are required",
            )
        })
    };
    let task_id = required(task_id)?;
    let task_head = required(expected_task_head)?;
    let target_head = required(expected_target_head)?;
    let project = PathBuf::from(
        require_project_dir(project_dir.as_deref()).map_err(|e| MergeError::new("identity", e))?,
    );
    let key = merge_repository_key(&project).await?;
    let initial = agentz_task_state::merge::Preview {
        task_id,
        task_head,
        target_branch: base,
        target_head,
        merged_tree: String::new(),
        repository: key,
    };
    let repository_lease = state
        .repository_access
        .transaction(&initial.repository)
        .await
        .map_err(|e| MergeError::new("repository_lock", e))?;
    let execution_lease = state
        .environment_access
        .cleanup()
        .map_err(|e| MergeError::new("execution_busy", e))?;
    let id = uuid::Uuid::new_v4().to_string();
    let at = chrono::Utc::now().to_rfc3339();
    // IPC cancellation only drops the waiter. This independent worker retains both
    // guards until Git/receipt/recovery processing finishes, including blocking stages.
    tokio::spawn(async move {
        let _repository_lease = repository_lease;
        let _execution_lease = execution_lease;
        agentz_task_state::merge::execute(&project, &initial, &id, &at).await
    })
    .await
    .map_err(|e| MergeError::new("worker", e.to_string()))?
}

/// Archive the complete task checkout while retaining its branch and runtime files.
#[tauri::command]
pub async fn agent_task_discard(
    state: tauri::State<'_, crate::state::AppState>,
    project_dir: Option<String>,
    worktree_path: String,
    branch: String,
    task_id: Option<String>,
) -> Result<agentz_task_state::cleanup::Receipt, agentz_task_state::cleanup::CleanupError> {
    use agentz_task_state::cleanup::CleanupError;
    let error = |stage, message| CleanupError {
        stage,
        message,
        archive_id: None,
        retained_root: None,
    };
    let task_id = task_id.filter(|id| !id.is_empty()).ok_or_else(|| {
        error(
            "identity",
            "stable task identity required for archival".into(),
        )
    })?;
    let _lease = state
        .environment_access
        .cleanup()
        .map_err(|e| error("execution_busy", e))?;
    let project = require_project_dir(project_dir.as_deref()).map_err(|e| error("identity", e))?;
    let project = PathBuf::from(project);
    let id = uuid::Uuid::new_v4().to_string();
    let at = chrono::Utc::now().to_rfc3339();
    tokio::task::spawn_blocking(move || {
        let _worker_lease = _lease;
        agentz_task_state::cleanup::retain_task(
            &project,
            &task_id,
            &worktree_path,
            &branch,
            &id,
            &at,
        )
    })
    .await
    .map_err(|e| error("worker", e.to_string()))?
}

/// Open a pull request for the task branch via the GitHub `gh` CLI.
/// Falls back to a copy-the-branch message when `gh` is unavailable.
#[tauri::command]
pub async fn agent_task_open_pr(
    project_dir: Option<String>,
    branch: String,
    base: String,
    title: String,
    body: Option<String>,
) -> Result<PrResult, String> {
    let project = require_project_dir(project_dir.as_deref())?;
    let project = PathBuf::from(project);

    // gh present?
    let gh_ok = Command::new("gh")
        .arg("--version")
        .current_dir(&project)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !gh_ok {
        return Ok(PrResult {
            ok: false,
            url: None,
            message: format!(
                "GitHub CLI (`gh`) not found. Push branch `{branch}` and open a PR manually."
            ),
        });
    }

    // Push the branch first (best effort — origin may not exist).
    let _ = run_git(&project, &["push", "-u", "origin", &branch]).await;

    let body = body.unwrap_or_default();
    let output = Command::new("gh")
        .args([
            "pr", "create", "--base", &base, "--head", &branch, "--title", &title, "--body", &body,
        ])
        .current_dir(&project)
        .output()
        .await
        .map_err(|e| format!("failed to run gh: {e}"))?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(PrResult {
            ok: true,
            url: Some(url.clone()),
            message: format!("Pull request created: {url}"),
        })
    } else {
        Ok(PrResult {
            ok: false,
            url: None,
            message: format!(
                "gh pr create failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        })
    }
}
