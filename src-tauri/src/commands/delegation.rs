//! Global delegation settings, prompt context, concurrency cap and result cache
//! for sub-agent delegation (`delegate`, `call_fish`, `task`).
//!
//! Stored in an AgentZ-owned `{config}/delegation.json` (same approach as
//! `flash.json`), because the pinned kernel owns `config.json`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::commands::data_scope::resolve_global_config_dir;
use crate::runtime::subagent_policy::{metrics_snapshot, DelegationMetrics};

pub const DEFAULT_MAX_CONCURRENT: u32 = 4;
const CACHE_TTL: Duration = Duration::from_secs(600);
const CACHE_CAP: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegationMode {
    /// `delegate`, `call_fish` and `task` are all available.
    #[default]
    Auto,
    /// Only the read-only research tools (`delegate`, `call_fish`).
    Research,
    /// No delegation tools at all.
    Off,
}

impl DelegationMode {
    pub fn allows_research(self) -> bool {
        self != Self::Off
    }
    pub fn allows_task(self) -> bool {
        self == Self::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationConfig {
    #[serde(default)]
    pub mode: DelegationMode,
    /// Maximum sub-agents running at once across the app.
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
}

fn default_max_concurrent() -> u32 {
    DEFAULT_MAX_CONCURRENT
}

impl Default for DelegationConfig {
    fn default() -> Self {
        Self {
            mode: DelegationMode::Auto,
            max_concurrent: DEFAULT_MAX_CONCURRENT,
        }
    }
}

fn config_path(dir: &Path) -> PathBuf {
    dir.join("delegation.json")
}

pub fn load_config(dir: &Path) -> DelegationConfig {
    let mut cfg: DelegationConfig = std::fs::read_to_string(config_path(dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    cfg.max_concurrent = cfg.max_concurrent.clamp(1, 16);
    cfg
}

#[tauri::command]
pub async fn delegation_get(app: AppHandle) -> Result<DelegationConfig, String> {
    Ok(load_config(&resolve_global_config_dir(&app)?))
}

#[tauri::command]
pub async fn delegation_set(
    app: AppHandle,
    config: DelegationConfig,
) -> Result<DelegationConfig, String> {
    let dir = resolve_global_config_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut cfg = config;
    cfg.max_concurrent = cfg.max_concurrent.clamp(1, 16);
    let text = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(config_path(&dir), text).map_err(|e| e.to_string())?;
    Ok(cfg)
}

#[tauri::command]
pub async fn delegation_metrics() -> Result<DelegationMetrics, String> {
    Ok(metrics_snapshot())
}

// ── Prompt context (T2.7) ───────────────────────────────────────────────────

/// Lists delegable agents (and the builtin read-only fallbacks) for the main
/// agent's system prompt. Returns `None` when delegation tools are not offered.
pub fn delegable_agents_context(config_dir: &Path) -> Option<String> {
    let cfg = load_config(config_dir);
    if !cfg.mode.allows_task() {
        return None;
    }
    let manifests = crate::commands::agents::load_all_manifests(&config_dir.join("agents"));
    let mut lines: Vec<String> = manifests
        .iter()
        .filter(|m| m.delegable)
        .map(|m| {
            let mode = crate::runtime::subagent_policy::PermissionMode::parse(&m.permission_mode);
            format!(
                "- `{}` ({}) [{}]: {}",
                m.id,
                m.name,
                if mode == crate::runtime::subagent_policy::PermissionMode::ReadOnly {
                    "read-only"
                } else {
                    "can edit files and run commands (isolated copy, merged back on finish)"
                },
                m.description
            )
        })
        .collect();
    if lines.is_empty() {
        lines.push(
            "- (no registered agent is delegable) use the builtin read-only presets `scout`, \
             `summarizer`, `extractor` as `agent_id`."
                .to_string(),
        );
    }
    Some(format!(
        "## Delegable agents (`task` tool)\n{}\n",
        lines.join("\n")
    ))
}

// ── Concurrency cap (T2.8) ──────────────────────────────────────────────────

static ACTIVE: AtomicU32 = AtomicU32::new(0);

#[derive(Debug)]
pub struct ActiveGuard;

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Reserve a sub-agent slot, or return a readable error when `max` are running.
pub fn try_acquire(max: u32) -> Result<ActiveGuard, String> {
    let max = max.max(1);
    let mut cur = ACTIVE.load(Ordering::SeqCst);
    loop {
        if cur >= max {
            return Err(format!(
                "too many sub-agents running ({cur}/{max}); wait for some to finish or do this step yourself"
            ));
        }
        match ACTIVE.compare_exchange(cur, cur + 1, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => return Ok(ActiveGuard),
            Err(actual) => cur = actual,
        }
    }
}

// ── Result cache (T4.2) ─────────────────────────────────────────────────────

pub struct DelegationCache {
    entries: HashMap<String, (String, Instant, String)>,
}

impl Default for DelegationCache {
    fn default() -> Self {
        Self::new()
    }
}

impl DelegationCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Hit only when the workspace stamp is unchanged and the entry is fresh.
    pub fn get(&mut self, key: &str, stamp: &str, now: Instant) -> Option<String> {
        let fresh = match self.entries.get(key) {
            Some((s, at, _)) => s == stamp && now.duration_since(*at) < CACHE_TTL,
            None => return None,
        };
        if fresh {
            self.entries.get(key).map(|(_, _, v)| v.clone())
        } else {
            self.entries.remove(key);
            None
        }
    }

    pub fn put(&mut self, key: String, stamp: String, value: String, now: Instant) {
        if self.entries.len() >= CACHE_CAP {
            if let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, (_, at, _))| *at)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(key, (stamp, now, value));
    }
}

pub fn global_cache() -> &'static Mutex<DelegationCache> {
    static CACHE: OnceLock<Mutex<DelegationCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(DelegationCache::new()))
}

