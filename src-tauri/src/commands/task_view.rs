//! Existing local task observation. Returned paths are descriptions, never IDE capabilities.
use crate::state::AppState;
use agentz_task_state::{plan::PlanBinding, TaskEnvironment};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State, WebviewWindow};

#[derive(Debug, Serialize)]
pub struct TaskView {
    pub schema_version: u32,
    pub scope: PlanBinding,
    pub session_id: String,
    pub authority_project: String,
    pub workspace_root: String,
    pub observation_only: bool,
}
fn workspace(
    project: &Path,
    metadata: &super::session_metadata::SessionMetadata,
    registered: &Option<TaskEnvironment>,
) -> Result<PathBuf, String> {
    let root = registered
        .as_ref()
        .map(|e| e.workspace_root.as_str())
        .or(metadata
            .workspace
            .as_deref()
            .filter(|s| !s.trim().is_empty()))
        .or(metadata.cwd.as_deref().filter(|s| !s.trim().is_empty()));
    let path = root
        .map(PathBuf::from)
        .unwrap_or_else(|| project.to_path_buf());
    if !path.is_absolute() || !path.is_dir() {
        return Err("task view requires an existing absolute local workspace".into());
    }
    Ok(PathBuf::from(agentz_task_state::canonical_root(&path)?))
}
fn describe(
    project: &Path,
    session: &str,
    binding: &PlanBinding,
    root: &Path,
    captured_metadata: &super::session_metadata::SessionMetadata,
    captured_environment: &Option<TaskEnvironment>,
) -> Result<TaskView, String> {
    if super::session_metadata::read_existing(project, session)? != *captured_metadata
        || agentz_task_state::load_existing_readonly(project, session)? != *captured_environment
    {
        return Err("task view metadata changed during resolution".into());
    }
    let authority = agentz_task_state::canonical_root(project)?;
    let resolved = workspace(project, captured_metadata, captured_environment)?;
    if binding.project_id != authority || binding.task_id != session || resolved != root {
        return Err("task view scope/workspace mismatch".into());
    }
    if let Some(e) = captured_environment {
        if e.project_root != authority
            || e.task_id != session
            || e.environment_id != binding.environment_id
        {
            return Err("task view environment mismatch".into());
        }
    } else if binding.environment_id != format!("direct:{}", binding.binding_version) {
        return Err("task view unregistered environment".into());
    }
    Ok(TaskView {
        schema_version: 1,
        scope: binding.clone(),
        session_id: session.into(),
        authority_project: authority,
        workspace_root: resolved.to_string_lossy().into_owned(),
        observation_only: true,
    })
}
pub(crate) struct CapturedView {
    pub(crate) binding: PlanBinding,
    pub(crate) root: PathBuf,
    metadata: super::session_metadata::SessionMetadata,
    registered: Option<TaskEnvironment>,
    projectless_app_data: Option<PathBuf>,
}
pub(crate) async fn resolve_observation(
    app: &AppHandle,
    project: &Path,
    session_id: &str,
    source: crate::runtime::project_scope::TrustedOpenSource,
) -> Result<CapturedView, String> {
    let binding = super::chat_plan::existing_binding(app, project, session_id, source).await?;
    let metadata = super::session_metadata::read_existing(project, session_id)?;
    let registered = agentz_task_state::load_existing_readonly(project, session_id)?;
    let root = workspace(project, &metadata, &registered)?;
    let projectless_app_data = if metadata.workz && registered.is_none() {
        use tauri::Manager as _;
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
        match super::project_scope::projectless_session_directory(&app_data, project, session_id)? {
            Some(directory) => {
                if agentz_task_state::canonical_root(&directory)?
                    != agentz_task_state::canonical_root(&root)?
                {
                    return Err("task view differs from projectless catalog binding".into());
                }
                Some(app_data)
            }
            None => None,
        }
    } else {
        None
    };
    // Existing resolver also checks managed checkout identity/direct Git fingerprint.
    let resolved = crate::runtime::approval::resolve_binding_for_mode(
        project,
        &root,
        session_id,
        &uuid::Uuid::new_v4().to_string(),
        registered.is_some()
            || projectless_app_data.is_some()
            || (!(metadata.workz && metadata.cwd.is_some()) && root != project),
        projectless_app_data.as_deref(),
    )
    .await?;
    if PlanBinding::from_execution(&resolved)? != binding {
        return Err("task view binding changed during resolution".into());
    }
    Ok(CapturedView {
        binding,
        root,
        metadata,
        registered,
        projectless_app_data,
    })
}
pub(crate) fn verify_observation(
    project: &Path,
    session_id: &str,
    captured: &CapturedView,
) -> Result<(), String> {
    if let Some(app_data) = captured.projectless_app_data.as_deref() {
        let bound = agentz_task_state::projectless::AppDataStore::open_existing(app_data)?
            .reopen_session(project, session_id)?;
        if agentz_task_state::canonical_root(&bound)?
            != agentz_task_state::canonical_root(&captured.root)?
        {
            return Err("task view projectless catalog binding changed".into());
        }
    }
    describe(
        project,
        session_id,
        &captured.binding,
        &captured.root,
        &captured.metadata,
        &captured.registered,
    )
    .map(|_| ())
}
#[tauri::command]
pub async fn task_view_resolve(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
) -> Result<TaskView, String> {
    let _environment = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let captured = resolve_observation(&app, scope.project(), &session_id, scope.source()).await?;
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("task view project scope changed".into());
        }
        verify_observation(verified.project(), &session_id, &captured)?;
        describe(
            verified.project(),
            &session_id,
            &captured.binding,
            &captured.root,
            &captured.metadata,
            &captured.registered,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("task-view-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn session(&self, source: &str) -> String {
            std::fs::create_dir_all(self.0.join(".agentz")).unwrap();
            let db = piscis_kernel::store::db::Database::open(&self.0.join(".agentz/piscis.db"))
                .unwrap();
            db.create_session_with_source(None, source).unwrap().id
        }
        fn binding(&self, id: &str) -> PlanBinding {
            PlanBinding {
                project_id: agentz_task_state::canonical_root(&self.0).unwrap(),
                task_id: id.into(),
                environment_id: format!("direct:{}", "a".repeat(64)),
                binding_version: "a".repeat(64),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn direct_existing_description_preserves_authority_and_does_not_create_run() {
        let f = Fixture::new();
        let id = f.session("workz");
        let metadata = super::super::session_metadata::read_existing(&f.0, &id).unwrap();
        let before = std::fs::read(f.0.join(".agentz/piscis.db")).unwrap();
        let binding = f.binding(&id);
        let root = workspace(&f.0, &metadata, &None).unwrap();
        let dto = describe(&f.0, &id, &binding, &root, &metadata, &None).unwrap();
        assert_eq!(dto.session_id, id);
        assert_eq!(dto.authority_project, binding.project_id);
        assert_eq!(dto.workspace_root, binding.project_id);
        assert!(dto.observation_only);
        assert!(!f.0.join(".agentz/runs.db").exists());
        assert!(!f.0.join(".agentz/task-environments.db").exists());
        assert_eq!(
            std::fs::read(f.0.join(".agentz/piscis.db")).unwrap(),
            before
        );
    }
    #[test]
    fn missing_session_and_unsupported_source_fail_without_creation() {
        let f = Fixture::new();
        assert!(super::super::session_metadata::read_existing(&f.0, "missing").is_err());
        assert!(!f.0.join(".agentz").exists());
        for source in ["workz_team", "unknown", "background"] {
            let id = f.session(source);
            assert!(super::super::session_metadata::read_existing(&f.0, &id).is_err());
        }
    }
    #[test]
    fn changed_metadata_and_wrong_task_scope_are_rejected() {
        let f = Fixture::new();
        let id = f.session("workz");
        let metadata = super::super::session_metadata::read_existing(&f.0, &id).unwrap();
        let root = workspace(&f.0, &metadata, &None).unwrap();
        let mut binding = f.binding(&id);
        binding.task_id = "other".into();
        assert!(describe(&f.0, &id, &binding, &root, &metadata, &None).is_err());
        let db = piscis_kernel::store::db::Database::open(&f.0.join(".agentz/piscis.db")).unwrap();
        db.update_session_state_frame_json(
            &id,
            Some(r#"{"agentz_workz":{"cwd":"invalid-changed"}}"#),
        )
        .unwrap();
        drop(db);
        assert!(describe(&f.0, &id, &f.binding(&id), &root, &metadata, &None).is_err());
    }
    #[test]
    fn observed_workspace_cannot_be_replaced_by_unrelated_path() {
        let f = Fixture::new();
        let other = Fixture::new();
        let id = f.session("codez");
        let metadata = super::super::session_metadata::read_existing(&f.0, &id).unwrap();
        assert!(describe(&f.0, &id, &f.binding(&id), &other.0, &metadata, &None).is_err());
    }
    #[test]
    fn registered_workspace_retains_original_project_and_session() {
        let f = Fixture::new();
        let checkout = Fixture::new();
        let id = f.session("workz");
        let record = TaskEnvironment {
            schema_version: 1,
            task_id: id.clone(),
            environment_id: "registered-env".into(),
            project_root: agentz_task_state::canonical_root(&f.0).unwrap(),
            workspace_root: agentz_task_state::canonical_root(&checkout.0).unwrap(),
            kind: "local".into(),
            branch: None,
            base_ref: String::new(),
            base_sha: String::new(),
            created_at: "fixture".into(),
        };
        agentz_task_state::register(&f.0, &record).unwrap();
        let metadata = super::super::session_metadata::read_existing(&f.0, &id).unwrap();
        let mut binding = f.binding(&id);
        binding.environment_id = record.environment_id.clone();
        let captured = Some(record.clone());
        let root = workspace(&f.0, &metadata, &captured).unwrap();
        let before = std::fs::read(f.0.join(".agentz/task-environments.db")).unwrap();
        let dto = describe(&f.0, &id, &binding, &root, &metadata, &captured).unwrap();
        assert_eq!(dto.authority_project, record.project_root);
        assert_eq!(dto.session_id, id);
        assert_eq!(dto.workspace_root, record.workspace_root);
        assert_ne!(dto.workspace_root, dto.authority_project);
        assert_eq!(
            std::fs::read(f.0.join(".agentz/task-environments.db")).unwrap(),
            before
        );
        assert!(!f.0.join(".agentz/runs.db").exists());
    }
}
