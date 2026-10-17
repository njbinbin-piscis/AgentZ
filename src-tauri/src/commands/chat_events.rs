//! Scope-authorized lifecycle history. Reading never resumes an agent action.
use crate::{runtime::event_replay::PageDto, state::AppState};
use agentz_task_state::events::Cursor;
use tauri::{AppHandle, State, WebviewWindow};

#[tauri::command]
pub async fn chat_task_details(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
) -> Result<agentz_task_state::task_details::TaskDetails, String> {
    let _environment = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let binding =
        super::chat_plan::existing_binding(&app, scope.project(), &session_id, scope.source())
            .await?;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("project scope changed during task detail lookup".into());
        }
        crate::runtime::task_details::read(verified.project(), &binding)
    })
}

#[tauri::command]
pub async fn chat_current_run(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
) -> Result<Option<agentz_task_state::run::Run>, String> {
    let _environment = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let binding =
        super::chat_plan::existing_binding(&app, scope.project(), &session_id, scope.source())
            .await?;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("project scope changed during Run lookup".into());
        }
        crate::runtime::event_replay::current_run(verified.project(), &binding)
    })
}

#[tauri::command]
pub async fn chat_run_events(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    run_id: String,
    attempt_id: String,
    cursor: Option<Cursor>,
    limit: usize,
) -> Result<PageDto, String> {
    if !(1..=200).contains(&limit) {
        return Err("event page limit must be between 1 and 200".into());
    }
    let _environment = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let binding =
        super::chat_plan::existing_binding(&app, scope.project(), &session_id, scope.source())
            .await?;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("project scope changed during event lookup".into());
        }
        crate::runtime::event_replay::replay(
            verified.project(),
            &binding,
            &run_id,
            &attempt_id,
            cursor,
            limit,
        )
    })
}
