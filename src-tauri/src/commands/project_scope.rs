//! Main-GUI opened-project identities. No arbitrary path -> scope command.
use crate::{
    runtime::project_scope::{TrustedOpenSource, VerifiedProjectScope},
    state::AppState,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

pub(crate) const MAIN_GENERATION: u64 = 1;
#[derive(Serialize)]
pub struct ProjectScopeDto {
    pub handle: String,
    pub project_dir: String,
}
fn dto(scope: VerifiedProjectScope) -> ProjectScopeDto {
    ProjectScopeDto {
        handle: scope.handle().into(),
        project_dir: renderer_path(scope.project()),
    }
}

/// Canonical Windows paths carry the verbatim `\\?\` prefix. In verbatim form Windows does not
/// treat `/` as a separator, and the renderer joins `projectDir` with `/` to open files, so
/// those joins resolve to nothing. The renderer gets the ordinary spelling. Backend checks keep
/// the canonical value; the renderer's path is canonicalized again on every use.
fn renderer_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == br":\" {
            return rest.to_string();
        }
    }
    text.into_owned()
}
fn require_main(window: &WebviewWindow) -> Result<(), String> {
    if window.label() != "main" {
        return Err("project scopes require the main GUI window".into());
    }
    Ok(())
}
pub(crate) fn lookup_for_main(
    state: &AppState,
    window: &WebviewWindow,
    handle: &str,
) -> Result<VerifiedProjectScope, String> {
    require_main(window)?;
    state
        .project_scopes
        .lock()
        .map_err(|_| "project scope registry unavailable")?
        .lookup(handle, "main", MAIN_GENERATION)
}
/// All async preparation must finish first. Keep scope revocation excluded through
/// the synchronous callback's identity check/storage CAS; never await inside it.
pub(crate) fn with_verified_scope<T>(
    state: &AppState,
    window: &WebviewWindow,
    handle: &str,
    operation: impl FnOnce(&VerifiedProjectScope) -> Result<T, String>,
) -> Result<T, String> {
    require_main(window)?;
    let registry = state
        .project_scopes
        .lock()
        .map_err(|_| "project scope registry unavailable")?;
    let scope = registry.lookup(handle, "main", MAIN_GENERATION)?;
    operation(&scope)
}
fn mint(
    state: &AppState,
    project: &Path,
    source: TrustedOpenSource,
) -> Result<ProjectScopeDto, String> {
    let scope = state
        .project_scopes
        .lock()
        .map_err(|_| "project scope registry unavailable")?
        .mint("main", MAIN_GENERATION, project, source)?;
    Ok(dto(scope))
}
#[derive(Serialize)]
pub struct ProjectlessScopeDto {
    pub directory_id: String,
    pub scope: ProjectScopeDto,
}
#[derive(Serialize)]
pub struct ProjectlessSessionScopeDto {
    pub session_id: String,
    pub session_project_dir: String,
    pub directory: ProjectlessScopeDto,
}
/// Allocates a new identity only; never adopts an existing chat session.
/// Binding is persisted before returning, but this does not run/create chat.
#[tauri::command]
pub async fn projectless_session_create(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<ProjectlessSessionScopeDto, String> {
    require_main(&window)?;
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let project =
        super::projects::free_sessions_path(&super::data_scope::resolve_global_config_dir(&app)?);
    let project = project
        .to_str()
        .ok_or("session project is not UTF-8")?
        .to_owned();
    let session_id = uuid::Uuid::new_v4().to_string();
    let directory_id = uuid::Uuid::new_v4().to_string();
    let session = session_id.clone();
    let id = directory_id.clone();
    let root = PathBuf::from(&project);
    let directory = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let store = agentz_task_state::projectless::AppDataStore::initialize(&app_data)?;
        store.create_task(&id)?;
        store.bind_session(&root, &session, &id)
    })
    .await
    .map_err(|e| e.to_string())??;
    let scope = mint(&state, &directory, TrustedOpenSource::TrustedGuiOpen).map_err(|e| {
        format!("session {session_id}, directory {directory_id} retained; scope unavailable: {e}")
    })?;
    Ok(ProjectlessSessionScopeDto {
        session_id,
        session_project_dir: project,
        directory: ProjectlessScopeDto {
            directory_id,
            scope,
        },
    })
}
#[tauri::command]
pub async fn projectless_session_reopen(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<ProjectlessSessionScopeDto, String> {
    require_main(&window)?;
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    // Pure path resolution: a missing session DB root must remain missing.
    let project =
        super::projects::free_sessions_path(&super::data_scope::resolve_global_config_dir(&app)?);
    let project = project
        .to_str()
        .ok_or("session project is not UTF-8")?
        .to_owned();
    let session = session_id.clone();
    let root = PathBuf::from(&project);
    let directory = tokio::task::spawn_blocking(move || {
        agentz_task_state::projectless::AppDataStore::open_existing(&app_data)?
            .reopen_session(&root, &session)
    })
    .await
    .map_err(|e| e.to_string())??;
    let directory_id = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("managed directory identity unavailable")?
        .to_owned();
    let scope = mint(&state, &directory, TrustedOpenSource::TrustedGuiOpen)?;
    Ok(ProjectlessSessionScopeDto {
        session_id,
        session_project_dir: project,
        directory: ProjectlessScopeDto {
            directory_id,
            scope,
        },
    })
}
#[tauri::command]
pub async fn projectless_session_lookup(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Option<ProjectlessSessionScopeDto>, String> {
    require_main(&window)?;
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let project =
        super::projects::free_sessions_path(&super::data_scope::resolve_global_config_dir(&app)?);
    let root = project.clone();
    let session = session_id.clone();
    let directory = tokio::task::spawn_blocking(move || {
        projectless_session_directory(&app_data, &root, &session)
    })
    .await
    .map_err(|e| e.to_string())??;
    let Some(directory) = directory else {
        return Ok(None);
    };
    let directory_id = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("managed directory identity unavailable")?
        .to_owned();
    let scope = mint(&state, &directory, TrustedOpenSource::TrustedGuiOpen)?;
    Ok(Some(ProjectlessSessionScopeDto {
        session_id,
        session_project_dir: project
            .to_str()
            .ok_or("session project is not UTF-8")?
            .to_owned(),
        directory: ProjectlessScopeDto {
            directory_id,
            scope,
        },
    }))
}
/// Managed directory that a projectless chat session must run in. `Ok(None)` when the session
/// is not projectless (no store was ever created, or it has no binding). Any failure for a
/// bound session is an error, so the caller refuses the turn instead of using the shared root.
pub(crate) fn projectless_session_directory(
    app_data: &std::path::Path,
    project: &std::path::Path,
    session_id: &str,
) -> Result<Option<PathBuf>, String> {
    let store_dir = app_data.join("projectless");
    if !store_dir.join("catalog.sqlite").exists() {
        if store_dir.join("session-bindings.sqlite").exists()
            || store_dir.join("workspaces").exists()
        {
            return Err(
                "projectless catalog is missing; refusing legacy workspace fallback".into(),
            );
        }
        return Ok(None);
    }
    agentz_task_state::projectless::AppDataStore::open_existing(app_data)?
        .session_directory(project, session_id)
}

/// Directory identity is not a chat session ID or an execution grant.
#[tauri::command]
pub async fn projectless_directory_create(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<ProjectlessScopeDto, String> {
    require_main(&window)?;
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let directory_id = uuid::Uuid::new_v4().to_string();
    let id = directory_id.clone();
    let directory = tokio::task::spawn_blocking(move || {
        // The base is selected by Tauri, never by renderer parameters/config overrides.
        std::fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;
        agentz_task_state::projectless::AppDataStore::initialize(&app_data)?.create_task(&id)
    })
    .await
    .map_err(|e| e.to_string())??;
    let scope = mint(&state, &directory, TrustedOpenSource::TrustedGuiOpen)
        .map_err(|e| format!("directory {directory_id} retained; scope unavailable: {e}"))?;
    Ok(ProjectlessScopeDto {
        directory_id,
        scope,
    })
}
#[tauri::command]
pub async fn projectless_directory_reopen(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    directory_id: String,
) -> Result<ProjectlessScopeDto, String> {
    require_main(&window)?;
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let id = directory_id.clone();
    let directory = tokio::task::spawn_blocking(move || {
        agentz_task_state::projectless::AppDataStore::open_existing(&app_data)?.reopen_task(&id)
    })
    .await
    .map_err(|e| e.to_string())??;
    let scope = mint(&state, &directory, TrustedOpenSource::TrustedGuiOpen)?;
    Ok(ProjectlessScopeDto {
        directory_id,
        scope,
    })
}
#[tauri::command]
pub async fn project_scope_pick(
    window: WebviewWindow,
    state: State<'_, AppState>,
) -> Result<Option<ProjectScopeDto>, String> {
    require_main(&window)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .pick_folder(move |selected| {
            let _ = sender.send(selected);
        });
    let selected = receiver
        .await
        .map_err(|_| "native folder picker did not return")?;
    match selected {
        None => Ok(None),
        Some(selected) => {
            let path = selected
                .into_path()
                .map_err(|_| "selected directory is not local")?;
            mint(&state, &path, TrustedOpenSource::NativePicker).map(Some)
        }
    }
}
#[tauri::command]
pub async fn project_scope_restore(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    source: String,
    path: Option<String>,
) -> Result<ProjectScopeDto, String> {
    require_main(&window)?;
    let project = match source.as_str() {
        "workspace" => {
            if path.is_some() {
                return Err("workspace restore does not accept a project path".into());
            }
            super::workspace::workspace_load(app)
                .await?
                .and_then(|s| s.project_dir)
                .map(PathBuf::from)
                .ok_or("no saved local workspace")?
        }
        "workz_project" => {
            let requested = path.ok_or("known WorkZ project path required")?;
            let overview = super::projects::workz_overview(app).await?;
            select_known_project(
                &requested,
                &overview
                    .projects
                    .iter()
                    .map(|p| p.path.clone())
                    .collect::<Vec<_>>(),
            )?
        }
        "workz_free" => {
            if path.is_some() {
                return Err("free session scope does not accept a path".into());
            }
            PathBuf::from(super::projects::workz_overview(app).await?.free_dir)
        }
        _ => return Err("unknown project restore source".into()),
    };
    mint(&state, &project, TrustedOpenSource::RestoredWorkspace)
}
fn select_known_project(requested: &str, known: &[String]) -> Result<PathBuf, String> {
    let requested =
        std::fs::canonicalize(requested).map_err(|_| "requested project cannot be resolved")?;
    known
        .iter()
        .filter_map(|p| std::fs::canonicalize(p).ok())
        .find(|p| p == &requested)
        .ok_or_else(|| "project is not in the host WorkZ opened list".into())
}
#[tauri::command]
pub fn project_scope_revoke(
    window: WebviewWindow,
    state: State<'_, AppState>,
    handle: String,
) -> Result<(), String> {
    require_main(&window)?;
    state
        .project_scopes
        .lock()
        .map_err(|_| "project scope registry unavailable")?
        .revoke(&handle, "main", MAIN_GENERATION)
}
pub(crate) fn install_main_lifecycle(app: &AppHandle) -> Result<(), String> {
    app.state::<AppState>()
        .project_scopes
        .lock()
        .map_err(|_| "project scope registry unavailable")?
        .begin_window("main", MAIN_GENERATION)?;
    if let Some(window) = app.get_webview_window("main") {
        let scopes = app.state::<AppState>().project_scopes.clone();
        window.on_window_event(move |event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                if let Ok(mut registry) = scopes.lock() {
                    let _ = registry.close_window("main", MAIN_GENERATION);
                }
            }
        });
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_path_drops_verbatim_prefix_only_when_safe() {
        assert_eq!(
            renderer_path(Path::new(r"\\?\C:\Projects\AgentZ")),
            r"C:\Projects\AgentZ"
        );
        assert_eq!(
            renderer_path(Path::new(r"\\?\UNC\server\share\project")),
            r"\\server\share\project"
        );
        // Other verbatim forms and ordinary paths are returned unchanged.
        assert_eq!(renderer_path(Path::new(r"C:\plain")), r"C:\plain");
        assert_eq!(
            renderer_path(Path::new(r"\\?\Volume{1}\x")),
            r"\\?\Volume{1}\x"
        );
    }
    #[test]
    fn known_restore_does_not_accept_arbitrary_existing_directory() {
        let owned =
            std::env::temp_dir().join(format!("agentz-known-project-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(owned.join("known")).unwrap();
        std::fs::create_dir_all(owned.join("other")).unwrap();
        let known = vec![owned.join("known").to_string_lossy().into_owned()];
        assert!(select_known_project(&owned.join("other").to_string_lossy(), &known).is_err());
        assert_eq!(
            select_known_project(&owned.join("known/../known").to_string_lossy(), &known).unwrap(),
            std::fs::canonicalize(owned.join("known")).unwrap()
        );
        std::fs::remove_dir_all(owned).unwrap();
    }

    #[test]
    fn projectless_directory_lookup_is_none_until_a_store_exists_and_refuses_lost_bindings() {
        let app_data =
            std::env::temp_dir().join(format!("agentz-projectless-cwd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&app_data).unwrap();
        let project = app_data.join("free-sessions");
        std::fs::create_dir_all(&project).unwrap();
        let session = uuid::Uuid::new_v4().to_string();
        // No projectless store at all: an ordinary session, no error and no creation.
        assert!(projectless_session_directory(&app_data, &project, &session)
            .unwrap()
            .is_none());
        assert!(!app_data.join("projectless").exists());

        let store = agentz_task_state::projectless::AppDataStore::initialize(&app_data).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let directory = store.create_task(&id).unwrap();
        store.bind_session(&project, &session, &id).unwrap();
        drop(store);
        assert_eq!(
            projectless_session_directory(&app_data, &project, &session)
                .unwrap()
                .as_deref(),
            Some(directory.as_path())
        );
        std::fs::rename(
            app_data.join("projectless/catalog.sqlite"),
            app_data.join("projectless/lost-catalog.sqlite"),
        )
        .unwrap();
        assert!(projectless_session_directory(&app_data, &project, &session).is_err());
        std::fs::remove_dir_all(app_data).unwrap();
    }
}
