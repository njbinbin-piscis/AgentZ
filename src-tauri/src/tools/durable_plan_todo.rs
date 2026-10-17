//! Unregistered draft: caller must own an active Run/environment lease.
use crate::runtime::{approval::resolve_binding, durable_plan::DurablePlanSession};
use agentz_task_state::{
    approval::Binding,
    plan::{PlanBinding, PlanStore},
};
use async_trait::async_trait;
use piscis_core::host::EventSink;
use piscis_kernel::agent::{
    messages::AgentEvent,
    plan::{summarize_todos, PlanStore as KernelPlanStore, PlanTodoItem},
    tool::{Tool, ToolContext, ToolResult},
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

struct State {
    repository: PlanStore,
    session: DurablePlanSession,
}

struct AttemptState {
    binding: Binding,
    active: bool,
}
/// Capability is cloned into tools; only the host owns the non-cloneable guard.
#[derive(Clone)]
pub struct ActiveAttemptCapability(Arc<std::sync::Mutex<AttemptState>>);
pub struct ActiveAttemptGuard(ActiveAttemptCapability);
impl ActiveAttemptGuard {
    /// Call only after the host has validated and acquired the actual Run/attempt lease.
    pub fn acquire(binding: Binding) -> Result<(Self, ActiveAttemptCapability), String> {
        binding.validate()?;
        let capability = ActiveAttemptCapability(Arc::new(std::sync::Mutex::new(AttemptState {
            binding,
            active: true,
        })));
        Ok((Self(capability.clone()), capability))
    }
}
impl Drop for ActiveAttemptGuard {
    fn drop(&mut self) {
        let mut state = self
            .0
             .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.active = false;
    }
}
impl ActiveAttemptCapability {
    fn lock_active(
        &self,
        binding: &Binding,
    ) -> Result<std::sync::MutexGuard<'_, AttemptState>, String> {
        let state = self
            .0
            .lock()
            .map_err(|_| "attempt capability poisoned".to_string())?;
        if !state.active || &state.binding != binding {
            return Err("captured host attempt is no longer active".into());
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            assert!(self.0.starts_with(std::env::temp_dir()));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[derive(Default)]
    struct Recorder(std::sync::Mutex<Vec<Value>>);
    impl EventSink for Recorder {
        fn emit_session(&self, _: &str, _: &str, payload: Value) {
            self.0.lock().unwrap().push(payload);
        }
        fn emit_broadcast(&self, _: &str, _: Value) {
            panic!("unexpected broadcast");
        }
    }
    async fn fixture() -> (
        Directory,
        DurablePlanTodoTool,
        ToolContext,
        Arc<Recorder>,
        ActiveAttemptGuard,
    ) {
        let directory = Directory(
            std::env::temp_dir().join(format!("agentz-plan-tool-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir_all(&directory.0).unwrap();
        let mut binding = resolve_binding(&directory.0, &directory.0, "task", "run", false)
            .await
            .unwrap();
        binding.attempt_id = "distinct-host-attempt".into();
        let cancel = Arc::new(AtomicBool::new(false));
        let sink = Arc::new(Recorder::default());
        let (guard, capability) = ActiveAttemptGuard::acquire(binding.clone()).unwrap();
        let tool = DurablePlanTodoTool::new(
            directory.0.clone(),
            directory.0.clone(),
            binding,
            false,
            "goal".into(),
            PlanStore::open(&directory.0).unwrap(),
            cancel.clone(),
            piscis_kernel::agent::plan::new_plan_store(),
            sink.clone(),
            capability,
        )
        .unwrap();
        let ctx = ToolContext {
            session_id: "task".into(),
            workspace_root: directory.0.clone(),
            bypass_permissions: false,
            settings: Arc::new(piscis_kernel::agent::tool::ToolSettings::default()),
            max_iterations: None,
            memory_owner_id: "test".into(),
            pool_session_id: None,
            tool_use_id: None,
            cancel,
            loop_halt: None,
        };
        (directory, tool, ctx, sink, guard)
    }
    fn input() -> Value {
        json!({"todos":[{"id":"one","content":"step","status":"pending"}]})
    }
    #[tokio::test]
    async fn finished_guard_and_wrong_attempt_capability_reject_without_effects() {
        let (_directory, tool, ctx, sink, guard) = fixture().await;
        drop(guard);
        assert!(tool.call(input(), &ctx).await.unwrap().is_error);
        assert!(tool.publish_recovered_projection().await.is_err());
        assert!(tool.cache.lock().await.is_empty());
        assert!(sink.0.lock().unwrap().is_empty());
        let (_directory2, mut tool2, ctx2, sink2, _guard2) = fixture().await;
        let mut wrong = tool2.binding.clone();
        wrong.attempt_id = "old-attempt".into();
        let (_wrong_guard, wrong_capability) = ActiveAttemptGuard::acquire(wrong).unwrap();
        tool2.capability = wrong_capability;
        assert!(tool2.call(input(), &ctx2).await.unwrap().is_error);
        assert!(tool2.cache.lock().await.is_empty());
        assert!(sink2.0.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn omitted_merge_keeps_open_items_and_explicit_replacement_is_refused() {
        let (directory, tool, ctx, _sink, _guard) = fixture().await;
        let stored = || async {
            PlanStore::open(&directory.0)
                .unwrap()
                .continue_plan(&PlanBinding::from_execution(&tool.binding).unwrap())
                .unwrap()
                .unwrap()
        };
        assert!(
            !tool
                .call(
                    json!({"todos":[
                        {"id":"one","content":"step","status":"pending"},
                        {"id":"two","content":"other","status":"in_progress"}
                    ]}),
                    &ctx
                )
                .await
                .unwrap()
                .is_error
        );
        // Continuation lists only one step and omits merge: the other open item survives.
        assert!(
            !tool
                .call(
                    json!({"todos":[{"id":"one","content":"step","status":"completed"}]}),
                    &ctx
                )
                .await
                .unwrap()
                .is_error
        );
        let plan = stored().await;
        assert_eq!(plan.items.len(), 2);
        assert_eq!(plan.items[1].id, "two");
        // An explicit replacement that would drop the open item is refused without effect.
        let refused = tool
            .call(
                json!({"merge": false, "todos":[{"id":"one","content":"step","status":"completed"}]}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(refused.is_error, "{}", refused.content);
        assert_eq!(stored().await, plan);
    }

    #[tokio::test]
    async fn new_tool_recovers_existing_sqlite_plan_into_prompt_cache_without_write() {
        let (directory, tool, ctx, sink, _guard) = fixture().await;
        assert!(!tool.call(input(), &ctx).await.unwrap().is_error);
        let cache = piscis_kernel::agent::plan::new_plan_store();
        let (guard, capability) = ActiveAttemptGuard::acquire(tool.binding.clone()).unwrap();
        let restored = DurablePlanTodoTool::new(
            directory.0.clone(),
            directory.0.clone(),
            tool.binding.clone(),
            false,
            "goal".into(),
            PlanStore::open(&directory.0).unwrap(),
            ctx.cancel.clone(),
            cache.clone(),
            sink.clone(),
            capability,
        )
        .unwrap();
        let before = PlanStore::open(&directory.0)
            .unwrap()
            .continue_plan(&PlanBinding::from_execution(&tool.binding).unwrap())
            .unwrap()
            .unwrap();
        let initial = restored.recovered_projection().await;
        assert_eq!(
            restored.publish_recovered_projection().await.unwrap(),
            initial
        );
        assert_eq!(cache.lock().await.get("task").unwrap(), &initial);
        let after = PlanStore::open(&directory.0)
            .unwrap()
            .continue_plan(&PlanBinding::from_execution(&tool.binding).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(before, after);
        drop(guard);
    }
    #[tokio::test]
    async fn distinct_attempt_commits_before_matching_cache_event_and_schema() {
        let (directory, tool, ctx, sink, _guard) = fixture().await;
        assert_eq!(
            tool.input_schema(),
            tool.kernel_schema_tool().input_schema()
        );
        let result = tool.call(input(), &ctx).await.unwrap();
        assert!(!result.is_error, "{}", result.content);
        let db = PlanStore::open(&directory.0).unwrap();
        let plan = db
            .continue_plan(&PlanBinding::from_execution(&tool.binding).unwrap())
            .unwrap()
            .unwrap();
        let cached = tool.cache.lock().await.get("task").unwrap().clone();
        assert_eq!(cached, crate::runtime::durable_plan::to_kernel(&plan.items));
        let events = sink.0.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            serde_json::to_value(AgentEvent::PlanUpdate { items: cached }).unwrap()
        );
    }
    #[tokio::test]
    async fn mismatched_context_and_cancel_have_no_persistent_cache_or_event_effect() {
        let (_directory, tool, mut ctx, sink, _guard) = fixture().await;
        ctx.session_id = "other".into();
        assert!(tool.call(input(), &ctx).await.unwrap().is_error);
        ctx.session_id = "task".into();
        ctx.cancel = Arc::new(AtomicBool::new(false));
        assert!(tool.call(input(), &ctx).await.unwrap().is_error);
        ctx.cancel = tool.cancel.clone();
        ctx.cancel.store(true, Ordering::Release);
        assert!(tool.call(input(), &ctx).await.unwrap().is_error);
        assert!(tool.cache.lock().await.is_empty());
        assert!(sink.0.lock().unwrap().is_empty());
        assert!(tool
            .state
            .lock()
            .await
            .repository
            .continue_plan(&PlanBinding::from_execution(&tool.binding).unwrap())
            .unwrap()
            .is_none());
    }
    #[tokio::test]
    async fn sqlite_insert_failure_does_not_publish_cache_or_event() {
        let (directory, tool, ctx, sink, _guard) = fixture().await;
        let connection = rusqlite::Connection::open(directory.0.join(".agentz/plans.db")).unwrap();
        connection.execute_batch("CREATE TRIGGER deny_plan_insert BEFORE INSERT ON plans BEGIN SELECT RAISE(ABORT,'owned test failure'); END;").unwrap();
        assert!(tool.call(input(), &ctx).await.unwrap().is_error);
        assert!(tool.cache.lock().await.is_empty());
        assert!(sink.0.lock().unwrap().is_empty());
        assert!(tool
            .state
            .lock()
            .await
            .session
            .captured_revision()
            .is_none());
        assert!(tool
            .state
            .lock()
            .await
            .repository
            .continue_plan(&PlanBinding::from_execution(&tool.binding).unwrap())
            .unwrap()
            .is_none());
    }
}
pub struct DurablePlanTodoTool {
    state: Mutex<State>,
    project: PathBuf,
    workspace: PathBuf,
    binding: Binding,
    require_binding: bool,
    objective: String,
    new_id: String,
    cancel: Arc<AtomicBool>,
    cache: KernelPlanStore,
    sink: Arc<dyn EventSink>,
    capability: ActiveAttemptCapability,
}
impl DurablePlanTodoTool {
    /// Host supplies trusted fixed session/Run binding and keeps its access lease alive.
    /// Arbitrary renderer-supplied identities are not valid constructor inputs.
    pub fn new(
        project: PathBuf,
        workspace: PathBuf,
        binding: Binding,
        require_binding: bool,
        objective: String,
        repository: PlanStore,
        cancel: Arc<AtomicBool>,
        cache: KernelPlanStore,
        sink: Arc<dyn EventSink>,
        capability: ActiveAttemptCapability,
    ) -> Result<Self, String> {
        binding.validate()?;
        drop(capability.lock_active(&binding)?);
        let project = std::fs::canonicalize(project).map_err(|e| e.to_string())?;
        if objective.trim().is_empty() {
            return Err("plan objective is empty".into());
        }
        let workspace = std::fs::canonicalize(workspace).map_err(|e| e.to_string())?;
        let session =
            DurablePlanSession::recover(&repository, PlanBinding::from_execution(&binding)?)?;
        Ok(Self {
            state: Mutex::new(State {
                repository,
                session,
            }),
            project,
            workspace,
            binding,
            require_binding,
            objective,
            new_id: uuid::Uuid::new_v4().to_string(),
            cancel,
            cache,
            sink,
            capability,
        })
    }
    fn kernel_schema_tool(&self) -> piscis_kernel::tools::plan_todo::PlanTodoTool {
        piscis_kernel::tools::plan_todo::PlanTodoTool {
            store: self.cache.clone(),
            event_sink: self.sink.clone(),
        }
    }
    /// The exact projection recovered by this tool, without a second database read/token.
    pub async fn recovered_projection(&self) -> Vec<PlanTodoItem> {
        self.state.lock().await.session.projection().to_vec()
    }
    /// Host calls before prompt assembly; no tool write or revision increment occurs.
    /// Uses the same restored token/projection and the same revocation/publication boundary.
    pub async fn publish_recovered_projection(&self) -> Result<Vec<PlanTodoItem>, String> {
        let state = self.state.lock().await;
        let mut cache = self.cache.lock().await;
        let mut fresh = resolve_binding(
            &self.project,
            &self.workspace,
            &self.binding.task_id,
            &self.binding.run_id,
            self.require_binding,
        )
        .await?;
        fresh.attempt_id = self.binding.attempt_id.clone();
        fresh.validate()?;
        if fresh != self.binding {
            return Err("plan execution binding changed".into());
        }
        let _active = self.capability.lock_active(&self.binding)?;
        if self.cancel.load(Ordering::Acquire) {
            return Err("plan Run cancelled".into());
        }
        let current = state
            .repository
            .continue_plan(&PlanBinding::from_execution(&fresh)?)?;
        let identity = current.as_ref().map(|p| (p.id.as_str(), p.revision));
        if identity != state.session.captured_revision() {
            return Err("stale recovered plan identity/revision".into());
        }
        let projection = state.session.projection().to_vec();
        let payload = serde_json::to_value(AgentEvent::PlanUpdate {
            items: projection.clone(),
        })
        .map_err(|e| e.to_string())?;
        cache.insert(self.binding.task_id.clone(), projection.clone());
        self.sink
            .emit_session(&self.binding.task_id, "agent_event", payload);
        Ok(projection)
    }
}
#[async_trait]
impl Tool for DurablePlanTodoTool {
    fn name(&self) -> &str {
        "plan_todo"
    }
    fn description(&self) -> &str {
        "Maintain the durable visible execution plan for this task. Updates only the plan; execution requires real tools and their permissions."
    }
    fn input_schema(&self) -> Value {
        self.kernel_schema_tool().input_schema()
    }
    fn is_read_only(&self) -> bool {
        false
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<ToolResult> {
        if ctx.session_id != self.binding.task_id || !Arc::ptr_eq(&ctx.cancel, &self.cancel) {
            return Ok(ToolResult::err(
                "plan tool context is not the captured host Run",
            ));
        }
        let workspace = match std::fs::canonicalize(&ctx.workspace_root) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::err(e.to_string())),
        };
        if workspace != self.workspace {
            return Ok(ToolResult::err("plan tool workspace changed"));
        }
        let updates: Vec<PlanTodoItem> = match serde_json::from_value(input["todos"].clone()) {
            Ok(v) => v,
            Err(e) => return Ok(ToolResult::err(format!("invalid todos: {e}"))),
        };
        // Omitted merge keeps open items: a continuation turn that lists only some steps must
        // not drop the rest. Replacement requires an explicit merge=false and still cannot
        // drop unfinished items (see DurablePlanSession::write).
        let merge = input["merge"].as_bool().unwrap_or(true);
        // Lock order is service -> disposable cache. No SQLite transaction spans an await.
        // Acquire publication capacity before committing so no await separates commit/cache/event.
        let mut state = self.state.lock().await;
        let mut cache = self.cache.lock().await;
        let mut fresh = match resolve_binding(
            &self.project,
            &self.workspace,
            &self.binding.task_id,
            &self.binding.run_id,
            self.require_binding,
        )
        .await
        {
            Ok(b) => b,
            Err(e) => return Ok(ToolResult::err(e)),
        };
        // Resolver supplies environment identity; attempt remains the trusted host-captured identity.
        fresh.attempt_id = self.binding.attempt_id.clone();
        if let Err(e) = fresh.validate() {
            return Ok(ToolResult::err(e));
        }
        if fresh != self.binding {
            return Ok(ToolResult::err("plan execution binding changed"));
        }
        // All awaits are above this point. Guard drop and commit/publication are linearized
        // by the same host-owned lock; cancellation alone does not prove attempt liveness.
        let _active = match self.capability.lock_active(&self.binding) {
            Ok(active) => active,
            Err(e) => return Ok(ToolResult::err(e)),
        };
        if self.cancel.load(Ordering::Acquire) || ctx.is_cancelled() {
            return Ok(ToolResult::err("plan Run cancelled"));
        }
        let now = match SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| u64::try_from(d.as_millis()).ok())
        {
            Some(n) => n,
            None => return Ok(ToolResult::err("invalid plan clock")),
        };
        let State {
            repository,
            session,
        } = &mut *state;
        let plan = match session.write(
            repository,
            || PlanBinding::from_execution(&fresh),
            &updates,
            merge,
            &self.new_id,
            &self.objective,
            now,
        ) {
            Ok(plan) => plan,
            Err(e) => return Ok(ToolResult::err(e)),
        };
        let projection = session.projection().to_vec();
        let payload = serde_json::to_value(AgentEvent::PlanUpdate {
            items: projection.clone(),
        })?;
        cache.insert(self.binding.task_id.clone(), projection.clone());
        self.sink
            .emit_session(&self.binding.task_id, "agent_event", payload);
        Ok(ToolResult::ok(format!(
            "计划已持久化（{} 项，revision {}）:\n{}\n仅更新计划，不执行工作。",
            projection.len(),
            plan.revision,
            summarize_todos(&projection)
        )))
    }
}
