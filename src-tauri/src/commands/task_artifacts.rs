//! Scoped immutable text snapshots. No HTML execution or renderer-supplied filesystem roots.
use crate::state::AppState;
use agentz_task_state::{
    artifacts::{Artifact, ArtifactStore, Capture, CaptureInput, Delete, Kind, Metadata, Page},
    plan::PlanBinding,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, State, WebviewWindow};

#[derive(Serialize)]
pub struct ArtifactList {
    pub scope: PlanBinding,
    pub store_missing: bool,
    pub page: Page,
}
#[derive(Serialize)]
pub struct ArtifactText {
    pub scope: PlanBinding,
    pub metadata: Metadata,
    pub content: String,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeleteResult {
    Deleted,
    AlreadyDeleted,
    NotFound,
}

#[derive(Debug, Deserialize)]
pub struct ArtifactContinuationRef {
    pub artifact_id: String,
    pub version: i64,
    pub payload_sha256: String,
    pub expected_binding: PlanBinding,
}

fn continuation_excerpt(
    artifact: Artifact,
    reference: &ArtifactContinuationRef,
) -> Result<String, String> {
    if artifact.metadata.artifact_id != reference.artifact_id
        || artifact.metadata.version != reference.version
        || artifact.metadata.binding.project_id != reference.expected_binding.project_id
        || artifact.metadata.binding.task_id != reference.expected_binding.task_id
        || artifact.metadata.binding.environment_id != reference.expected_binding.environment_id
        || !matches!(artifact.metadata.kind, Kind::Document | Kind::Report)
        || artifact.metadata.payload_sha256 != reference.payload_sha256
    {
        return Err("artifact continuation version changed".into());
    }
    let content = String::from_utf8(artifact.payload)
        .map_err(|_| "artifact continuation is not UTF-8 text")?;
    let excerpt: String = content.chars().take(12_000).collect();
    let truncated = content.chars().count() > 12_000;
    Ok(format!(
        "[Task artifact {} v{}; SHA-256 {}{}]\n{}",
        reference.artifact_id,
        reference.version,
        reference.payload_sha256,
        if truncated { "; excerpt truncated" } else { "" },
        serde_json::to_string(&excerpt).map_err(|e| e.to_string())?
    ))
}

/// Resolve the immutable version again at send time. The caller already owns the
/// environment execution lease, so this helper must not acquire it recursively.
pub async fn resolve_continuation_text(
    app: &AppHandle,
    window: &WebviewWindow,
    state: &AppState,
    handle: &str,
    project: &Path,
    session: &str,
    reference: &ArtifactContinuationRef,
) -> Result<String, String> {
    if reference.version < 1
        || reference.artifact_id.is_empty()
        || reference.artifact_id.len() > 256
        || !reference
            .artifact_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        || reference.payload_sha256.len() != 64
        || !reference
            .payload_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid artifact continuation reference".into());
    }
    let scope = super::project_scope::lookup_for_main(state, window, handle)?;
    if scope.project() != project {
        return Err("artifact continuation project changed".into());
    }
    let captured =
        super::task_view::resolve_observation(app, project, session, scope.source()).await?;
    if captured.binding != reference.expected_binding {
        return Err("stale artifact continuation binding".into());
    }
    super::project_scope::with_verified_scope(state, window, handle, |verified| {
        if verified.project() != project {
            return Err("artifact continuation project changed".into());
        }
        super::task_view::verify_observation(project, session, &captured)?;
        let mut store =
            ArtifactStore::open_existing(project, false)?.ok_or("artifact store is missing")?;
        let artifact = store
            .read_version(&captured.binding, &reference.artifact_id, reference.version)?
            .ok_or("artifact continuation version is missing or deleted")?;
        let excerpt = continuation_excerpt(artifact, reference)?;
        super::task_view::verify_observation(project, session, &captured)?;
        Ok(excerpt)
    })
}

