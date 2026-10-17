//! Pure task-detail aggregation. Callers validate storage and retain host authority;
//! this module opens no database, grants no permission, and executes no action.
use crate::{
    budget::Snapshot,
    events::Cursor,
    plan::{Plan, PlanBinding},
    run::Run,
    TaskEnvironment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Source<T> {
    Available { value: T, version: String },
    Missing,
    Unsupported,
    /// The source cannot be read for a known reason, e.g. a managed environment that is not
    /// registered. `version` identifies the observation that produced this state.
    Unavailable { version: String },
    /// The stored source does not belong to the requested scope (for example an environment
    /// record for a different environment). `version` identifies the observation.
    Stale { version: String },
    Unknown,
}
impl<T> Source<T> {
    fn value(&self) -> Option<&T> {
        match self {
            Self::Available { value, .. } => Some(value),
            _ => None,
        }
    }
    fn validate_version(&self) -> Result<(), String> {
        let version = match self {
            Self::Available { version, .. }
            | Self::Unavailable { version }
            | Self::Stale { version } => version,
            _ => return Ok(()),
        };
        if version.trim().is_empty()
            || version.len() > 256
            || version.chars().any(char::is_control)
        {
            return Err("task detail source version invalid".into());
        }
        Ok(())
    }
}
/// Host-verified opaque references; no renderer paths, content or tool arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactReferences {
    pub task_id: String,
    pub environment_id: String,
    pub ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sources {
    pub environment: Source<TaskEnvironment>,
    pub run: Source<Run>,
    pub plan: Source<Plan>,
    pub events: Source<Cursor>,
    pub budget: Source<Snapshot>,
    pub artifacts: Source<ArtifactReferences>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetails {
    pub scope: PlanBinding,
    /// Always false: source versions describe independent observations, not a
    /// shared SQLite transaction or a guarantee of contemporaneous freshness.
    pub atomic_snapshot: bool,
    pub sources: Sources,
}
/// Aggregate already validated observations. An Available cross-scope value is
/// an error; callers must label stale/unavailable sources explicitly instead of
/// substituting empty plans, zero budget, or fabricated artifact references.
pub fn aggregate(scope: PlanBinding, sources: Sources) -> Result<TaskDetails, String> {
    for id in [&scope.project_id, &scope.task_id, &scope.environment_id] {
        if id.trim().is_empty() || id.chars().any(char::is_control) {
            return Err("task detail scope invalid".into());
        }
    }
    if scope.binding_version.len() != 64
        || !scope.binding_version.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("task detail binding version invalid".into());
    }
    sources.environment.validate_version()?;
    sources.run.validate_version()?;
    sources.plan.validate_version()?;
    sources.events.validate_version()?;
    sources.budget.validate_version()?;
    sources.artifacts.validate_version()?;
    if let Some(e) = sources.environment.value() {
        if e.project_root != scope.project_id
            || e.task_id != scope.task_id
            || e.environment_id != scope.environment_id
        {
            return Err("task detail environment scope mismatch".into());
        }
    }
    if let Some(r) = sources.run.value() {
        if r.task_id != scope.task_id || r.environment_id != scope.environment_id {
            return Err("task detail Run scope mismatch".into());
        }
    }
    if let Some(p) = sources.plan.value() {
        if p.binding != scope {
            return Err("task detail plan binding mismatch".into());
        }
    }
    if let Some(c) = sources.events.value() {
        let r = sources
            .run
            .value()
            .ok_or("event reference requires an available authoritative Run")?;
        if c.sequence < 0
            || c.stream.task_id != scope.task_id
            || c.stream.environment_id != scope.environment_id
            || c.stream.run_id != r.run_id
            || c.stream.attempt_id != r.attempt_id
        {
            return Err("task detail event reference mismatch".into());
        }
    }
    if let Some(b) = sources.budget.value() {
        if b.root_task_id != scope.task_id {
            return Err("task detail budget root mismatch".into());
        }
    }
    if let Some(a) = sources.artifacts.value() {
        if a.task_id != scope.task_id
            || a.environment_id != scope.environment_id
            || a.ids.len() > 200
            || a.ids.iter().any(|id| {
                id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control)
            })
        {
            return Err("task detail artifact scope/reference mismatch".into());
        }
        let unique: std::collections::HashSet<_> = a.ids.iter().collect();
        if unique.len() != a.ids.len() {
            return Err("task detail duplicate artifact reference".into());
        }
    }
    Ok(TaskDetails {
        scope,
        atomic_snapshot: false,
        sources,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        events::Stream,
        plan::{ItemState, PlanItem, PlanStatus},
        run::{Delivery, Execution},
    };
    fn scope() -> PlanBinding {
        PlanBinding {
            project_id: "host-project".into(),
            task_id: "task".into(),
            environment_id: "env".into(),
            binding_version: "a".repeat(64),
        }
    }
    fn sources() -> Sources {
        Sources {
            environment: Source::Missing,
            run: Source::Missing,
            plan: Source::Missing,
            events: Source::Missing,
            budget: Source::Unknown,
            artifacts: Source::Unsupported,
        }
    }
    fn available<T>(value: T) -> Source<T> {
        Source::Available {
            value,
            version: "independent-observation-1".into(),
        }
    }
    fn run() -> Run {
        Run {
            run_id: "run".into(),
            task_id: "task".into(),
            environment_id: "env".into(),
            attempt_id: "attempt".into(),
            execution: Execution::Succeeded,
            delivery: Delivery::AwaitingReview,
            updated_ms: 1,
        }
    }
    #[test]
    fn missing_unknown_and_stale_remain_distinct_and_non_atomic() {
        let mut s = sources();
        s.plan = Source::Stale {
            version: "plan-v1:7".into(),
        };
        s.environment = Source::Unavailable {
            version: "environment-unregistered:v1".into(),
        };
        let dto = aggregate(scope(), s.clone()).unwrap();
        assert!(!dto.atomic_snapshot);
        assert_eq!(dto.sources, s);
        let json = serde_json::to_value(dto).unwrap();
        assert_eq!(json["sources"]["budget"]["status"], "unknown");
        assert_eq!(json["sources"]["plan"]["status"], "stale");
    }
    #[test]
    fn cancelled_full_plan_items_are_preserved() {
        let p = Plan {
            id: "plan".into(),
            binding: scope(),
            revision: 7,
            objective: "goal".into(),
            items: vec![PlanItem {
                id: "item".into(),
                description: "retained".into(),
                state: ItemState::Pending,
            }],
            status: PlanStatus::Cancelled,
            created_ms: 1,
            updated_ms: 2,
        };
        let mut s = sources();
        s.plan = available(p.clone());
        let dto = aggregate(scope(), s).unwrap();
        assert_eq!(dto.sources.plan.value(), Some(&p));
    }
    #[test]
    fn plan_binding_and_run_cross_scope_are_rejected() {
        let mut r = run();
        r.environment_id = "old".into();
        let mut s = sources();
        s.run = available(r);
        assert!(aggregate(scope(), s).is_err());
        let mut p = Plan {
            id: "p".into(),
            binding: scope(),
            revision: 1,
            objective: "g".into(),
            items: vec![],
            status: PlanStatus::Active,
            created_ms: 1,
            updated_ms: 1,
        };
        p.binding.task_id = "other".into();
        let mut s = sources();
        s.plan = available(p);
        assert!(aggregate(scope(), s).is_err());
    }
    #[test]
    fn event_reference_requires_current_authoritative_attempt() {
        let c = Cursor {
            stream: Stream {
                task_id: "task".into(),
                run_id: "run".into(),
                attempt_id: "attempt".into(),
                environment_id: "env".into(),
            },
            sequence: 2,
        };
        let mut s = sources();
        s.events = available(c.clone());
        assert!(aggregate(scope(), s.clone()).is_err());
        s.run = available(run());
        assert!(aggregate(scope(), s.clone()).is_ok());
        let mut old = c;
        old.stream.attempt_id = "old".into();
        s.events = available(old);
        assert!(aggregate(scope(), s).is_err());
    }
    #[test]
    fn invalid_versions_and_artifact_scopes_fail_closed() {
        let mut s = sources();
        s.run = Source::Available {
            value: run(),
            version: "".into(),
        };
        assert!(aggregate(scope(), s).is_err());
        let mut s = sources();
        s.artifacts = available(ArtifactReferences {
            task_id: "other".into(),
            environment_id: "env".into(),
            ids: vec![],
        });
        assert!(aggregate(scope(), s).is_err());
        let mut s = sources();
        s.artifacts = available(ArtifactReferences {
            task_id: "task".into(),
            environment_id: "env".into(),
            ids: vec!["a".into(), "a".into()],
        });
        assert!(aggregate(scope(), s).is_err());
    }
}
