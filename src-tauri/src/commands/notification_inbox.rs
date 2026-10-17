//! Project-scoped notification reads. Only trusted host event sources may post.
use crate::state::AppState;
use agentz_task_state::{
    approval::{ApprovalStore, Status},
    inbox::{InboxStore, Notice},
};
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{State, WebviewWindow};

#[derive(Serialize)]
pub struct InboxPage {
    pub notices: Vec<Notice>,
    pub unread_count: u64,
}

#[tauri::command]
pub async fn notification_inbox_list(
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    limit: usize,
) -> Result<InboxPage, String> {
    if !(1..=100).contains(&limit) {
        return Err("inbox limit must be between 1 and 100".into());
    }
    let _lease = state.environment_access.execution().await;
    let boot_id = state.approval_broker.boot_id().to_string();
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |scope| {
        if !scope.project().join(".agentz/inbox.db").exists() {
            return Ok(InboxPage {
                notices: Vec::new(),
                unread_count: 0,
            });
        }
        let mut store = InboxStore::open(scope.project())?;
        if let Ok(now_ms) = clock_ms() {
            if let Err(error) =
                reconcile_approval_notices(scope.project(), &boot_id, now_ms, &mut store)
            {
                tracing::warn!(%error, "approval inbox reconciliation unavailable");
            }
        }
        Ok(InboxPage {
            notices: store.recent(limit)?,
            unread_count: store.unread_count()?,
        })
    })
}

#[tauri::command]
pub async fn notification_inbox_mark_read(
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    event_key: String,
) -> Result<bool, String> {
    if event_key.is_empty() || event_key.len() > 512 {
        return Err("invalid notification event key".into());
    }
    let read_ms = clock_ms()?;
    let _lease = state.environment_access.execution().await;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |scope| {
        if !scope.project().join(".agentz/inbox.db").exists() {
            return Ok(false);
        }
        InboxStore::open(scope.project())?.mark_read(&event_key, read_ms)
    })
}

fn clock_ms() -> Result<i64, String> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid notification clock")?
            .as_millis(),
    )
    .map_err(|_| "notification clock overflow".into())
}

fn reconcile_approval_notices(
    project: &Path,
    boot_id: &str,
    now_ms: i64,
    inbox: &mut InboxStore,
) -> Result<(), String> {
    let path = project.join(".agentz/approvals.db");
    if !path.exists() {
        return Ok(());
    }
    // Opening with this boot expires pending intents from a previous process.
    // It never makes an old approval executable again.
    let approvals = ApprovalStore::open_existing(&path, boot_id, now_ms)?;
    for (event_key, task_id) in inbox.unread_approval_candidates(1000)? {
        let Some(request_id) = event_key.strip_prefix("approval:") else {
            continue;
        };
        let Some((intent, status)) = approvals.get(request_id)? else {
            continue;
        };
        if intent.binding.task_id == task_id
            && (status != Status::Pending || intent.expires_ms <= now_ms)
        {
            inbox.mark_read(&event_key, now_ms)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_task_state::approval::Binding;
    use agentz_task_state::inbox::NoticeKind;

    #[test]
    fn expired_and_previous_boot_approval_notices_stop_requesting_action() {
        let project =
            std::env::temp_dir().join(format!("agentz-inbox-reconcile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        let path = project.join(".agentz/approvals.db");
        let mut approvals = ApprovalStore::open(&path, "boot-a", 100).unwrap();
        let binding = Binding {
            project_id: "project".into(),
            task_id: "task".into(),
            environment_id: "environment".into(),
            binding_version: "binding".into(),
            run_id: "run".into(),
            attempt_id: "attempt".into(),
            capability_version: "capability".into(),
        };
        for request in ["approval:live", "approval:old", "approval:expired"] {
            approvals
                .issue(
                    request,
                    binding.clone(),
                    "tool",
                    &serde_json::json!({"target":"local"}),
                    &serde_json::json!({}),
                    100,
                    if request == "approval:expired" {
                        50
                    } else {
                        1000
                    },
                )
                .unwrap();
        }
        let mut inbox = InboxStore::open(&project).unwrap();
        for request in ["approval:live", "approval:old", "approval:expired"] {
            inbox
                .post(&Notice {
                    event_key: format!("approval:{request}"),
                    task_id: "task".into(),
                    kind: NoticeKind::ActionRequired,
                    summary: "Approval required".into(),
                    created_ms: 100,
                    read_ms: None,
                })
                .unwrap();
        }
        for index in 0..101 {
            inbox
                .post(&Notice {
                    event_key: format!("other:{index}"),
                    task_id: "task".into(),
                    kind: NoticeKind::MeaningfulChange,
                    summary: "Other event".into(),
                    created_ms: 101 + index,
                    read_ms: None,
                })
                .unwrap();
        }
        reconcile_approval_notices(&project, "boot-a", 200, &mut inbox).unwrap();
        assert_eq!(inbox.unread_count().unwrap(), 103);
        approvals
            .decide(
                &approvals.get("approval:live").unwrap().unwrap().0,
                false,
                300,
            )
            .unwrap();
        reconcile_approval_notices(&project, "boot-a", 300, &mut inbox).unwrap();
        assert_eq!(inbox.unread_count().unwrap(), 102);
        drop(approvals);
        reconcile_approval_notices(&project, "boot-b", 400, &mut inbox).unwrap();
        assert_eq!(inbox.unread_count().unwrap(), 101);
        drop(inbox);
        std::fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn approval_reconciliation_eventually_reaches_a_stale_notice_past_live_page() {
        let project =
            std::env::temp_dir().join(format!("agentz-inbox-reconcile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        let path = project.join(".agentz/approvals.db");
        let mut approvals = ApprovalStore::open(&path, "boot-a", 100).unwrap();
        approvals
            .issue(
                "approval:zzzz",
                Binding {
                    project_id: "project".into(),
                    task_id: "task".into(),
                    environment_id: "environment".into(),
                    binding_version: "binding".into(),
                    run_id: "run".into(),
                    attempt_id: "attempt".into(),
                    capability_version: "capability".into(),
                },
                "tool",
                &serde_json::json!({"target":"local"}),
                &serde_json::json!({}),
                100,
                150,
            )
            .unwrap();
        let mut inbox = InboxStore::open(&project).unwrap();
        for index in 0..1000 {
            inbox
                .post(&Notice {
                    event_key: format!("approval:approval:{index:04}"),
                    task_id: "task".into(),
                    kind: NoticeKind::ActionRequired,
                    summary: "Approval required".into(),
                    created_ms: 100,
                    read_ms: None,
                })
                .unwrap();
        }
        inbox
            .post(&Notice {
                event_key: "approval:approval:zzzz".into(),
                task_id: "task".into(),
                kind: NoticeKind::ActionRequired,
                summary: "Approval required".into(),
                created_ms: 100,
                read_ms: None,
            })
            .unwrap();
        reconcile_approval_notices(&project, "boot-a", 300, &mut inbox).unwrap();
        assert_eq!(inbox.unread_count().unwrap(), 1001);
        drop(inbox);
        let mut reopened = InboxStore::open(&project).unwrap();
        reconcile_approval_notices(&project, "boot-a", 300, &mut reopened).unwrap();
        assert_eq!(reopened.unread_count().unwrap(), 1000);
        drop(reopened);
        drop(approvals);
        std::fs::remove_dir_all(project).unwrap();
    }
}
