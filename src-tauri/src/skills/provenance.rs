//! Skill lifecycle, paths, and write-guard policy for the evolution system.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const LIFECYCLE_BUILTIN: &str = "builtin";
pub const LIFECYCLE_INSTALLED: &str = "installed";
pub const LIFECYCLE_DRAFT: &str = "draft";
pub const LIFECYCLE_LEARNED: &str = "learned";
pub const LIFECYCLE_ARCHIVED: &str = "archived";

pub const SUBDIR_INSTALLED: &str = "installed";
pub const SUBDIR_DRAFT: &str = ".draft";
pub const SUBDIR_LEARNED: &str = "learned";
pub const SUBDIR_ARCHIVE: &str = ".archive";
pub const SUBDIR_CURATOR_BACKUPS: &str = ".curator_backups";
pub const SUBDIR_HUB: &str = ".hub";

const RESERVED_DIR_NAMES: &[&str] = &[
    SUBDIR_INSTALLED,
    SUBDIR_LEARNED,
    SUBDIR_DRAFT,
    SUBDIR_ARCHIVE,
    SUBDIR_CURATOR_BACKUPS,
    SUBDIR_HUB,
];

/// JSON stored in `skills.config` column.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillConfigMeta {
    #[serde(default = "default_lifecycle_installed")]
    pub lifecycle: String,
    #[serde(default = "default_locked_true")]
    pub locked: bool,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub registry_version: Option<String>,
    #[serde(default)]
    pub content_sha256: Option<String>,
    /// Tools requested by this skill. These are surfaced as capabilities and
    /// are not persisted as a blanket user grant.
    #[serde(default)]
    pub requested_tools: Vec<String>,
    #[serde(default)]
    pub install_history: Vec<SkillInstallRecord>,
    #[serde(default)]
    pub installed_from_session_id: Option<String>,
    #[serde(default)]
    pub promoted_at: Option<String>,
}

fn default_lifecycle_installed() -> String {
    LIFECYCLE_INSTALLED.to_string()
}

fn default_locked_true() -> bool {
    true
}

impl SkillConfigMeta {
    pub fn installed(source: &str, source_url: Option<String>, session_id: Option<String>) -> Self {
        Self {
            lifecycle: LIFECYCLE_INSTALLED.to_string(),
            locked: true,
            pinned: false,
            source: Some(source.to_string()),
            source_url,
            version: None,
            registry_version: None,
            content_sha256: None,
            requested_tools: Vec::new(),
            install_history: Vec::new(),
            installed_from_session_id: session_id,
            promoted_at: None,
        }
    }

    pub fn draft(created_by: &str, session_id: Option<String>) -> Self {
        Self {
            lifecycle: LIFECYCLE_DRAFT.to_string(),
            locked: false,
            pinned: false,
            source: Some(created_by.to_string()),
            source_url: None,
            version: None,
            registry_version: None,
            content_sha256: None,
            requested_tools: Vec::new(),
            install_history: Vec::new(),
            installed_from_session_id: session_id,
            promoted_at: None,
        }
    }