pub fn cache_key(agent_id: &str, brief: &str, workspace: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    agent_id.hash(&mut h);
    brief.trim().hash(&mut h);
    workspace.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// Workspace version stamp: git HEAD, dirty file list and their mtimes.
/// `None` (no caching) for non-git or remote workspaces.
pub fn workspace_stamp(workspace: &str) -> Option<String> {
    if crate::remote::is_remote(workspace) {
        return None;
    }
    let git = |args: &[&str]| -> Option<String> {
        let mut cmd = std::process::Command::new("git");
        cmd.args(args).current_dir(workspace);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let out = cmd.output().ok()?;
        if !out.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&out.stdout).to_string())
    };
    let head = git(&["rev-parse", "HEAD"])?;
    let status = git(&["status", "--porcelain"]).unwrap_or_default();
    let mut stamp = format!("{}|{}", head.trim(), status);
    for line in status.lines() {
        let path = line.get(3..).unwrap_or("").trim().trim_matches('"');
        if path.is_empty() {
            continue;
        }
        if let Ok(meta) = std::fs::metadata(Path::new(workspace).join(path)) {
            if let Ok(m) = meta.modified() {
                if let Ok(d) = m.duration_since(std::time::UNIX_EPOCH) {
                    stamp.push_str(&format!("|{path}:{}", d.as_nanos()));
                }
            }
        }
    }
    Some(stamp)
}

// ── Failure explanation (T4.3) ──────────────────────────────────────────────

/// Turn a sub-agent failure into a readable message for the parent agent.
pub fn explain_failure(err: &str) -> String {
    let l = err.to_ascii_lowercase();
    if l.contains("budget") || l.contains("quota") || l.contains("reserv") {
        format!(
            "delegation refused: the task budget is exhausted ({err}). Do not delegate further; \
             finish with what you have or ask the user to raise the budget."
        )
    } else {
        format!("task failed: {err}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_gating() {
        assert!(DelegationMode::Auto.allows_task());
        assert!(DelegationMode::Auto.allows_research());
        assert!(!DelegationMode::Research.allows_task());
        assert!(DelegationMode::Research.allows_research());
        assert!(!DelegationMode::Off.allows_research());
        assert!(!DelegationMode::Off.allows_task());
    }

    #[test]
    fn config_defaults_and_clamp() {
        let dir = std::env::temp_dir().join(format!("agentz-deleg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load_config(&dir).mode, DelegationMode::Auto);
        std::fs::write(
            dir.join("delegation.json"),
            r#"{"mode":"off","max_concurrent":99}"#,
        )
        .unwrap();
        let c = load_config(&dir);
        assert_eq!(c.mode, DelegationMode::Off);
        assert_eq!(c.max_concurrent, 16);
        assert!(delegable_agents_context(&dir).is_none(), "off hides the agent list");
        std::fs::write(dir.join("delegation.json"), r#"{"mode":"research"}"#).unwrap();
        assert!(delegable_agents_context(&dir).is_none(), "research hides task agents");
        std::fs::write(dir.join("delegation.json"), r#"{}"#).unwrap();
        let ctx = delegable_agents_context(&dir).expect("auto lists agents");
        assert!(ctx.contains("scout"), "falls back to builtin presets");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delegable_agents_are_listed_with_flags() {
        let dir = std::env::temp_dir().join(format!("agentz-deleg2-{}", std::process::id()));
        let adir = dir.join("agents").join("rev");
        std::fs::create_dir_all(&adir).unwrap();
        std::fs::write(
            adir.join("agent.json"),
            r#"{"id":"rev","name":"Reviewer","description":"reviews","delegable":true}"#,
        )
        .unwrap();
        let other = dir.join("agents").join("nope");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("agent.json"), r#"{"id":"nope","name":"Nope"}"#).unwrap();
        let ctx = delegable_agents_context(&dir).unwrap();
        assert!(ctx.contains("`rev`") && ctx.contains("read-only"));
        assert!(!ctx.contains("`nope`"), "non-delegable agents stay hidden");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrency_cap_enforced_and_released() {
        let a = try_acquire(2).unwrap();
        let b = try_acquire(2).unwrap();
        assert!(try_acquire(2).unwrap_err().contains("too many"));
        drop(a);
        let c = try_acquire(2).expect("slot freed");
        drop(b);
        drop(c);
    }

    #[test]
    fn cache_hits_only_on_same_stamp() {
        let mut c = DelegationCache::new();
        let t0 = Instant::now();
        c.put("k".into(), "v1".into(), "report".into(), t0);
        assert_eq!(c.get("k", "v1", t0).as_deref(), Some("report"));
        assert!(c.get("k", "v2", t0).is_none(), "changed files invalidate");
        assert!(c.get("k", "v1", t0).is_none(), "stale entry was evicted");
    }

    #[test]
    fn cache_expires_after_ttl() {
        let mut c = DelegationCache::new();
        let t0 = Instant::now();
        c.put("k".into(), "s".into(), "r".into(), t0);
        assert!(c.get("k", "s", t0 + CACHE_TTL + Duration::from_secs(1)).is_none());
    }

    #[test]
    fn cache_key_differs_by_inputs() {
        assert_ne!(cache_key("a", "x", "w"), cache_key("b", "x", "w"));
        assert_ne!(cache_key("a", "x", "w"), cache_key("a", "y", "w"));
        assert_eq!(cache_key("a", " x ", "w"), cache_key("a", "x", "w"));
    }

    #[test]
    fn budget_failures_get_readable_message() {
        assert!(explain_failure("task budget exhausted").contains("delegation refused"));
        assert!(explain_failure("network down").starts_with("task failed"));
    }
}
