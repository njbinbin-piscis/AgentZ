//! Scoped, local reminder source for durable automation occurrences. A due
//! reminder posts an inbox event; it never runs a model or external action.
use crate::state::AppState;
use agentz_task_state::{
    automation::{DispatchOutcome, MissedWindow, Trigger, TriggerStore},
    inbox::{InboxStore, Notice, NoticeKind},
};
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{State, WebviewWindow};

const MIN_MINUTES: i64 = 5;
const MAX_MINUTES: i64 = 7 * 24 * 60;
const MAX_PER_POLL: usize = 25;

#[derive(Serialize)]
pub struct ReminderStatus {
    pub interval_minutes: i64,
    pub next_due_ms: i64,
}

fn clock_ms() -> Result<i64, String> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid reminder clock")?
            .as_millis(),
    )
    .map_err(|_| "reminder clock overflow".into())
}

fn reminder_id(task_id: &str) -> Result<String, String> {
    if task_id.trim().is_empty() || task_id.len() > 200 || task_id.chars().any(char::is_control) {
        return Err("invalid reminder task".into());
    }
    Ok(format!("reminder:{task_id}"))
}

#[tauri::command]
pub async fn automation_reminder_status(
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    task_id: String,
) -> Result<Option<ReminderStatus>, String> {
    let id = reminder_id(&task_id)?;
    let _lease = state.environment_access.execution().await;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |scope| {
        if !scope.project().join(".agentz/automation.db").exists() {
            return Ok(None);
        }
        Ok(TriggerStore::open(scope.project())?
            .active(&id)?
            .map(|trigger| ReminderStatus {
                interval_minutes: trigger.interval_ms / 60_000,
                next_due_ms: trigger.next_due_ms,
            }))
    })
}

#[tauri::command]
pub async fn automation_reminder_set(
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    task_id: String,
    interval_minutes: i64,
) -> Result<ReminderStatus, String> {
    let id = reminder_id(&task_id)?;
    if !(MIN_MINUTES..=MAX_MINUTES).contains(&interval_minutes) {
        return Err("reminder interval must be 5 minutes to 7 days".into());
    }
    let interval_ms = interval_minutes * 60_000;
    let due_ms = clock_ms()?
        .checked_add(interval_ms)
        .ok_or("reminder clock overflow")?;
    let _lease = state.environment_access.execution().await;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |scope| {
        TriggerStore::open(scope.project())?.register(&Trigger {
            id,
            task_id,
            interval_ms,
            next_due_ms: due_ms,
            missed: MissedWindow::Latest,
        })?;
        Ok(ReminderStatus {
            interval_minutes,
            next_due_ms: due_ms,
        })
    })
}

#[tauri::command]
pub async fn automation_reminder_disable(
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    task_id: String,
) -> Result<(), String> {
    let id = reminder_id(&task_id)?;
    let _lease = state.environment_access.execution().await;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |scope| {
        TriggerStore::open(scope.project())?.disable(&id)
    })
}

#[tauri::command]
pub async fn automation_reminder_poll(
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
) -> Result<usize, String> {
    let now_ms = clock_ms()?;
    let _lease = state.environment_access.execution().await;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |scope| {
        poll_project(scope.project(), now_ms)
    })
}

fn poll_project(project: &Path, now_ms: i64) -> Result<usize, String> {
    if !project.join(".agentz/automation.db").exists() {
        return Ok(0);
    }
    let mut triggers = TriggerStore::open(project)?;
    let mut posted = 0;
    for _ in 0..MAX_PER_POLL {
        let Some(occurrence) = triggers.claim_due(now_ms)? else {
            break;
        };
        if !triggers.begin_dispatch(&occurrence)? {
            continue;
        }
        let notice = Notice {
            event_key: occurrence.action_key(),
            task_id: occurrence.task_id.clone(),
            kind: NoticeKind::ActionRequired,
            summary: "Scheduled task reminder is due".into(),
            created_ms: now_ms,
            read_ms: None,
        };
        match InboxStore::open(project).and_then(|mut inbox| inbox.post(&notice)) {
            Ok(inserted) => {
                triggers.finish_dispatch(&occurrence, DispatchOutcome::Succeeded)?;
                posted += usize::from(inserted);
            }
            Err(error) => {
                triggers.finish_dispatch(&occurrence, DispatchOutcome::Unknown)?;
                return Err(error);
            }
        }
    }
    Ok(posted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminder_poll_after_clock_jump_posts_once_and_disable_stops_future_slots() {
        let project =
            std::env::temp_dir().join(format!("agentz-reminder-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        let mut triggers = TriggerStore::open(&project).unwrap();
        triggers
            .register(&Trigger {
                id: "reminder:task".into(),
                task_id: "task".into(),
                interval_ms: 300_000,
                next_due_ms: 300_000,
                missed: MissedWindow::Latest,
            })
            .unwrap();
        drop(triggers);
        assert_eq!(poll_project(&project, 900_000).unwrap(), 1);
        assert_eq!(poll_project(&project, 900_000).unwrap(), 0);
        let inbox = InboxStore::open(&project).unwrap();
        assert_eq!(inbox.unread_count().unwrap(), 1);
        assert_eq!(inbox.recent(10).unwrap()[0].task_id, "task");
        drop(inbox);
        TriggerStore::open(&project)
            .unwrap()
            .disable("reminder:task")
            .unwrap();
        assert_eq!(poll_project(&project, 1_200_000).unwrap(), 0);
        std::fs::remove_dir_all(project).unwrap();
    }

    #[test]
    fn concurrent_polls_post_one_notice_for_one_due_slot() {
        let project =
            std::env::temp_dir().join(format!("agentz-reminder-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&project).unwrap();
        TriggerStore::open(&project)
            .unwrap()
            .register(&Trigger {
                id: "reminder:task".into(),
                task_id: "task".into(),
                interval_ms: 300_000,
                next_due_ms: 300_000,
                missed: MissedWindow::Latest,
            })
            .unwrap();
        let workers: Vec<_> = (0..2)
            .map(|_| {
                let project = project.clone();
                std::thread::spawn(move || poll_project(&project, 300_000).unwrap())
            })
            .collect();
        assert_eq!(
            workers
                .into_iter()
                .map(|worker| worker.join().unwrap())
                .sum::<usize>(),
            1
        );
        let inbox = InboxStore::open(&project).unwrap();
        assert_eq!(inbox.unread_count().unwrap(), 1);
        drop(inbox);
        std::fs::remove_dir_all(project).unwrap();
    }
}
