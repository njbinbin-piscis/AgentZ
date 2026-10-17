//! Worktree integration helpers — dependency gating and merge backlog signals.
//!
//! These are pure functions over todo snapshots so hosts can assess integration
//! pressure without shelling out to git.

use crate::models::KoiTodo;

pub const INTEGRATION_NONE: &str = "none";
pub const INTEGRATION_READY: &str = "ready";
pub const INTEGRATION_MERGED: &str = "merged";
pub const INTEGRATION_CONFLICT: &str = "conflict";

/// Resolve a depends_on hint (full id or unique prefix) to a todo row.
pub fn resolve_todo_dependency<'a>(dep_hint: &str, todos: &'a [KoiTodo]) -> Option<&'a KoiTodo> {
    let hint = dep_hint.trim();
    if hint.is_empty() {
        return None;
    }
    if let Some(exact) = todos.iter().find(|t| t.id == hint) {
        return Some(exact);
    }
    let matches: Vec<_> = todos
        .iter()
        .filter(|t| t.id.starts_with(hint) || hint.starts_with(&t.id[..8.min(t.id.len())]))
        .collect();
    if matches.len() == 1 {
        Some(matches[0])
    } else {
        None
    }
}

fn dependency_produces_branch(dep: &KoiTodo) -> bool {
    dep.git_branch
        .as_deref()
        .map(str::trim)
        .is_some_and(|b| !b.is_empty())
}

fn integration_is_merged(todo: &KoiTodo) -> bool {
    todo.integration_status
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| s.eq_ignore_ascii_case(INTEGRATION_MERGED))
}

/// True when a todo may start (assign / activate / execute).
pub fn is_todo_dependency_satisfied(todo: &KoiTodo, todos: &[KoiTodo]) -> bool {
    let Some(dep_hint) = todo
        .depends_on
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return true;
    };
    let Some(dep) = resolve_todo_dependency(dep_hint, todos) else {
        return false;
    };
    if dep.status != "done" {
        return false;
    }
    if dependency_produces_branch(dep) && !integration_is_merged(dep) {
        return false;
    }
    true
}

/// Branches completed on the board but not yet merged into main.
pub fn count_integration_ready(todos: &[KoiTodo]) -> usize {
    todos
        .iter()
        .filter(|t| {
            t.git_branch
                .as_deref()
                .map(str::trim)
                .is_some_and(|b| !b.is_empty())
                && matches!(t.status.as_str(), "done" | "needs_review")
                && !integration_is_merged(t)
        })
        .count()
}

/// Todos blocked on an unmerged upstream branch.
pub fn count_dependency_blocked(todos: &[KoiTodo]) -> usize {
    todos
        .iter()
        .filter(|t| {
            t.status == "todo"
                && t.claimed_by.is_none()
                && t.depends_on
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty())
                && !is_todo_dependency_satisfied(t, todos)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn todo(
        id: &str,
        status: &str,
        depends_on: Option<&str>,
        branch: Option<&str>,
        integration: Option<&str>,
    ) -> KoiTodo {
        KoiTodo {
            id: id.into(),
            owner_id: "k1".into(),
            title: "t".into(),
            description: String::new(),
            status: status.into(),
            priority: "medium".into(),
            assigned_by: "piscis".into(),
            pool_session_id: Some("pool-1".into()),
            claimed_by: None,
            claimed_at: None,
            depends_on: depends_on.map(String::from),
            blocked_reason: None,
            git_branch: branch.map(String::from),
            integration_status: integration.map(String::from),
            result_message_id: None,
            source_type: "koi".into(),
            task_timeout_secs: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn dependency_requires_upstream_merge_when_branch_exists() {
        let todos = vec![
            todo(
                "aaa-1111",
                "done",
                None,
                Some("koi/a-aaa1111"),
                Some(INTEGRATION_READY),
            ),
            todo("bbb-2222", "todo", Some("aaa-1111"), None, None),
        ];
        assert!(!is_todo_dependency_satisfied(&todos[1], &todos));

        let merged = vec![
            todo(
                "aaa-1111",
                "done",
                None,
                Some("koi/a-aaa1111"),
                Some(INTEGRATION_MERGED),
            ),
            todo("bbb-2222", "todo", Some("aaa-1111"), None, None),
        ];
        assert!(is_todo_dependency_satisfied(&merged[1], &merged));
    }

    #[test]
    fn integration_ready_counts_unmerged_done_branches() {
        let todos = vec![todo(
            "aaa-1111",
            "done",
            None,
            Some("koi/a-aaa1111"),
            Some(INTEGRATION_READY),
        )];
        assert_eq!(count_integration_ready(&todos), 1);
    }
}
