//! Task-level resource budget enforcement for model calls (AZ-024).
//!
//! The default policy is unlimited, so nothing changes until a user configures a limit.
//! A storage or configuration error stops a bound call before it reaches the provider.
//! Explicit policy denials retain the `budget denied:` prefix.
//!
//! A [`BudgetScope`] is shared by the parent turn and its research sub-agents, so every model
//! call in the task (including delegated ones) is charged to the same task total.
use agentz_task_state::{
    approval::Binding,
    budget::{self, BudgetStore, Measurement, Usage},
    budget_reservation::{
        Policy as ReservePolicy, Request, Reservation, ReservationStore, ReserveOutcome, Resources,
        StartOutcome, Status,
    },
};
use async_trait::async_trait;
use piscis_kernel::llm::{LlmChunk, LlmClient, LlmRequest, LlmResponse};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::AppHandle;

/// Prefix the reservation core puts on every refusal caused by the policy.
pub const DENIED_PREFIX: &str = "budget denied:";
const LIMITS_FILE: &str = "task_budget.json";
/// Calls within one turn are sequential; this only bounds leaked reservations.
const MAX_IN_FLIGHT: i64 = 4;
/// Upper bound on one model call's active time, used for reservations. A call that runs longer
/// settles above its reservation, which the core records as an overshoot for that revision.
const PER_CALL_ACTIVE_MS: i64 = 600_000;
/// Re-read the policy when a concurrent revision lands between read and reserve.
const MAX_RESERVE_ATTEMPTS: usize = 3;
const STALE_REVISION: &str = "stale reservation policy revision";

/// User-configured limits. `None` means unlimited.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TaskBudgetLimits {
    pub max_total_tokens: Option<i64>,
    pub max_active_ms: Option<i64>,
}

impl TaskBudgetLimits {
    pub fn validate(&self) -> Result<(), String> {
        for value in [self.max_total_tokens, self.max_active_ms]
            .into_iter()
            .flatten()
        {
            if value < 0 {
                return Err("budget limits must not be negative".into());
            }
        }
        Ok(())
    }
}

/// Read global limits. Only a genuinely absent file means unlimited.
pub fn load_limits(config_dir: &Path) -> Result<TaskBudgetLimits, String> {
    let path = config_dir.join(LIMITS_FILE);
    match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<TaskBudgetLimits>(&raw) {
            Ok(limits) => {
                limits.validate()?;
                Ok(limits)
            }
            Err(error) => Err(format!(
                "invalid task budget limits {}: {error}",
                path.display()
            )),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(TaskBudgetLimits::default())
        }
        Err(error) => Err(format!(
            "cannot read task budget limits {}: {error}",
            path.display()
        )),
    }
}

/// Persist the global limits. Applied to each task at its next turn as a new policy revision.
pub fn save_limits(config_dir: &Path, limits: &TaskBudgetLimits) -> Result<(), String> {
    limits.validate()?;
    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;
    let path = config_dir.join(LIMITS_FILE);
    let tmp = config_dir.join(format!("{LIMITS_FILE}.{}.tmp", uuid::Uuid::new_v4()));
    let raw = serde_json::to_string_pretty(limits).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn load_limits_for_app(app: &AppHandle) -> Result<TaskBudgetLimits, String> {
    let dir = crate::commands::data_scope::resolve_global_config_dir(app)?;
    load_limits(&dir)
}

fn boot_id() -> &'static str {
    static BOOT: OnceLock<String> = OnceLock::new();
    BOOT.get_or_init(|| uuid::Uuid::new_v4().to_string())
}

fn now_ms() -> Result<i64, String> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "invalid budget clock")?
        .as_millis();
    i64::try_from(ms).map_err(|_| "budget clock overflow".into())
}