    pub fn learned(from_draft: &Self) -> Self {
        let mut m = from_draft.clone();
        m.lifecycle = LIFECYCLE_LEARNED.to_string();
        m.promoted_at = Some(chrono::Utc::now().to_rfc3339());
        m
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn from_json(s: &str) -> Self {
        serde_json::from_str(s).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInstallRecord {
    pub action: String,
    pub at: String,
    pub source: String,
    pub source_url: Option<String>,
    pub version: Option<String>,
    pub registry_version: Option<String>,
    pub content_sha256: String,
    pub requested_tools: Vec<String>,
    pub added_tools: Vec<String>,
    pub removed_tools: Vec<String>,
}

pub fn skills_root(config_dir: &Path) -> PathBuf {
    config_dir.join("skills")
}

pub fn installed_dir(root: &Path) -> PathBuf {
    root.join(SUBDIR_INSTALLED)
}

pub fn draft_dir(root: &Path) -> PathBuf {
    root.join(SUBDIR_DRAFT)
}

pub fn learned_dir(root: &Path) -> PathBuf {
    root.join(SUBDIR_LEARNED)
}

pub fn archive_dir(root: &Path) -> PathBuf {
    root.join(SUBDIR_ARCHIVE)
}

pub fn hub_lock_path(root: &Path) -> PathBuf {
    root.join(SUBDIR_HUB).join("lock.json")
}

pub fn ensure_evolution_dirs(root: &Path) -> std::io::Result<()> {
    for d in [
        root,
        &installed_dir(root),
        &draft_dir(root),
        &learned_dir(root),
        &archive_dir(root),
        &root.join(SUBDIR_CURATOR_BACKUPS),
        &root.join(SUBDIR_HUB),
    ] {
        std::fs::create_dir_all(d)?;
    }
    Ok(())
}

pub fn is_reserved_dir_name(name: &str) -> bool {
    name.starts_with('.') || RESERVED_DIR_NAMES.contains(&name)
}

pub fn infer_lifecycle_from_path(_skills_root: &Path, skill_md: &Path) -> String {
    let path_str = skill_md.to_string_lossy();
    if path_str.contains("/.draft/") || path_str.contains("\\.draft\\") {
        return LIFECYCLE_DRAFT.to_string();
    }
    if path_str.contains("/learned/") || path_str.contains("\\learned\\") {
        return LIFECYCLE_LEARNED.to_string();
    }
    if path_str.contains("/installed/") || path_str.contains("\\installed\\") {
        return LIFECYCLE_INSTALLED.to_string();
    }
    if path_str.contains("/.archive/") || path_str.contains("\\.archive\\") {
        return LIFECYCLE_ARCHIVED.to_string();
    }
    LIFECYCLE_INSTALLED.to_string()
}

pub fn resolve_skill_dir(root: &Path, skill_id: &str, lifecycle: &str) -> PathBuf {
    match lifecycle {
        LIFECYCLE_DRAFT => draft_dir(root).join(skill_id),
        LIFECYCLE_LEARNED => learned_dir(root).join(skill_id),
        LIFECYCLE_ARCHIVED => archive_dir(root).join(skill_id),
        LIFECYCLE_INSTALLED => installed_dir(root).join(skill_id),
        _ => installed_dir(root).join(skill_id),
    }
}

pub fn find_skill_md(root: &Path, skill_id: &str) -> Option<PathBuf> {
    let candidates = [
        draft_dir(root).join(skill_id).join("SKILL.md"),
        learned_dir(root).join(skill_id).join("SKILL.md"),
        installed_dir(root).join(skill_id).join("SKILL.md"),
        root.join(skill_id).join("SKILL.md"),
        archive_dir(root).join(skill_id).join("SKILL.md"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

pub fn is_writable(meta: &SkillConfigMeta, hub_locked: bool) -> bool {
    if hub_locked {
        return false;
    }
    if meta.locked || meta.pinned {
        return false;
    }
    matches!(meta.lifecycle.as_str(), LIFECYCLE_DRAFT | LIFECYCLE_LEARNED)
}

pub fn is_hub_locked(root: &Path, skill_id: &str) -> bool {
    let lock_path = hub_lock_path(root);
    if !lock_path.exists() {
        return false;
    }
    let Ok(raw) = std::fs::read_to_string(&lock_path) else {
        return false;
    };
    let Ok(ids) = serde_json::from_str::<Vec<String>>(&raw) else {
        return false;
    };
    ids.iter().any(|id| id == skill_id)
}

pub fn add_hub_lock(root: &Path, skill_id: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(root.join(SUBDIR_HUB))?;
    let lock_path = hub_lock_path(root);
    let mut ids: Vec<String> = if lock_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&lock_path)?).unwrap_or_default()
    } else {
        vec![]
    };
    if !ids.iter().any(|id| id == skill_id) {
        ids.push(skill_id.to_string());
        std::fs::write(
            &lock_path,
            serde_json::to_string_pretty(&ids).unwrap_or_default(),
        )?;
    }
    Ok(())
}

pub fn remove_hub_lock(root: &Path, skill_id: &str) -> std::io::Result<()> {
    let lock_path = hub_lock_path(root);
    if !lock_path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&lock_path)?;
    let mut ids: Vec<String> = serde_json::from_str(&raw)?;
    ids.retain(|id| id != skill_id);
    if ids.is_empty() {
        std::fs::remove_file(lock_path)?;
    } else {
        std::fs::write(lock_path, serde_json::to_vec_pretty(&ids)?)?;
    }
    Ok(())
}

#[derive(Serialize)]
struct SkillAuthorizationEvent<'a> {
    at: String,
    action: &'static str,
    skill_id: &'a str,
    source: Option<&'a str>,
    source_url: Option<&'a str>,
    version: Option<&'a str>,
    registry_version: Option<&'a str>,
    content_sha256: Option<&'a str>,
    requested_tools: &'a [String],
}

/// Record a durable uninstall intent before deleting the skill metadata that
/// describes its requested capabilities. An intent event remains accurate if
/// deletion subsequently fails; it does not claim deletion already succeeded.
pub fn record_skill_revocation_intent(
    root: &Path,
    skill_id: &str,
    meta: &SkillConfigMeta,
) -> std::io::Result<()> {
    use std::io::Write;
    let dir = root.join(SUBDIR_HUB);
    std::fs::create_dir_all(&dir)?;
    let event = SkillAuthorizationEvent {
        at: chrono::Utc::now().to_rfc3339(),
        action: "revocation_requested",
        skill_id,
        source: meta.source.as_deref(),
        source_url: meta.source_url.as_deref(),
        version: meta.version.as_deref(),
        registry_version: meta.registry_version.as_deref(),
        content_sha256: meta.content_sha256.as_deref(),
        requested_tools: &meta.requested_tools,
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("authorization-events.jsonl"))?;
    let encoded = serde_json::to_vec(&event).map_err(std::io::Error::other)?;
    file.write_all(&encoded)?;
    file.write_all(b"\n")?;
    file.sync_data()
}

/// One-time migration: move flat `skills/{name}/` → `skills/installed/{name}/`.
pub fn migrate_flat_skills_to_installed(root: &Path) -> std::io::Result<u32> {
    ensure_evolution_dirs(root)?;
    let marker = root.join(".migrated_to_quadrants");
    if marker.exists() {
        return Ok(0);
    }
    let mut moved = 0u32;
    let entries = std::fs::read_dir(root)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_reserved_dir_name(&name) {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }
        let dest_parent = installed_dir(root).join(&name);
        if dest_parent.exists() {
            continue;
        }
        std::fs::rename(&path, &dest_parent)?;
        moved += 1;
    }
    std::fs::write(&marker, format!("migrated {} skills\n", moved))?;
    Ok(moved)
}