async fn authorized<T, F>(
    app: &AppHandle,
    window: &WebviewWindow,
    state: &AppState,
    handle: &str,
    session: &str,
    expected: &PlanBinding,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce(&Path, &super::task_view::CapturedView) -> Result<T, String>,
{
    let _lease = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(state, window, handle)?;
    let captured =
        super::task_view::resolve_observation(app, scope.project(), session, scope.source())
            .await?;
    if captured.binding != *expected {
        return Err("stale artifact task binding".into());
    }
    super::project_scope::with_verified_scope(state, window, handle, |verified| {
        if verified.project() != scope.project() {
            return Err("artifact project scope changed".into());
        }
        super::task_view::verify_observation(verified.project(), session, &captured)?;
        let result = operation(verified.project(), &captured)?;
        super::task_view::verify_observation(verified.project(), session, &captured)?;
        Ok(result)
    })
}
fn artifact_id(relative: &str) -> String {
    format!("path.{}", hex::encode(Sha256::digest(relative.as_bytes())))
}
fn clock() -> Result<i64, String> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| "invalid artifact clock")?
            .as_millis(),
    )
    .map_err(|_| "artifact clock overflow".into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResearchReport {
    schema_version: u32,
    task_id: String,
    question: String,
    conclusion: String,
    findings: Vec<ReportFinding>,
    sources: Vec<ReportSource>,
    limitations: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportFinding {
    claim: String,
    source_ids: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportSource {
    id: String,
    title: String,
    locator: String,
}

fn report_text(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= max
        && !value.chars().any(char::is_control)
}
fn report_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
fn report_locator(value: &str) -> bool {
    if !report_text(value, 4096) {
        return false;
    }
    if let Some(rest) = value.strip_prefix("https://") {
        return !rest.is_empty() && !rest.chars().any(char::is_whitespace);
    }
    let local = value
        .strip_prefix("workspace:")
        .or_else(|| value.strip_prefix("artifact:"));
    local.is_some_and(|path| {
        !path.is_empty()
            && path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..")
    })
}
fn report_prose(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        if "\\`*_{}[]()#+-.!|".contains(ch) {
            escaped.push('\\');
        }
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}
fn render_research_report(json: &str, bound_task: &str) -> Result<String, String> {
    let report: ResearchReport =
        serde_json::from_str(json).map_err(|_| "invalid research report JSON")?;
    if report.schema_version != 1
        || report.task_id != bound_task
        || !report_id(&report.task_id)
        || !report_text(&report.question, 8192)
        || !report_text(&report.conclusion, 8192)
        || report.sources.is_empty()
        || report.sources.len() > 100
        || report.findings.is_empty()
        || report.findings.len() > 100
        || report.limitations.is_empty()
        || report.limitations.len() > 100
        || !report.limitations.iter().all(|v| report_text(v, 8192))
    {
        return Err("research report does not match the task contract".into());
    }
    let mut ids = std::collections::HashSet::new();
    for source in &report.sources {
        if !report_id(&source.id)
            || !ids.insert(source.id.as_str())
            || !report_text(&source.title, 1024)
            || !report_locator(&source.locator)
        {
            return Err("research report has an invalid source locator".into());
        }
    }
    for finding in &report.findings {
        if !report_text(&finding.claim, 8192)
            || finding.source_ids.is_empty()
            || finding
                .source_ids
                .iter()
                .any(|id| !ids.contains(id.as_str()))
        {
            return Err("research report finding lacks a known source".into());
        }
    }
    let mut lines = vec![
        "# Research report".to_string(),
        String::new(),
        format!("Task: {}", report_prose(&report.task_id)),
        String::new(),
        "## Question".into(),
        String::new(),
        report_prose(&report.question),
        String::new(),
        "## Conclusion".into(),
        String::new(),
        report_prose(&report.conclusion),
        String::new(),
        "## Findings".into(),
        String::new(),
    ];
    for finding in &report.findings {
        lines.push(format!(
            "- {} ({})",
            report_prose(&finding.claim),
            finding
                .source_ids
                .iter()
                .map(|id| format!("[{id}]"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.extend([String::new(), "## Sources".into(), String::new()]);
    for source in &report.sources {
        lines.push(format!(
            "- [{}] {} — {}",
            source.id,
            report_prose(&source.title),
            report_prose(&source.locator)
        ));
    }
    lines.extend([String::new(), "## Limitations".into(), String::new()]);
    for limitation in &report.limitations {
        lines.push(format!("- {}", report_prose(limitation)));
    }
    let rendered = format!("{}\n", lines.join("\n"));
    if rendered.len() > 1_048_576 {
        return Err("rendered research report is too large".into());
    }
    Ok(rendered)
}

#[tauri::command]
pub async fn task_artifact_capture(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected_binding: PlanBinding,
    relative_path: String,
    kind: Kind,
    title: String,
) -> Result<Metadata, String> {
    authorized(
        &app,
        &window,
        &state,
        &scope_handle,
        &session_id,
        &expected_binding,
        |project, captured| {
            let (content, _) =
                super::task_view_read::read_captured_text(&captured.root, &relative_path)?;
            let (content, media_type) =
                if kind == Kind::Report && relative_path.to_ascii_lowercase().ends_with(".json") {
                    (
                        render_research_report(&content, &captured.binding.task_id)?,
                        "text/markdown; charset=utf-8",
                    )
                } else {
                    (content, "text/plain; charset=utf-8")
                };
            super::task_view::verify_observation(project, &session_id, captured)?;
            let mut store = ArtifactStore::initialize(project)?;
            let id = artifact_id(&relative_path);
            // The version number is assigned inside the insert transaction (`capture_next`),
            // so two captures of the same file cannot race onto one number.
            let input = CaptureInput {
                artifact_id: id,
                version: 0,
                kind,
                title,
                source_relative_path: Some(relative_path),
                media_type: media_type.into(),
                created_ms: clock()?,
            };
            match store.capture_next(&captured.binding, input, content.as_bytes())? {
                Capture::Created(m) | Capture::Existing(m) => Ok(m),
            }
        },
    )
    .await
}

/// Commit a reviewed text edit or restoration as a new immutable version.
/// Renderer paths are not accepted; the host re-resolves the current task binding.
#[tauri::command]
pub async fn task_artifact_save_text(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected_binding: PlanBinding,
    artifact_id: String,
    expected_latest: i64,
    content: String,
) -> Result<Metadata, String> {
    if expected_latest < 1
        || content.is_empty()
        || content.len() > 1_048_576
        || content.contains('\0')
    {
        return Err("invalid artifact edit".into());
    }
    authorized(
        &app,
        &window,
        &state,
        &scope_handle,
        &session_id,
        &expected_binding,
        |project, captured| {
            let mut store =
                ArtifactStore::open_existing(project, false)?.ok_or("artifact store is missing")?;
            let current = store
                .read_version(&captured.binding, &artifact_id, expected_latest)?
                .ok_or("artifact version is missing or deleted")?;
            if !matches!(&current.metadata.kind, Kind::Document | Kind::Report) {
                return Err("only documents and reports support human text revisions".into());
            }
            let input = CaptureInput {
                artifact_id,
                version: 0,
                kind: current.metadata.kind,
                title: current.metadata.title,
                source_relative_path: None,
                media_type: "text/plain; charset=utf-8".into(),
                created_ms: clock()?,
            };
            match store.capture_next_if_latest(
                &captured.binding,
                input,
                content.as_bytes(),
                expected_latest,
            )? {
                Capture::Created(metadata) | Capture::Existing(metadata) => Ok(metadata),
            }
        },
    )
    .await
}
#[tauri::command]
pub async fn task_artifact_list(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected_binding: PlanBinding,
    after: Option<(String, i64)>,
    limit: usize,
) -> Result<ArtifactList, String> {
    if !(1..=200).contains(&limit) {
        return Err("artifact list limit must be 1..200".into());
    }
    authorized(
        &app,
        &window,
        &state,
        &scope_handle,
        &session_id,
        &expected_binding,
        |project, captured| {
            let mut store = ArtifactStore::open_existing(project, true)?;
            let missing = store.is_none();
            let page = match store.as_mut() {
                Some(s) => s.list_versions(&captured.binding, after, limit)?,
                None => {
                    if after.is_some() {
                        return Err("artifact cursor supplied for missing store".into());
                    }
                    Page {
                        versions: vec![],
                        next_cursor: None,
                        has_more: false,
                    }
                }
            };
            Ok(ArtifactList {
                scope: captured.binding.clone(),
                store_missing: missing,
                page,
            })
        },
    )
    .await
}
#[tauri::command]
pub async fn task_artifact_read(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected_binding: PlanBinding,
    artifact_id: String,
    version: i64,
) -> Result<Option<ArtifactText>, String> {
    if version < 1 {
        return Err("invalid artifact version".into());
    }
    authorized(
        &app,
        &window,
        &state,
        &scope_handle,
        &session_id,
        &expected_binding,
        |project, captured| {
            let Some(mut store) = ArtifactStore::open_existing(project, true)? else {
                return Ok(None);
            };
            let Some(artifact) = store.read_version(&captured.binding, &artifact_id, version)?
            else {
                return Ok(None);
            };
            let content = String::from_utf8(artifact.payload)
                .map_err(|_| "artifact version is not UTF-8 text")?;
            Ok(Some(ArtifactText {
                scope: captured.binding.clone(),
                metadata: artifact.metadata,
                content,
            }))
        },
    )
    .await
}
#[tauri::command]
pub async fn task_artifact_delete(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected_binding: PlanBinding,
    artifact_id: String,
    version: i64,
) -> Result<DeleteResult, String> {
    if version < 1 {
        return Err("invalid artifact version".into());
    }
    authorized(
        &app,
        &window,
        &state,
        &scope_handle,
        &session_id,
        &expected_binding,
        |project, captured| {
            let Some(mut store) = ArtifactStore::open_existing(project, false)? else {
                return Ok(DeleteResult::NotFound);
            };
            Ok(
                match store.delete_version(&captured.binding, &artifact_id, version)? {
                    Delete::Deleted => DeleteResult::Deleted,
                    Delete::AlreadyDeleted => DeleteResult::AlreadyDeleted,
                    Delete::NotFound => DeleteResult::NotFound,
                },
            )
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuation_uses_verified_immutable_version_and_bounded_excerpt() {
        let binding = PlanBinding {
            project_id: "project".into(),
            task_id: "task".into(),
            environment_id: "env".into(),
            binding_version: "a".repeat(64),
        };
        let content = "x".repeat(12_001);
        let digest = hex::encode(Sha256::digest(content.as_bytes()));
        let artifact = Artifact {
            metadata: Metadata {
                schema_version: 1,
                binding: binding.clone(),
                artifact_id: "report".into(),
                version: 2,
                kind: Kind::Report,
                title: "Report".into(),
                source_relative_path: None,
                media_type: "text/markdown; charset=utf-8".into(),
                payload_sha256: digest.clone(),
                bytes: content.len() as i64,
                created_ms: 1,
            },
            payload: content.into_bytes(),
        };
        let mut reference = ArtifactContinuationRef {
            artifact_id: "report".into(),
            version: 2,
            payload_sha256: digest,
            expected_binding: binding,
        };
        let excerpt = continuation_excerpt(artifact.clone(), &reference).unwrap();
        assert!(excerpt.contains("excerpt truncated"));
        assert!(!excerpt.contains(&"x".repeat(12_001)));
        reference.expected_binding.binding_version = "b".repeat(64);
        assert!(continuation_excerpt(artifact.clone(), &reference).is_ok());
        reference.version = 1;
        assert!(continuation_excerpt(artifact.clone(), &reference).is_err());
        reference.version = 2;
        reference.payload_sha256 = "0".repeat(64);
        assert!(continuation_excerpt(artifact.clone(), &reference).is_err());
        reference.payload_sha256 = artifact.metadata.payload_sha256.clone();
        reference.expected_binding.environment_id = "other".into();
        assert!(continuation_excerpt(artifact, &reference).is_err());
    }
    #[test]
    fn structured_report_requires_bound_task_and_known_locators() {
        let valid = serde_json::json!({
            "schema_version": 1,
            "task_id": "task-1",
            "question": "Which file?",
            "conclusion": "The <fixture> applies.",
            "findings": [{"claim": "See *fixture*", "source_ids": ["S1"]}],
            "sources": [{"id": "S1", "title": "Local [fixture]", "locator": "workspace:docs/fixture.md"}],
            "limitations": ["Only local evidence"]
        });
        let rendered = render_research_report(&valid.to_string(), "task-1").unwrap();
        assert!(rendered.contains("The &lt;fixture&gt; applies"));
        assert!(rendered.contains("See \\*fixture\\*"));
        assert!(rendered.contains("workspace:docs/fixture\\.md"));
        assert!(render_research_report(&valid.to_string(), "other-task").is_err());
        let mut traversal = valid.clone();
        traversal["sources"][0]["locator"] = "workspace:../secret".into();
        assert!(render_research_report(&traversal.to_string(), "task-1").is_err());
        let mut unknown = valid;
        unknown["findings"][0]["source_ids"] = serde_json::json!(["unknown"]);
        assert!(render_research_report(&unknown.to_string(), "task-1").is_err());
    }
    #[test]
    fn artifact_identity_is_host_path_specific_and_deletion_status_is_explicit() {
        assert_eq!(artifact_id("report.txt"), artifact_id("report.txt"));
        assert_ne!(artifact_id("Report.txt"), artifact_id("report.txt"));
        assert_eq!(
            serde_json::to_value(DeleteResult::AlreadyDeleted).unwrap(),
            "already_deleted"
        );
        assert_eq!(
            serde_json::to_value(DeleteResult::NotFound).unwrap(),
            "not_found"
        );
    }
    #[cfg(windows)]
    #[test]
    fn native_code_and_report_snapshots_keep_old_bytes_and_tombstones() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let folder =
            std::env::temp_dir().join(format!("native-artifacts-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap();
        let fixture = Fixture(std::fs::canonicalize(folder).unwrap());
        let scope = PlanBinding {
            project_id: agentz_task_state::canonical_root(&fixture.0).unwrap(),
            task_id: "controlled-task".into(),
            environment_id: "controlled-env".into(),
            binding_version: "a".repeat(64),
        };
        let mut store = ArtifactStore::initialize(&fixture.0).unwrap();
        for (relative, original, kind) in [
            (
                "sample.rs",
                "pub fn answer() -> u32 { 42 }\n",
                Kind::ChangeSet,
            ),
            (
                "report.md",
                "# Report\nScope: controlled fixture\n",
                Kind::Report,
            ),
        ] {
            let source = fixture.0.join(relative);
            std::fs::write(&source, original).unwrap();
            let (captured, bytes) =
                super::super::task_view_read::read_captured_text(&fixture.0, relative).unwrap();
            assert_eq!(captured, original);
            assert_eq!(bytes, original.len() as u64);
            let id = artifact_id(relative);
            let input = CaptureInput {
                artifact_id: id.clone(),
                version: 1,
                kind,
                title: relative.into(),
                source_relative_path: Some(relative.into()),
                media_type: "text/plain; charset=utf-8".into(),
                created_ms: 1,
            };
            assert!(matches!(
                store.capture(&scope, input, captured.as_bytes()).unwrap(),
                Capture::Created(_)
            ));
            std::fs::write(&source, "changed live content").unwrap();
            drop(store);
            store = ArtifactStore::open_existing(&fixture.0, false)
                .unwrap()
                .unwrap();
            let mut fresh = scope.clone();
            fresh.binding_version = "b".repeat(64);
            let old = store.read_version(&fresh, &id, 1).unwrap().unwrap();
            assert_eq!(old.payload, original.as_bytes());
            assert_eq!(old.metadata.binding, scope);
            for field in ["task", "environment"] {
                let mut wrong = scope.clone();
                if field == "task" {
                    wrong.task_id = "another-task".into();
                } else {
                    wrong.environment_id = "another-env".into();
                }
                assert!(store.read_version(&wrong, &id, 1).unwrap().is_none());
                assert!(matches!(
                    store.delete_version(&wrong, &id, 1).unwrap(),
                    Delete::NotFound
                ));
            }
            let mut wrong_project = scope.clone();
            wrong_project.project_id.push_str("-other");
            assert!(store.read_version(&wrong_project, &id, 1).is_err());
            assert!(matches!(
                store.delete_version(&fresh, &id, 1).unwrap(),
                Delete::Deleted
            ));
            assert!(store.read_version(&fresh, &id, 1).unwrap().is_none());
            assert!(matches!(
                store.delete_version(&fresh, &id, 1).unwrap(),
                Delete::AlreadyDeleted
            ));
            assert_eq!(
                std::fs::read_to_string(&source).unwrap(),
                "changed live content"
            );
            assert_eq!(store.next_version(&fresh, &id).unwrap(), 2);
        }
    }
}
