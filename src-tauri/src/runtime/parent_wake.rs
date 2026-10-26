//! Parent/child todos for BotZ rooms.
//!
//! The kernel todo has no parent column. By convention the coordinator writes a
//! `Parent: <todo id>` line into each subtask's description. When every subtask
//! of a still-open parent is finished, patrol wakes the parent's owner once with
//! an `@!` mention asking for a summary and `complete_todo`.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use piscis_core::models::KoiTodo;

fn is_terminal(status: &str) -> bool {
    matches!(status, "done" | "completed" | "cancelled")
}

/// Parent reference (id or prefix) from a todo description, if any.
pub fn parent_ref(description: &str) -> Option<String> {
    for line in description.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("Parent:").or_else(|| line.strip_prefix("parent:")) else {
            continue;
        };
        let id: String = rest
            .trim()
            .trim_start_matches('`')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        if id.len() >= 6 {
            return Some(id);
        }
    }
    None
}

/// Resolve each todo's parent id within the same pool.
pub fn parent_map(todos: &[KoiTodo]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for t in todos {
        if let Some(r) = parent_ref(&t.description) {
            if let Some(p) = todos.iter().find(|p| p.id != t.id && p.id.starts_with(&r)) {
                out.insert(t.id.clone(), p.id.clone());
            }
        }
    }
    out
}

/// Open parents whose subtasks are all finished (at least one subtask).
pub fn finished_parents(todos: &[KoiTodo]) -> Vec<String> {
    let map = parent_map(todos);
    let mut children: HashMap<&str, Vec<&KoiTodo>> = HashMap::new();
    for t in todos {
        if let Some(p) = map.get(&t.id) {
            children.entry(p.as_str()).or_default().push(t);
        }
    }
    let mut out = Vec::new();
    for (pid, kids) in children {
        let Some(parent) = todos.iter().find(|t| t.id == pid) else { continue };
        if !is_terminal(&parent.status) && kids.iter().all(|k| is_terminal(&k.status)) {
            out.push(pid.to_string());
        }
    }
    out.sort();
    out
}

fn woken() -> &'static Mutex<HashSet<String>> {
    static W: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    W.get_or_init(|| Mutex::new(HashSet::new()))
}

/// True the first time `parent_id` is seen, so each parent is woken once.
pub fn mark_woken(parent_id: &str) -> bool {
    woken().lock().unwrap_or_else(|e| e.into_inner()).insert(parent_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todo(id: &str, status: &str, desc: &str) -> KoiTodo {
        let now = chrono::Utc::now();
        KoiTodo {
            id: id.into(),
            owner_id: "a".into(),
            title: id.into(),
            description: desc.into(),
            status: status.into(),
            priority: "medium".into(),
            assigned_by: "piscis".into(),
            pool_session_id: Some("p".into()),
            claimed_by: None,
            claimed_at: None,
            depends_on: None,
            blocked_reason: None,
            git_branch: None,
            integration_status: None,
            result_message_id: None,
            source_type: "user".into(),
            task_timeout_secs: 0,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn parses_parent_line() {
        assert_eq!(parent_ref("Do x\nParent: abcdef12\n"), Some("abcdef12".into()));
        assert_eq!(parent_ref("Parent: ab"), None);
        assert_eq!(parent_ref("no parent"), None);
    }

    #[test]
    fn wakes_parent_only_when_all_children_finished() {
        let mut todos = vec![
            todo("abcdef1234", "in_progress", ""),
            todo("c1c1c1c1", "done", "Parent: abcdef12"),
            todo("c2c2c2c2", "in_progress", "Parent: abcdef12"),
        ];
        assert!(finished_parents(&todos).is_empty());
        todos[2].status = "cancelled".into();
        assert_eq!(finished_parents(&todos), vec!["abcdef1234".to_string()]);
        todos[0].status = "done".into();
        assert!(finished_parents(&todos).is_empty());
    }

    #[test]
    fn marks_woken_once() {
        assert!(mark_woken("zz-unique-parent"));
        assert!(!mark_woken("zz-unique-parent"));
    }
}
