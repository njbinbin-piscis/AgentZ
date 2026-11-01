//! Project-scoped notification reads. Only trusted host event sources may post.
use crate::commands::session::session_notice_preview;
use crate::state::AppState;
use agentz_task_state::{
    approval::{ApprovalStore, Status},
    inbox::{InboxStore, Notice, NoticeKind},
};
use piscis_kernel::store::db::Database;
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{State, WebviewWindow};

#[derive(Serialize)]
pub struct InboxNoticeView {
    pub event_key: String,
    pub task_id: String,
    pub kind: NoticeKind,
    pub summary: String,
    pub created_ms: i64,
    pub read_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reply: Option<String>,
}

#[derive(Serialize)]
pub struct InboxPage {
    pub notices: Vec<InboxNoticeView>,
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
        let notices = store.recent(limit)?;
        let unread_count = store.unread_count()?;
        Ok(InboxPage {
            notices: enrich_notices(scope.project(), notices),
            unread_count,
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

fn enrich_notices(project: &Path, notices: Vec<Notice>) -> Vec<InboxNoticeView> {
    let db_path = project.join(".agentz").join("piscis.db");
    let db = if db_path.is_file() {
        match Database::open(&db_path) {
            Ok(db) => Some(db),
            Err(error) => {
                tracing::warn!(%error, "notification session lookup unavailable");
                None
            }
        }
    } else {
        None
    };
    notices
        .into_iter()
        .map(|notice| {
            let preview = db
                .as_ref()
                .and_then(|db| session_notice_preview(db, &notice.task_id));
            InboxNoticeView {
                event_key: notice.event_key,
                task_id: notice.task_id,
                kind: notice.kind,
                summary: notice.summary,
                created_ms: notice.created_ms,
                read_ms: notice.read_ms,
                session_title: preview.as_ref().and_then(|preview| preview.title.clone()),
                last_reply: preview.and_then(|preview| preview.last_reply),
            }
        })
        .collect()
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

    #[test]
    fn listed_notices_include_the_session_title_and_latest_reply() {
        let project =
            std::env::temp_dir().join(format!("agentz-inbox-preview-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(project.join(".agentz")).unwrap();
        let db = piscis_kernel::store::db::Database::open(&project.join(".agentz/piscis.db")).unwrap();
        let session = db
            .create_session_with_source(Some("修通知"), "codez")
            .unwrap();
        db.append_message(&session.id, "user", "通知看不出是哪个对话")
            .unwrap();
        db.append_message(&session.id, "assistant", "先改列表")
            .unwrap();
        db.append_message(&session.id, "assistant", "已带上标题和最后回复")
            .unwrap();
        drop(db);
        let views = enrich_notices(
            &project,
            vec![
                Notice {
                    event_key: "run-terminal:done".into(),
                    task_id: session.id.clone(),
                    kind: NoticeKind::Completed,
                    summary: "Task completed".into(),
                    created_ms: 10,
                    read_ms: None,
                },
                Notice {
                    event_key: "run-terminal:missing".into(),
                    task_id: "missing-session".into(),
                    kind: NoticeKind::Failed,
                    summary: "Task failed".into(),
                    created_ms: 11,
                    read_ms: None,
                },
            ],
        );
        assert_eq!(views[0].session_title.as_deref(), Some("修通知"));
        assert_eq!(views[0].last_reply.as_deref(), Some("已带上标题和最后回复"));
        assert_eq!(views[0].summary, "Task completed");
        assert!(views[1].session_title.is_none());
        assert!(views[1].last_reply.is_none());
        drop(views);
        std::fs::remove_dir_all(project).unwrap();
    }
}