pub fn content_hash(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    content.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub fn content_sha256(content: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

/// Capabilities newly requested by a candidate relative to the installed one.
/// Sorting and de-duplication keep comparisons stable and auditable.
pub fn added_tools(previous: &[String], candidate: &[String]) -> Vec<String> {
    let previous: std::collections::BTreeSet<_> = previous.iter().map(String::as_str).collect();
    candidate
        .iter()
        .filter(|tool| !previous.contains(tool.as_str()))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn removed_tools(previous: &[String], candidate: &[String]) -> Vec<String> {
    added_tools(candidate, previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_writable_matrix() {
        let draft = SkillConfigMeta::draft("agent", None);
        assert!(is_writable(&draft, false));
        assert!(!is_writable(&draft, true));

        let installed = SkillConfigMeta::installed("clawhub", None, None);
        assert!(!is_writable(&installed, false));
    }

    #[test]
    fn content_sha256_uses_standard_digest_and_tool_diff_is_stable() {
        assert_eq!(
            content_sha256("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            added_tools(
                &["read_file".into()],
                &["write_file".into(), "read_file".into(), "write_file".into()]
            ),
            vec!["write_file"]
        );
        assert!(added_tools(&["read_file".into()], &["read_file".into()]).is_empty());
    }

    #[test]
    fn legacy_metadata_loads_without_provenance_as_unverified_defaults() {
        let meta = SkillConfigMeta::from_json(r#"{"lifecycle":"installed","locked":true}"#);
        assert_eq!(meta.lifecycle, LIFECYCLE_INSTALLED);
        assert!(meta.source.is_none());
        assert!(meta.content_sha256.is_none());
        assert!(meta.requested_tools.is_empty());
        assert!(meta.install_history.is_empty());
    }

    #[test]
    fn revocation_intent_keeps_audit_and_cleans_only_its_hub_lock() {
        let root = std::env::temp_dir().join(format!(
            "agentz-skill-revoke-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).expect("create fixture");
        let mut meta =
            SkillConfigMeta::installed("clawhub", Some("https://example.test/skill".into()), None);
        meta.version = Some("2.4.0".into());
        meta.content_sha256 = Some(content_sha256("skill"));
        meta.requested_tools = vec!["read_file".into(), "write_file".into()];
        add_hub_lock(&root, "target").expect("lock target");
        add_hub_lock(&root, "other").expect("lock other");

        record_skill_revocation_intent(&root, "target", &meta).expect("write audit event");
        remove_hub_lock(&root, "target").expect("remove target lock");

        let event: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(SUBDIR_HUB).join("authorization-events.jsonl"))
                .expect("read audit event"),
        )
        .expect("valid event");
        assert_eq!(event["action"], "revocation_requested");
        assert_eq!(event["skill_id"], "target");
        assert_eq!(event["requested_tools"][1], "write_file");
        let remaining: Vec<String> =
            serde_json::from_slice(&std::fs::read(hub_lock_path(&root)).expect("read lock file"))
                .expect("valid lock file");
        assert_eq!(remaining, vec!["other"]);
        let _ = std::fs::remove_dir_all(root);
    }
}
