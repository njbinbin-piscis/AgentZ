//! WorkZ project registry and defaults.
//!
//! Sessions still live in `{project}/.agentz/piscis.db`. This module only remembers
//! which folders the user has opened, the default working directory for plain
//! (project-less) sessions, and the internal "free sessions" folder that holds the
//! database for those plain sessions.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::commands::data_scope::resolve_global_config_dir;
use crate::commands::session::{chat_list_sessions, SessionMeta};

const STORE_FILE: &str = "workz.json";
const FREE_DIR_NAME: &str = "free-sessions";
const SCRATCH_DIR_NAME: &str = "workspace";
const MAX_PROJECTS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredProject {
    path: String,
    #[serde(default)]
    last_opened: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Store {
    #[serde(default)]
    projects: Vec<StoredProject>,
    #[serde(default)]
    default_work_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
    pub path: String,
    pub name: String,
    /// `repo` when the folder contains `.git`, otherwise `dir`.
    pub kind: String,
    pub exists: bool,
    pub last_opened: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkzOverview {
    pub projects: Vec<ProjectView>,
    /// Effective default working directory for plain sessions.
    pub default_work_dir: String,
    pub default_is_custom: bool,
    /// Internal project folder that stores plain-session databases.
    pub free_dir: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectSessions {
    pub project_dir: String,
    pub sessions: Vec<SessionMeta>,
    pub error: Option<String>,
}

fn store_path(config_dir: &Path) -> PathBuf {
    config_dir.join(STORE_FILE)
}

fn load_store(config_dir: &Path) -> Store {
    std::fs::read_to_string(store_path(config_dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_store(config_dir: &Path, store: &Store) -> Result<(), String> {
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    std::fs::write(store_path(config_dir), text).map_err(|e| e.to_string())
}

fn norm_key(path: &str) -> String {
    let s = path.trim().replace('\\', "/");
    let s = s.trim_end_matches('/');
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

fn display_name(path: &str) -> String {
    let p = path.trim().trim_end_matches(['/', '\\']);
    p.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or(p).to_string()
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn touch_project(store: &mut Store, path: &str, now: i64) {
    let key = norm_key(path);
    if let Some(p) = store.projects.iter_mut().find(|p| norm_key(&p.path) == key) {
        p.last_opened = now;
        return;
    }
    store.projects.push(StoredProject {
        path: path.trim().trim_end_matches(['/', '\\']).to_string(),
        last_opened: now,
    });
    store.projects.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
    store.projects.truncate(MAX_PROJECTS);
}

fn remove_project(store: &mut Store, path: &str) {
    let key = norm_key(path);
    store.projects.retain(|p| norm_key(&p.path) != key);
}

fn free_dir(config_dir: &Path) -> Result<PathBuf, String> {
    let dir = config_dir.join(FREE_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| format!("create free-sessions dir: {e}"))?;
    Ok(dir)
}

fn effective_default_dir(config_dir: &Path, store: &Store) -> (PathBuf, bool) {
    if let Some(custom) = store
        .default_work_dir
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        let p = PathBuf::from(custom);
        if p.is_dir() {
            return (p, true);
        }
    }
    let scratch = config_dir.join(SCRATCH_DIR_NAME);
    let _ = std::fs::create_dir_all(&scratch);
    (scratch, false)
}

fn view_of(store: &Store) -> Vec<ProjectView> {
    let mut out: Vec<ProjectView> = store
        .projects
        .iter()
        .map(|p| {
            let dir = Path::new(&p.path);
            let exists = dir.is_dir();
            ProjectView {
                path: p.path.clone(),
                name: display_name(&p.path),
                kind: if exists && dir.join(".git").exists() { "repo" } else { "dir" }.to_string(),
                exists,
                last_opened: p.last_opened,
            }
        })
        .collect();
    out.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
    out
}

fn overview(config_dir: &Path) -> Result<WorkzOverview, String> {
    let store = load_store(config_dir);
    let free = free_dir(config_dir)?;
    let (default_dir, custom) = effective_default_dir(config_dir, &store);
    Ok(WorkzOverview {
        projects: view_of(&store),
        default_work_dir: default_dir.to_string_lossy().to_string(),
        default_is_custom: custom,
        free_dir: free.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn workz_overview(app: AppHandle) -> Result<WorkzOverview, String> {
    overview(&resolve_global_config_dir(&app)?)
}

#[tauri::command]
pub async fn workz_project_add(app: AppHandle, path: String) -> Result<WorkzOverview, String> {
    let config_dir = resolve_global_config_dir(&app)?;
    let trimmed = path.trim();
    if trimmed.is_empty() || !Path::new(trimmed).is_dir() {
        return Err(format!("directory not found: {trimmed}"));
    }
    let mut store = load_store(&config_dir);
    touch_project(&mut store, trimmed, now_secs());
    save_store(&config_dir, &store)?;
    overview(&config_dir)
}

#[tauri::command]
pub async fn workz_project_remove(app: AppHandle, path: String) -> Result<WorkzOverview, String> {
    let config_dir = resolve_global_config_dir(&app)?;
    let mut store = load_store(&config_dir);
    remove_project(&mut store, &path);
    save_store(&config_dir, &store)?;
    overview(&config_dir)
}

#[tauri::command]
pub async fn workz_set_default_dir(
    app: AppHandle,
    path: Option<String>,
) -> Result<WorkzOverview, String> {
    let config_dir = resolve_global_config_dir(&app)?;
    let mut store = load_store(&config_dir);
    store.default_work_dir = match path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
        Some(p) => {
            if !Path::new(&p).is_dir() {
                return Err(format!("directory not found: {p}"));
            }
            Some(p)
        }
        None => None,
    };
    save_store(&config_dir, &store)?;
    overview(&config_dir)
}

/// Header-only session lists for several projects at once. A project whose
/// database cannot be opened reports an error instead of failing the whole call.
#[tauri::command]
pub async fn workz_list_all_sessions(
    app: AppHandle,
    project_dirs: Vec<String>,
    sources: Option<Vec<String>>,
) -> Result<Vec<ProjectSessions>, String> {
    let mut out = Vec::with_capacity(project_dirs.len());
    for dir in project_dirs {
        if !Path::new(&dir).is_dir() {
            out.push(ProjectSessions {
                project_dir: dir,
                sessions: Vec::new(),
                error: Some("directory not found".into()),
            });
            continue;
        }
        match chat_list_sessions(app.clone(), Some(dir.clone()), sources.clone(), None).await {
            Ok(sessions) => out.push(ProjectSessions { project_dir: dir, sessions, error: None }),
            Err(e) => out.push(ProjectSessions {
                project_dir: dir,
                sessions: Vec::new(),
                error: Some(e),
            }),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("agentz-projects-{tag}-{}", now_secs()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn add_dedups_and_normalizes() {
        let mut s = Store::default();
        touch_project(&mut s, "C:\\Work\\App\\", 1);
        touch_project(&mut s, "c:/work/app", 5);
        if cfg!(windows) {
            assert_eq!(s.projects.len(), 1);
            assert_eq!(s.projects[0].last_opened, 5);
        }
        touch_project(&mut s, "D:/other", 9);
        remove_project(&mut s, "D:\\other\\");
        assert!(s.projects.iter().all(|p| !p.path.contains("other")));
    }

    #[test]
    fn kind_name_and_defaults() {
        let cfg = temp_dir("cfg");
        let repo = temp_dir("repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let mut store = Store::default();
        touch_project(&mut store, &repo.to_string_lossy(), 2);
        touch_project(&mut store, "Z:/definitely/missing", 1);
        let v = view_of(&store);
        assert_eq!(v[0].kind, "repo");
        assert!(v[0].exists);
        assert!(!v[1].exists);
        assert_eq!(v[1].name, "missing");

        let (d, custom) = effective_default_dir(&cfg, &store);
        assert!(!custom && d.is_dir());
        store.default_work_dir = Some(repo.to_string_lossy().to_string());
        let (d2, custom2) = effective_default_dir(&cfg, &store);
        assert!(custom2);
        assert_eq!(d2, repo);

        save_store(&cfg, &store).unwrap();
        assert_eq!(load_store(&cfg).projects.len(), 2);
        let ov = overview(&cfg).unwrap();
        assert!(Path::new(&ov.free_dir).is_dir());
    }
}