fn digest(value: &str) -> String {
    use sha2::{Digest as _, Sha256};
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

/// Bring the task's policy in line with `limits`. The first call configures revision 1 (the
/// run must already exist); a changed limit becomes the next revision, so earlier usage keeps
/// counting against the same task total.
pub fn ensure_policy(project: &Path, task: &str, limits: &TaskBudgetLimits) -> Result<(), String> {
    limits.validate()?;
    let mut budget = BudgetStore::open_existing(project, false)?;
    budget.initialize()?;
    let mut reservations = ReservationStore::open_existing(project, false)?;
    reservations.initialize()?;
    match reservations.query(task)? {
        None => {
            budget.configure(
                task,
                budget::Policy {
                    revision: 1,
                    max_total_tokens: limits.max_total_tokens,
                    max_active_ms: limits.max_active_ms,
                    max_cost: None,
                },
            )?;
            reservations.configure(
                task,
                ReservePolicy {
                    revision: 1,
                    max_total_tokens: limits.max_total_tokens,
                    max_active_ms: limits.max_active_ms,
                    max_cost: None,
                    max_in_flight: MAX_IN_FLIGHT,
                },
            )?;
        }
        Some(snapshot) => {
            let current = snapshot.policy;
            if current.max_total_tokens != limits.max_total_tokens
                || current.max_active_ms != limits.max_active_ms
            {
                let next = current
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| "policy revision overflow".to_string())?;
                reservations.revise_policy(
                    task,
                    current.revision,
                    ReservePolicy {
                        revision: next,
                        max_total_tokens: limits.max_total_tokens,
                        max_active_ms: limits.max_active_ms,
                        max_cost: None,
                        max_in_flight: current.max_in_flight,
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// Shared handle to the bound task budget. The parent turn binds it once its binding is known;
/// sub-agents created earlier (with clones of the same handle) are then charged to that task.
/// An unbound scope is transparent for calls that have no task identity. The host must
/// propagate bind errors before it starts a task turn.
#[derive(Clone, Default)]
pub struct BudgetScope(Arc<OnceLock<Bound>>);

struct Bound {
    project: PathBuf,
    binding: Binding,
}

impl BudgetScope {
    /// Prepare the task policy and bind this scope to the task.
    pub fn bind(
        &self,
        project: &Path,
        binding: &Binding,
        limits: &TaskBudgetLimits,
    ) -> Result<(), String> {
        ensure_policy(project, &binding.task_id, limits)?;
        self.0
            .set(Bound {
                project: project.to_path_buf(),
                binding: binding.clone(),
            })
            .map_err(|_| "task budget scope was already bound".to_string())
    }

    /// Wrap a model client so each call is reserved and settled against the bound task.
    pub fn guard(&self, inner: Box<dyn LlmClient>) -> Box<dyn LlmClient> {
        match self.0.get() {
            Some(bound) => Box::new(BudgetedClient {
                inner,
                project: bound.project.clone(),
                binding: bound.binding.clone(),
            }),
            None => inner,
        }
    }
}

struct BudgetedClient {
    inner: Box<dyn LlmClient>,
    project: PathBuf,
    binding: Binding,
}

struct Ticket {
    operation: String,
    reserved_tokens: i64,
    started: Instant,
}

/// Settles a started call exactly once. If the call future is dropped before completion
/// (user stop, turn timeout), the reserved estimate is recorded on drop so the reservation
/// cannot stay in flight and hold budget.
struct Settlement<'a> {
    client: &'a BudgetedClient,
    ticket: Option<Ticket>,
}

impl Settlement<'_> {
    fn settle(mut self, measured: Option<(u32, u32)>) {
        if let Some(ticket) = self.ticket.take() {
            self.client.finish(ticket, measured);
        }
    }
}

impl Drop for Settlement<'_> {
    fn drop(&mut self) {
        if let Some(ticket) = self.ticket.take() {
            self.client.finish(ticket, None);
        }
    }
}

/// Settlements that could not be persisted in this process, retried before the next reservation.
/// Without the retry, a transient storage error would leave a started call holding a slot (and
/// budget) until the process restarts.
static PENDING: Mutex<Vec<(PathBuf, Pending)>> = Mutex::new(Vec::new());

/// A reservation state change that could not be persisted and must be retried.
#[derive(Clone)]
enum Pending {
    Settle(Box<Usage>),
    Release { binding: Binding, operation: String },
}

fn pending() -> MutexGuard<'static, Vec<(PathBuf, Pending)>> {
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn apply_pending(project: &Path, item: &Pending) -> Result<(), String> {
    let mut store = ReservationStore::open_existing(project, false)?;
    match item {
        Pending::Settle(usage) => store.settle(usage.as_ref().clone()).map(|_| ()),
        Pending::Release { binding, operation } => store
            .release_unstarted(binding, operation, "release retried after storage error")
            .map(|_| ()),
    }
}

/// Apply each entry; return the ones that still fail so they stay queued.
fn retry_entries(entries: Vec<(PathBuf, Pending)>) -> Vec<(PathBuf, Pending)> {
    entries
        .into_iter()
        .filter_map(|(project, item)| match apply_pending(&project, &item) {
            Ok(()) => None,
            Err(error) => {
                tracing::warn!(%error, project = %project.display(), "budget reservation retry failed; will retry again");
                Some((project, item))
            }
        })
        .collect()
}

fn retry_pending_settlements() {
    let entries = std::mem::take(&mut *pending());
    let failed = retry_entries(entries);
    pending().extend(failed);
}

impl BudgetedClient {
    /// Reserve and start one call. Storage and policy errors both stop the call.
    fn begin(&self, req: &LlmRequest) -> Result<Option<Ticket>, String> {
        retry_pending_settlements();
        let task = self.binding.task_id.clone();
        let mut store = match ReservationStore::open_existing(&self.project, false) {
            Ok(store) => store,
            Err(error) => {
                return Err(format!("budget unavailable: {error}"));
            }
        };
        let input = piscis_kernel::llm::estimate_request_input_tokens(
            &req.messages,
            req.system.as_deref(),
            &req.tools,
        );
        let reserved_tokens = i64::try_from(input)
            .unwrap_or(i64::MAX)
            .saturating_add(i64::from(req.max_tokens));
        let operation = uuid::Uuid::new_v4().to_string();
        let claim = uuid::Uuid::new_v4().to_string();
        let denied_or_unavailable = |error: String| -> Result<Option<Ticket>, String> {
            if error.starts_with(DENIED_PREFIX) {
                Err(error)
            } else {
                Err(format!("budget unavailable: {error}"))
            }
        };
        let mut attempt = 0;
        let active_ms = loop {
            attempt += 1;
            let snapshot = match store.query(&task) {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => return Err("budget unavailable: task policy missing".into()),
                Err(error) => {
                    return Err(format!("budget unavailable: {error}"));
                }
            };
            // Every host reservation carries a bounded active time so that recovered records
            // always have a duration. Under a limit the bound never exceeds the remaining budget,
            // and is capped per call so concurrent calls are not refused by one call's bound.
            let active_ms = match snapshot.policy.max_active_ms {
                None => PER_CALL_ACTIVE_MS,
                Some(limit) => {
                    let remaining = limit.saturating_sub(snapshot.usage.known_active_ms);
                    if remaining <= 0 {
                        return Err(format!("{DENIED_PREFIX} task active-time budget exhausted"));
                    }
                    remaining.min(PER_CALL_ACTIVE_MS)
                }
            };
            let reservation = Reservation {
                binding: self.binding.clone(),
                member_id: task.clone(),
                operation_id: operation.clone(),
                boot_id: boot_id().into(),
                kind: "model_call".into(),
                intent_digest: digest(&format!("model_call:{operation}")),
                claim_digest: digest(&claim),
                policy_revision: snapshot.policy.revision,
                resources: Resources {
                    total_tokens: Some(reserved_tokens),
                    active_ms: Some(active_ms),
                    cost: None,
                },
                status: Status::Reserved,
                usage_id: None,
                reason: None,
            };
            match store.reserve(Request {
                reservation,
                claim: claim.clone(),
            }) {
                // A fresh operation id cannot replay; refuse rather than run a free call.
                Ok(ReserveOutcome::Replay(_)) => {
                    return Err(format!("{DENIED_PREFIX} duplicate call operation"));
                }
                Ok(_) => break active_ms,
                Err(error)
                    if error.starts_with(STALE_REVISION) && attempt < MAX_RESERVE_ATTEMPTS =>
                {
                    continue;
                }
                Err(error) => {
                    // A failed commit may still have landed; undo it so it cannot hold a slot.
                    if !error.starts_with(DENIED_PREFIX) {
                        self.recover_outstanding(
                            &mut store,
                            &operation,
                            reserved_tokens,
                            active_ms,
                        );
                    }
                    return denied_or_unavailable(error);
                }
            }
        };
        match store.start(&self.binding, &operation, &claim, boot_id()) {
            Ok(StartOutcome::Granted(_)) => Ok(Some(Ticket {
                operation,
                reserved_tokens,
                started: Instant::now(),
            })),
            // A fresh operation id cannot replay; treat it as a refusal rather than a free call.
            Ok(StartOutcome::Replay(_)) => Err(format!("{DENIED_PREFIX} duplicate call operation")),
            Err(error) => {
                // Undo the reservation (or settle it if the start landed) so it cannot hold budget.
                self.recover_outstanding(&mut store, &operation, reserved_tokens, active_ms);
                denied_or_unavailable(error)
            }
        }
    }

    /// Settle a started call with its measured usage. Without a measurement, the reserved
    /// estimate is recorded (conservative). Settlement errors are logged; the reservation then
    /// remains as a started call and is recovered conservatively on a later attempt.
    fn finish(&self, ticket: Ticket, measured: Option<(u32, u32)>) {
        let active_ms = i64::try_from(ticket.started.elapsed().as_millis()).unwrap_or(i64::MAX);
        let (input_tokens, output_tokens, source) = match measured {
            Some((input, output)) => (i64::from(input), i64::from(output), "kernel-usage"),
            None => (ticket.reserved_tokens, 0, "estimate-without-usage"),
        };
        match self.usage_for(
            &ticket.operation,
            input_tokens,
            output_tokens,
            active_ms,
            source,
        ) {
            Ok(usage) => self.settle_or_queue(usage),
            Err(error) => {
                tracing::warn!(%error, operation = %ticket.operation, "budget settlement skipped")
            }
        }
    }

    fn usage_for(
        &self,
        operation: &str,
        input_tokens: i64,
        output_tokens: i64,
        active_ms: i64,
        source: &str,
    ) -> Result<Usage, String> {
        Ok(Usage {
            project_id: self.binding.project_id.clone(),
            root_task_id: self.binding.task_id.clone(),
            member_id: self.binding.task_id.clone(),
            usage_id: uuid::Uuid::new_v4().to_string(),
            operation_id: operation.into(),
            binding: self.binding.clone(),
            measurement: Measurement {
                input_tokens: Some(input_tokens),
                output_tokens: Some(output_tokens),
                active_ms: Some(active_ms),
                cost: None,
                source: source.into(),
                price_source: None,
            },
            created_ms: now_ms()?,
        })
    }

    fn settle_or_queue(&self, usage: Usage) {
        let operation = usage.operation_id.clone();
        let settled = ReservationStore::open_existing(&self.project, false)
            .and_then(|mut store| store.settle(usage.clone()).map(|_| ()));
        if let Err(error) = settled {
            tracing::warn!(%error, %operation, "budget settlement failed; queued for retry");
            pending().push((self.project.clone(), Pending::Settle(Box::new(usage))));
        }
    }

    /// After a reserve or start failed without a policy denial, the commit may still have
    /// landed. Read the record back: a started call is settled at its estimate, a reserved call
    /// is released. Anything that cannot be applied now is queued, so it cannot hold a slot.
    fn recover_outstanding(
        &self,
        store: &mut ReservationStore,
        operation: &str,
        reserved_tokens: i64,
        active_ms: i64,
    ) {
        let outstanding = match store.query(&self.binding.task_id) {
            Ok(Some(snapshot)) => snapshot.outstanding,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(%error, %operation, "budget state unreadable after failed commit");
                return;
            }
        };
        let Some(started) = outstanding
            .iter()
            .find(|r| r.operation_id == operation)
            .map(|r| r.status == Status::Started)
        else {
            return;
        };
        if started {
            match self.usage_for(
                operation,
                reserved_tokens,
                0,
                active_ms,
                "estimate-without-usage",
            ) {
                Ok(usage) => self.settle_or_queue(usage),
                Err(error) => tracing::warn!(%error, %operation, "budget settlement skipped"),
            }
        } else if let Err(error) =
            store.release_unstarted(&self.binding, operation, "start refused")
        {
            tracing::warn!(%error, %operation, "budget release failed; queued for retry");
            pending().push((
                self.project.clone(),
                Pending::Release {
                    binding: self.binding.clone(),
                    operation: operation.into(),
                },
            ));
        }
    }
}

#[async_trait]
impl LlmClient for BudgetedClient {
    async fn stream(
        &self,
        req: LlmRequest,
        tx: tokio::sync::mpsc::Sender<LlmChunk>,
    ) -> anyhow::Result<()> {
        let ticket = self.begin(&req).map_err(anyhow::Error::msg)?;
        let settlement = Settlement {
            client: self,
            ticket,
        };
        let (inner_tx, mut inner_rx) = tokio::sync::mpsc::channel::<LlmChunk>(64);
        let forward = async {
            let mut measured = None;
            while let Some(chunk) = inner_rx.recv().await {
                if let LlmChunk::Done {
                    input_tokens,
                    output_tokens,
                } = &chunk
                {
                    measured = Some((*input_tokens, *output_tokens));
                }
                if tx.send(chunk).await.is_err() {
                    break;
                }
            }
            measured
        };
        let (result, measured) = tokio::join!(self.inner.stream(req, inner_tx), forward);
        settlement.settle(if result.is_ok() { measured } else { None });
        result
    }

    async fn complete(&self, req: LlmRequest) -> anyhow::Result<LlmResponse> {
        let ticket = self.begin(&req).map_err(anyhow::Error::msg)?;
        let settlement = Settlement {
            client: self,
            ticket,
        };
        let result = self.inner.complete(req).await;
        let measured = result
            .as_ref()
            .ok()
            .map(|response| (response.input_tokens, response.output_tokens));
        settlement.settle(measured);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentz_task_state::run::{Execution, RunStore};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture {
        path: PathBuf,
        binding: Binding,
    }

    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("agentz-task-budget-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            let binding = Binding {
                project_id: agentz_task_state::canonical_root(&path).unwrap(),
                task_id: "task".into(),
                environment_id: format!("direct:{}", "a".repeat(64)),
                binding_version: "a".repeat(64),
                run_id: "run".into(),
                attempt_id: "a1".into(),
                capability_version: "fixture".into(),
            };
            let mut runs = RunStore::open(&path).unwrap();
            runs.create_direct(&binding, 1).unwrap();
            runs.transition("run", "a1", Execution::Queued, Execution::Running, 2)
                .unwrap();
            Self { path, binding }
        }

        fn client(&self, inner: Box<dyn LlmClient>) -> BudgetedClient {
            BudgetedClient {
                inner,
                project: self.path.clone(),
                binding: self.binding.clone(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    /// Returns a fixed usage and counts how many calls reached the provider.
    struct FakeClient {
        calls: Arc<AtomicUsize>,
        input: u32,
        output: u32,
    }

    #[async_trait]
    impl LlmClient for FakeClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> anyhow::Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let _ = tx
                .send(LlmChunk::Done {
                    input_tokens: self.input,
                    output_tokens: self.output,
                })
                .await;
            Ok(())
        }

        async fn complete(&self, _req: LlmRequest) -> anyhow::Result<LlmResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(LlmResponse {
                content: "ok".into(),
                tool_calls: Vec::new(),
                input_tokens: self.input,
                output_tokens: self.output,
            })
        }
    }

    /// Never completes, to simulate a call that the user stops or times out.
    struct HangingClient;

    #[async_trait]
    impl LlmClient for HangingClient {
        async fn stream(
            &self,
            _req: LlmRequest,
            _tx: tokio::sync::mpsc::Sender<LlmChunk>,
        ) -> anyhow::Result<()> {
            std::future::pending::<()>().await;
            Ok(())
        }

        async fn complete(&self, _req: LlmRequest) -> anyhow::Result<LlmResponse> {
            std::future::pending::<()>().await;
            unreachable!()
        }
    }

    fn request(max_tokens: u32) -> LlmRequest {
        LlmRequest {
            messages: vec![piscis_kernel::llm::LlmMessage {
                role: "user".into(),
                content: piscis_kernel::llm::MessageContent::text("hello"),
            }],
            system: None,
            tools: Vec::new(),
            model: "fixture".into(),
            max_tokens,
            stream: false,
            vision_override: None,
        }
    }

    /// Reserved amount for `request(4)`, i.e. estimate plus max_tokens.
    fn reserved_for_fixture() -> i64 {
        let req = request(4);
        let estimate = piscis_kernel::llm::estimate_request_input_tokens(
            &req.messages,
            req.system.as_deref(),
            &req.tools,
        );
        i64::try_from(estimate).unwrap() + 4
    }

    fn snapshot(path: &Path) -> agentz_task_state::budget_reservation::ReservationSnapshot {
        ReservationStore::open_existing(path, true)
            .unwrap()
            .query("task")
            .unwrap()
            .unwrap()
    }

    #[tokio::test]
    async fn unlimited_default_records_usage_and_never_denies() {
        let f = Fixture::new();
        ensure_policy(&f.path, "task", &TaskBudgetLimits::default()).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let client = f.client(Box::new(FakeClient {
            calls: calls.clone(),
            input: 2,
            output: 3,
        }));
        for _ in 0..3 {
            client.complete(request(4)).await.unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let snap = snapshot(&f.path);
        assert_eq!(snap.usage.usage_count, 3);
        assert_eq!(snap.usage.known_input_tokens, 6);
    }

    #[tokio::test]
    async fn exhausted_limit_stops_next_call_and_raised_limit_resumes_without_reset() {
        let f = Fixture::new();
        let limit = reserved_for_fixture();
        let limits = TaskBudgetLimits {
            max_total_tokens: Some(limit),
            max_active_ms: None,
        };
        ensure_policy(&f.path, "task", &limits).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let client = f.client(Box::new(FakeClient {
            calls: calls.clone(),
            input: 2,
            output: 3,
        }));

        // First call fits exactly; it settles with 5 real tokens.
        client.complete(request(4)).await.unwrap();

        // Second call would exceed the task total: refused before the provider is reached.
        let error = client.complete(request(4)).await.unwrap_err().to_string();
        assert!(error.starts_with(DENIED_PREFIX), "{error}");
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // Raising the limit is a new revision. The 5 tokens already used still count.
        let raised = TaskBudgetLimits {
            max_total_tokens: Some(limit + 100),
            max_active_ms: None,
        };
        ensure_policy(&f.path, "task", &raised).unwrap();
        let snap = snapshot(&f.path);
        assert_eq!(snap.policy.revision, 2);
        assert_eq!(snap.usage.known_input_tokens, 2);
        client.complete(request(4)).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn dropped_calls_are_settled_and_do_not_hold_slots_or_budget() {
        let f = Fixture::new();
        ensure_policy(&f.path, "task", &TaskBudgetLimits::default()).unwrap();
        let hanging = f.client(Box::new(HangingClient));
        // More stopped calls than the slot cap: none may leave a reservation in flight.
        for _ in 0..(MAX_IN_FLIGHT + 1) {
            let (tx, _rx) = tokio::sync::mpsc::channel(8);
            let stopped = tokio::time::timeout(
                std::time::Duration::from_millis(5),
                hanging.stream(request(4), tx),
            )
            .await;
            assert!(stopped.is_err());
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let client = f.client(Box::new(FakeClient {
            calls: calls.clone(),
            input: 1,
            output: 1,
        }));
        client.complete(request(4)).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(snapshot(&f.path).usage.usage_count, (MAX_IN_FLIGHT + 2));
    }

    #[tokio::test]
    async fn active_time_limit_admits_calls_with_a_bounded_reservation() {
        let f = Fixture::new();
        let limits = TaskBudgetLimits {
            max_total_tokens: None,
            max_active_ms: Some(60_000),
        };
        ensure_policy(&f.path, "task", &limits).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let client = f.client(Box::new(FakeClient {
            calls: calls.clone(),
            input: 1,
            output: 1,
        }));
        client.complete(request(4)).await.unwrap();
        client.complete(request(4)).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn exhausted_active_time_refuses_before_the_provider() {
        let f = Fixture::new();
        let limits = TaskBudgetLimits {
            max_total_tokens: None,
            max_active_ms: Some(0),
        };
        ensure_policy(&f.path, "task", &limits).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let client = f.client(Box::new(FakeClient {
            calls: calls.clone(),
            input: 1,
            output: 1,
        }));
        let error = client.complete(request(4)).await.unwrap_err().to_string();
        assert!(error.starts_with(DENIED_PREFIX), "{error}");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn sub_agent_scope_shares_the_parent_task_budget() {
        let f = Fixture::new();
        let limits = TaskBudgetLimits {
            max_total_tokens: Some(1),
            max_active_ms: None,
        };
        let scope = BudgetScope::default();
        // Sub-agent clones the handle before the parent binds it (as in the host).
        let for_sub_agent = scope.clone();
        scope.bind(&f.path, &f.binding, &limits).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let guarded = for_sub_agent.guard(Box::new(FakeClient {
            calls: calls.clone(),
            input: 1,
            output: 1,
        }));
        let error = guarded.complete(request(4)).await.unwrap_err().to_string();
        assert!(error.starts_with(DENIED_PREFIX), "{error}");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unbound_scope_is_transparent() {
        let calls = Arc::new(AtomicUsize::new(0));
        let guarded = BudgetScope::default().guard(Box::new(FakeClient {
            calls: calls.clone(),
            input: 1,
            output: 1,
        }));
        guarded.complete(request(4)).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn missing_store_stops_call_before_provider() {
        let path = std::env::temp_dir().join(format!("agentz-no-store-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        let binding = Binding {
            project_id: agentz_task_state::canonical_root(&path).unwrap(),
            task_id: "task".into(),
            environment_id: format!("direct:{}", "a".repeat(64)),
            binding_version: "a".repeat(64),
            run_id: "run".into(),
            attempt_id: "a1".into(),
            capability_version: "fixture".into(),
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let scope = BudgetScope::default();
        let error = scope
            .bind(
                &path,
                &binding,
                &TaskBudgetLimits {
                    max_total_tokens: Some(1),
                    max_active_ms: None,
                },
            )
            .unwrap_err();
        assert!(!error.is_empty());
        let guarded = scope.guard(Box::new(FakeClient {
            calls: calls.clone(),
            input: 1,
            output: 1,
        }));
        // The host propagates bind failure and never calls guard/complete.
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        drop(guarded);
        let _ = std::fs::remove_dir_all(&path);
    }

    #[tokio::test]
    async fn lost_reservation_store_stops_bound_model_call() {
        let f = Fixture::new();
        ensure_policy(
            &f.path,
            "task",
            &TaskBudgetLimits {
                max_total_tokens: Some(100),
                max_active_ms: None,
            },
        )
        .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let client = f.client(Box::new(FakeClient {
            calls: calls.clone(),
            input: 1,
            output: 1,
        }));
        std::fs::rename(
            f.path.join(".agentz/runs.db"),
            f.path.join(".agentz/lost-runs.db"),
        )
        .unwrap();
        let error = client.complete(request(4)).await.unwrap_err().to_string();
        assert!(error.starts_with("budget unavailable:"), "{error}");
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn failed_reservation_updates_stay_queued_until_applied() {
        // A project without a store cannot be updated, so the entry must come back for retry.
        let missing = std::env::temp_dir().join(format!("agentz-missing-{}", uuid::Uuid::new_v4()));
        let entry = (
            missing.clone(),
            Pending::Release {
                binding: Binding {
                    project_id: "p".into(),
                    task_id: "task".into(),
                    environment_id: format!("direct:{}", "a".repeat(64)),
                    binding_version: "a".repeat(64),
                    run_id: "run".into(),
                    attempt_id: "a1".into(),
                    capability_version: "fixture".into(),
                },
                operation: "op".into(),
            },
        );
        let remaining = retry_entries(vec![entry]);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].0, missing);
    }

    #[test]
    fn limits_round_trip_and_reject_negative_values() {
        let dir = std::env::temp_dir().join(format!("agentz-limits-{}", uuid::Uuid::new_v4()));
        assert_eq!(load_limits(&dir).unwrap(), TaskBudgetLimits::default());
        let limits = TaskBudgetLimits {
            max_total_tokens: Some(10),
            max_active_ms: Some(20),
        };
        save_limits(&dir, &limits).unwrap();
        assert_eq!(load_limits(&dir).unwrap(), limits);
        std::fs::write(dir.join(LIMITS_FILE), "{").unwrap();
        assert!(load_limits(&dir).is_err());
        assert!(save_limits(
            &dir,
            &TaskBudgetLimits {
                max_total_tokens: Some(-1),
                max_active_ms: None,
            }
        )
        .is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
