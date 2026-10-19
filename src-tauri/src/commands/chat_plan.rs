//! Existing local single-agent task plans. No Run is created here.
use crate::{
    runtime::plan_intent::{ExpectedPlan, PlanIntent, PlanQuery},
    state::AppState,
};
use agentz_task_state::plan::{Plan, PlanBinding, PlanStore};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, State, WebviewWindow};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_workz_source_and_metadata_never_accept_team_pool_or_unknown_session() {
        let db = piscis_kernel::store::db::Database::open(Path::new(":memory:")).unwrap();
        assert!(require_existing_task(&db, "missing").is_err());
        assert!(db.get_session("missing").unwrap().is_none());
        let plain = db
            .create_session_with_source(None, super::super::session_sources::SOURCE_WORKZ)
            .unwrap();
        assert!(require_existing_task(&db, &plain.id).unwrap().workz);
        for source in [
            super::super::session_sources::SOURCE_WORKZ_TEAM,
            super::super::session_sources::SOURCE_POOL,
            super::super::session_sources::SOURCE_PISCIS_HEARTBEAT_POOL,
            "remote",
        ] {
            let session = db.create_session_with_source(None, source).unwrap();
            assert!(require_existing_task(&db, &session.id).is_err());
        }
        db.update_session_state_frame_json(
            &plain.id,
            Some(r#"{"agentz_workz":{"team_id":"team"}}"#),
        )
        .unwrap();
        assert!(require_existing_task(&db, &plain.id).is_err());
        db.update_session_state_frame_json(
            &plain.id,
            Some(r#"{"agentz_workz":{"pool_id":"pool"}}"#),
        )
        .unwrap();
        assert!(require_existing_task(&db, &plain.id).is_err());
        db.update_session_state_frame_json(&plain.id, Some("malformed"))
            .unwrap();
        assert!(require_existing_task(&db, &plain.id).is_err());
    }
    #[test]
    fn free_metadata_requires_trusted_scope_absolute_matching_workspace_no_fallback() {
        let root = std::env::temp_dir().join(format!("agentz-plan-cwd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("other")).unwrap();
        let cwd = root.to_string_lossy().to_string();
        let mut task = ExistingTask {
            workz: true,
            workspace: Some(cwd.clone()),
            cwd: Some(cwd.clone()),
        };
        assert!(requested_workspace(&task, false, false).is_err());
        assert_eq!(requested_workspace(&task, false, true).unwrap(), Some(cwd));
        task.workspace = Some(root.join("other").to_string_lossy().to_string());
        assert!(requested_workspace(&task, false, true).is_err());
        task.workspace = None;
        assert!(requested_workspace(&task, false, true).is_err());
        task.cwd = None;
        assert!(requested_workspace(&task, false, true).is_err());
        task.cwd = Some("relative".into());
        task.workspace = Some("relative".into());
        assert!(requested_workspace(&task, false, true).is_err());
        // Registered environments are resolved and validated by the authoritative resolver,
        // never substituted from free metadata.
        assert_eq!(
            requested_workspace(&task, true, false).unwrap(),
            Some("relative".into())
        );
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_session_validation_never_creates_missing_or_accepts_workz() {
        let db = piscis_kernel::store::db::Database::open(Path::new(":memory:")).unwrap();
        assert!(require_existing_codez(&db, "missing").is_err());
        assert!(db.get_session("missing").unwrap().is_none());
        let codez = db
            .create_session_with_source(None, super::super::session_sources::SOURCE_CODEZ)
            .unwrap();
        assert!(require_existing_codez(&db, &codez.id).is_ok());
        assert!(require_existing_codez(&db, &format!(" {}", codez.id)).is_err());
        let workz = db
            .create_session_with_source(None, super::super::session_sources::SOURCE_WORKZ)
            .unwrap();
        assert!(require_existing_codez(&db, &workz.id)
            .unwrap_err()
            .contains("WorkZ"));
        let legacy = db
            .create_session_with_source(None, super::super::session_sources::SOURCE_LEGACY)
            .unwrap();
        assert!(require_existing_codez(&db, &legacy.id).is_ok());
    }
}

#[cfg(test)]
fn require_existing_codez(
    db: &piscis_kernel::store::db::Database,
    session_id: &str,
) -> Result<Option<String>, String> {
    if session_id.trim().is_empty() || session_id.trim() != session_id {
        return Err("existing session identity required".into());
    }
    let session = db
        .get_session(session_id)
        .map_err(|e| format!("cannot read plan session: {e}"))?
        .ok_or("plan session not found")?;
    if !super::session_sources::is_codez_source(&session.source) {
        return Err(
            "plan IPC currently supports existing CodeZ sessions only; WorkZ is not supported yet"
                .into(),
        );
    }
    Ok(session
        .workspace_root
        .filter(|root| !root.trim().is_empty()))
}
struct ExistingTask {
    workspace: Option<String>,
    workz: bool,
    cwd: Option<String>,
}
#[cfg(test)]
fn require_existing_task(
    db: &piscis_kernel::store::db::Database,
    session_id: &str,
) -> Result<ExistingTask, String> {
    if session_id.trim().is_empty() || session_id.trim() != session_id {
        return Err("existing session identity required".into());
    }
    let session = db
        .get_session(session_id)
        .map_err(|e| e.to_string())?
        .ok_or("plan session not found")?;
    if super::session_sources::is_codez_source(&session.source) {
        return Ok(ExistingTask {
            workspace: require_existing_codez(db, session_id)?,
            workz: false,
            cwd: None,
        });
    }
    if session.source != super::session_sources::SOURCE_WORKZ || session.pool_session_id.is_some() {
        return Err("team, pool, remote and background plan sessions are unsupported".into());
    }
    let meta = super::session::load_workz_meta(db, session_id)?;
    if meta.team_id.is_some() || meta.pool_id.is_some() {
        return Err("team/pool plan metadata is unsupported".into());
    }
    Ok(ExistingTask {
        workspace: session.workspace_root,
        workz: true,
        cwd: meta.cwd,
    })
}
fn requested_workspace(
    task: &ExistingTask,
    registered: bool,
    free_scope: bool,
) -> Result<Option<String>, String> {
    if registered || !task.workz {
        return Ok(task.workspace.clone().filter(|s| !s.trim().is_empty()));
    }
    if free_scope {
        let cwd = task
            .cwd
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or("free task has no persisted cwd; explicit recovery required")?;
        let workspace = task
            .workspace
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .ok_or("free task has no matching session workspace; explicit recovery required")?;
        if !Path::new(cwd).is_absolute() || !Path::new(workspace).is_absolute() {
            return Err("free task cwd/workspace must be absolute local paths".into());
        }
        if agentz_task_state::canonical_root(Path::new(cwd))?
            != agentz_task_state::canonical_root(Path::new(workspace))?
        {
            return Err("free task persisted cwd/session workspace mismatch".into());
        }
        Ok(Some(cwd.into()))
    } else {
        if task.cwd.as_ref().is_some_and(|s| !s.trim().is_empty()) {
            return Err("unregistered WorkZ cwd requires a trusted host free scope".into());
        }
        Ok(task.workspace.clone().filter(|s| !s.trim().is_empty()))
    }
}
pub(crate) async fn existing_binding(
    app: &AppHandle,
    project: &Path,
    session_id: &str,
    scope_source: crate::runtime::project_scope::TrustedOpenSource,
) -> Result<PlanBinding, String> {
    let metadata = super::session_metadata::read_existing(project, session_id)?;
    let task = ExistingTask {
        workspace: metadata.workspace,
        workz: metadata.workz,
        cwd: metadata.cwd,
    };
    let registered = agentz_task_state::load(project, session_id)?.is_some();
    let free_scope = if task.workz && !registered {
        let config_dir = super::data_scope::resolve_global_config_dir(app)?;
        let free_dir = super::projects::free_sessions_path(&config_dir);
        let same = free_dir
            .try_exists()
            .map_err(|error| format!("cannot inspect free-task metadata location: {error}"))?
            && agentz_task_state::canonical_root(project)?
                == agentz_task_state::canonical_root(&free_dir)?;
        if same
            && scope_source != crate::runtime::project_scope::TrustedOpenSource::RestoredWorkspace
        {
            return Err("free task requires a host restored free-project scope".into());
        }
        same
    } else {
        false
    };
    let projectless_app_data = if free_scope {
        use tauri::Manager as _;
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
        if let Some(directory) =
            super::project_scope::projectless_session_directory(&app_data, project, session_id)?
        {
            let bound = agentz_task_state::canonical_root(&directory)?;
            let saved = task
                .workspace
                .as_deref()
                .ok_or("projectless session has no saved workspace")?;
            if agentz_task_state::canonical_root(Path::new(saved))? != bound {
                return Err("projectless session workspace differs from catalog binding".into());
            }
            Some((app_data, bound))
        } else {
            None
        }
    } else {
        None
    };
    let requested = if let Some((_, bound)) = &projectless_app_data {
        Some(bound.clone())
    } else {
        requested_workspace(&task, registered, free_scope)?
    };
    let workspace = super::agent_task::resolve_task_workspace(
        project,
        Some(session_id),
        requested.as_deref(),
        registered,
    )
    .await?;
    let workspace = PathBuf::from(agentz_task_state::canonical_root(Path::new(&workspace))?);
    let project_id = agentz_task_state::canonical_root(project)?;
    let require_binding = registered
        || projectless_app_data.is_some()
        || (!free_scope && workspace.as_path() != Path::new(&project_id));
    // Metadata-only host identity required by the existing resolver API. It is
    // never queued, started, persisted as a Run, or used to authorize tool execution.
    let metadata_id = uuid::Uuid::new_v4().to_string();
    let execution = crate::runtime::approval::resolve_binding_for_mode(
        project,
        &workspace,
        session_id,
        &metadata_id,
        require_binding,
        projectless_app_data
            .as_ref()
            .map(|(app_data, _)| app_data.as_path()),
    )
    .await?;
    PlanBinding::from_execution(&execution)
}

#[tauri::command]
pub async fn chat_plan_query(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
) -> Result<PlanQuery, String> {
    let _environment = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let binding = existing_binding(&app, scope.project(), &session_id, scope.source()).await?;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("project scope changed during plan lookup".into());
        }
        let store = PlanStore::open(verified.project())?;
        crate::runtime::plan_intent::query(&store, || Ok(binding))
    })
}

#[tauri::command]
pub async fn chat_plan_apply(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected: ExpectedPlan,
    intent: PlanIntent,
) -> Result<Plan, String> {
    let _environment = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let binding = existing_binding(&app, scope.project(), &session_id, scope.source()).await?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "invalid plan clock")?
        .as_millis();
    let now = u64::try_from(now).map_err(|_| "plan clock overflow")?;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("project scope changed during plan mutation".into());
        }
        crate::runtime::plan_intent::apply_under_task_gate(
            verified.project(),
            &state.task_activity,
            &binding,
            &expected,
            intent,
            now,
        )
    })
}
