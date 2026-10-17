//! Read-only local detail service. Caller supplies host-verified scope and retains
//! its environment lease; this function neither authorizes renderer paths nor resumes work.
use agentz_task_state::{
    artifacts::ArtifactStore,
    budget::BudgetStore,
    plan::{PlanBinding, PlanStore},
    run::RunStore,
    task_details::{aggregate, ArtifactReferences, Source, Sources, TaskDetails},
};
use sha2::{Digest, Sha256};
use std::path::Path;

fn exists(path: &Path) -> Result<bool, String> {
    path.try_exists()
        .map_err(|e| format!("cannot inspect task detail source: {e}"))
}
fn available<T>(value: T, version: String) -> Source<T> {
    Source::Available { value, version }
}
/// Errors propagate as errors, never as Missing or fabricated empty/zero data.
/// Sources are independently read; atomic_snapshot is explicitly false.
pub fn read(project: &Path, binding: &PlanBinding) -> Result<TaskDetails, String> {
    if binding.project_id != agentz_task_state::canonical_root(project)?
        || binding.task_id.trim().is_empty()
        || binding.task_id.trim() != binding.task_id
        || binding.binding_version.len() != 64
        || !binding
            .binding_version
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err("invalid trusted task detail binding".into());
    }
    let environment = match agentz_task_state::load_existing_readonly(project, &binding.task_id)? {
        Some(e) if e.environment_id == binding.environment_id => {
            let version = format!(
                "environment-v{}:{}",
                e.schema_version, binding.binding_version
            );
            available(e, version)
        }
        // A record for another environment is stale evidence for this scope, not an error.
        Some(e) => Source::Stale {
            version: format!(
                "environment-mismatch-v{}:{}",
                e.schema_version, binding.binding_version
            ),
        },
        None if binding.environment_id == format!("direct:{}", binding.binding_version) => {
            Source::Missing
        }
        // A managed environment that was never registered is reported as unavailable.
        None => Source::Unavailable {
            version: format!("environment-unregistered:{}", binding.binding_version),
        },
    };
    let run_file = project.join(".agentz/runs.db");
    let (run, budget, events) = if exists(&run_file)? {
        let mut store = RunStore::open_existing_readonly(project)?;
        let (run, events) =
            match store.current_task_run(&binding.task_id, &binding.environment_id)? {
                Some(r) => {
                    let version = format!("run-v1:{}", r.updated_ms);
                    let cursor = store.latest_event_cursor(&r.run_id, &r.attempt_id)?;
                    let events = match cursor {
                        Some(cursor) => {
                            available(cursor.clone(), format!("event-v1:{}", cursor.sequence))
                        }
                        None => Source::Unknown,
                    };
                    (available(r, version), events)
                }
                None => (Source::Missing, Source::Unknown),
            };
        let mut store = BudgetStore::open_existing(project, true)?;
        let budget = match store.query(&binding.task_id)? {
            Some(b) => {
                let version = format!("observer-v1:{}:{}", b.policy.revision, b.usage_count);
                available(b, version)
            }
            None => Source::Missing,
        };
        (run, budget, events)
    } else {
        (Source::Missing, Source::Missing, Source::Unknown)
    };
    let plan = if exists(&project.join(".agentz/plans.db"))? {
        let store = PlanStore::open_existing_readonly(project)?;
        match store.continue_plan(binding)? {
            Some(p) => {
                let version = format!("plan-v1:{}", p.revision);
                available(p, version)
            }
            None => Source::Missing,
        }
    } else {
        Source::Missing
    };
    let artifacts = match ArtifactStore::open_existing(project, true)? {
        None => Source::Missing,
        Some(mut store) => {
            // A task detail is a bounded overview. Never present a partial page as a
            // complete manifest; callers can use the paginated artifact API instead.
            let page = store.list_versions(binding, None, 200)?;
            if page.has_more {
                Source::Unavailable {
                    version: "artifact-list-over-200:v1".into(),
                }
            } else {
                let mut digest = Sha256::new();
                let mut ids = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for metadata in page.versions {
                    digest.update(metadata.artifact_id.as_bytes());
                    digest.update(metadata.version.to_le_bytes());
                    digest.update(metadata.payload_sha256.as_bytes());
                    if seen.insert(metadata.artifact_id.clone()) {
                        ids.push(metadata.artifact_id);
                    }
                }
                available(
                    ArtifactReferences {
                        task_id: binding.task_id.clone(),
                        environment_id: binding.environment_id.clone(),
                        ids,
                    },
                    format!("artifacts-v1:{:x}", digest.finalize()),
                )
            }
        }
    };
    aggregate(
        binding.clone(),
        Sources {
            environment,
            run,
            plan,
            budget,
            events,
            artifacts,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_task_state::{
        approval::Binding,
        artifacts::{CaptureInput, Kind},
        plan::{ItemState, PlanItem, PlanStatus},
        run::Execution,
    };
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("agentz-details-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn binding(&self) -> PlanBinding {
            PlanBinding {
                project_id: agentz_task_state::canonical_root(&self.0).unwrap(),
                task_id: "task".into(),
                environment_id: format!("direct:{}", "a".repeat(64)),
                binding_version: "a".repeat(64),
            }
        }
        fn run(&self, b: &PlanBinding) {
            let origin = Binding {
                project_id: b.project_id.clone(),
                task_id: b.task_id.clone(),
                environment_id: b.environment_id.clone(),
                binding_version: b.binding_version.clone(),
                run_id: "run".into(),
                attempt_id: "a1".into(),
                capability_version: "fixture".into(),
            };
            let mut store = RunStore::open(&self.0).unwrap();
            store.create_direct(&origin, 1).unwrap();
            store
                .transition("run", "a1", Execution::Queued, Execution::Running, 2)
                .unwrap();
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn missing_direct_sources_do_not_create_database_or_directory() {
        let d = Directory::new();
        let dto = read(&d.0, &d.binding()).unwrap();
        assert!(matches!(dto.sources.run, Source::Missing));
        assert!(matches!(dto.sources.plan, Source::Missing));
        assert!(matches!(dto.sources.budget, Source::Missing));
        assert!(matches!(dto.sources.artifacts, Source::Missing));
        assert!(matches!(dto.sources.events, Source::Unknown));
        assert!(!dto.atomic_snapshot);
        assert!(!d.0.join(".agentz").exists());
    }
    #[test]
    fn artifact_references_are_scoped_and_available_without_payloads() {
        let d = Directory::new();
        let binding = d.binding();
        let mut store = ArtifactStore::initialize(&d.0).unwrap();
        for version in [1, 2] {
            store
                .capture(
                    &binding,
                    CaptureInput {
                        artifact_id: "research-report".into(),
                        version,
                        kind: Kind::Report,
                        title: "Research".into(),
                        source_relative_path: None,
                        media_type: "text/plain".into(),
                        created_ms: version,
                    },
                    format!("version {version}").as_bytes(),
                )
                .unwrap();
        }
        drop(store);
        let details = read(&d.0, &binding).unwrap();
        match details.sources.artifacts {
            Source::Available { value, version } => {
                assert_eq!(value.ids, vec!["research-report"]);
                assert_eq!(value.task_id, binding.task_id);
                assert_eq!(value.environment_id, binding.environment_id);
                assert!(version.starts_with("artifacts-v1:"));
            }
            other => panic!("expected artifact references, got {other:?}"),
        }
    }
    #[test]
    fn current_attempt_event_cursor_is_bounded_and_scoped() {
        let d = Directory::new();
        let scope = d.binding();
        d.run(&scope);
        let origin = Binding {
            project_id: scope.project_id.clone(),
            task_id: scope.task_id.clone(),
            environment_id: scope.environment_id.clone(),
            binding_version: scope.binding_version.clone(),
            run_id: "run".into(),
            attempt_id: "a1".into(),
            capability_version: "fixture".into(),
        };
        let mut runs = RunStore::open_existing_readwrite(&d.0).unwrap();
        runs.append_agent_event(&origin, "first", &serde_json::json!({"note":"one"}), 3)
            .unwrap();
        let appended = runs
            .append_agent_event(&origin, "second", &serde_json::json!({"note":"two"}), 4)
            .unwrap();
        drop(runs);
        let details = read(&d.0, &scope).unwrap();
        match details.sources.events {
            Source::Available { value, version } => {
                assert_eq!(value.sequence, appended.cursor.sequence);
                assert_eq!(value.stream.run_id, "run");
                assert_eq!(value.stream.attempt_id, "a1");
                assert_eq!(version, format!("event-v1:{}", appended.cursor.sequence));
            }
            other => panic!("expected current event cursor, got {other:?}"),
        }
    }
    #[test]
    fn existing_cancelled_plan_and_running_run_are_read_without_writes() {
        let d = Directory::new();
        let b = d.binding();
        d.run(&b);
        let mut plans = PlanStore::open(&d.0).unwrap();
        plans
            .create(
                &b,
                "plan",
                "goal",
                vec![PlanItem {
                    id: "item".into(),
                    description: "retained".into(),
                    state: ItemState::Pending,
                }],
                1,
            )
            .unwrap();
        let expected = plans.cancel(&b, "plan", 1, 2).unwrap();
        drop(plans);
        let run_file = d.0.join(".agentz/runs.db");
        let plan_file = d.0.join(".agentz/plans.db");
        let before = (
            std::fs::read(&run_file).unwrap(),
            std::fs::read(&plan_file).unwrap(),
        );
        let dto = read(&d.0, &b).unwrap();
        match dto.sources.plan {
            Source::Available { value, .. } => {
                assert_eq!(value, expected);
                assert_eq!(value.status, PlanStatus::Cancelled);
                assert_eq!(value.items.len(), 1)
            }
            _ => panic!("missing full plan"),
        };
        assert!(matches!(dto.sources.run, Source::Available { .. }));
        assert_eq!(
            before,
            (
                std::fs::read(run_file).unwrap(),
                std::fs::read(plan_file).unwrap()
            )
        );
    }
    #[test]
    fn corrupt_existing_database_is_error_not_missing_and_is_not_repaired() {
        let d = Directory::new();
        let b = d.binding();
        d.run(&b);
        let file = d.0.join(".agentz/runs.db");
        let conn = rusqlite::Connection::open(&file).unwrap();
        conn.execute("UPDATE attempts SET created_ms=-1", [])
            .unwrap();
        drop(conn);
        let before = std::fs::read(&file).unwrap();
        assert!(read(&d.0, &b).is_err());
        assert_eq!(std::fs::read(file).unwrap(), before);
    }
    #[test]
    fn cross_project_scope_is_still_rejected() {
        let d = Directory::new();
        let other = Directory::new();
        assert!(read(&other.0, &d.binding()).is_err());
    }

    #[test]
    fn unregistered_managed_environment_is_unavailable_with_version() {
        let d = Directory::new();
        let mut managed = d.binding();
        managed.environment_id = "managed-env".into();
        let dto = read(&d.0, &managed).expect("unregistered managed env is a status, not an error");
        match &dto.sources.environment {
            Source::Unavailable { version } => assert!(version.contains(&"a".repeat(64))),
            other => panic!("expected unavailable, got {other:?}"),
        }
        assert!(!d.0.join(".agentz").exists(), "read must not create state");
    }

    #[test]
    fn environment_record_for_another_environment_is_stale_not_error() {
        let d = Directory::new();
        let record = agentz_task_state::TaskEnvironment {
            schema_version: 1,
            task_id: "task".into(),
            environment_id: "other-env".into(),
            project_root: agentz_task_state::canonical_root(&d.0).unwrap(),
            workspace_root: d.0.display().to_string(),
            kind: "worktree".into(),
            branch: None,
            base_ref: "HEAD".into(),
            base_sha: "0".repeat(40),
            created_at: "1".into(),
        };
        agentz_task_state::register(&d.0, &record).unwrap();
        let mut managed = d.binding();
        managed.environment_id = "managed-env".into();
        let dto = read(&d.0, &managed).expect("mismatch is stale evidence, not an error");
        assert!(matches!(dto.sources.environment, Source::Stale { .. }));
    }
}
