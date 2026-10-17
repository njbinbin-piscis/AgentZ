//! Harness configuration assembled per call site.
//!
//! Every call site that previously constructed an [`crate::agent::loop_::AgentLoop`]
//! literal (piscis chat / koi / fish / scheduler / debug) will build a
//! [`HarnessConfig`] in p1. The three `Option<...>` fields (`persistence`,
//! `summary_store`, `frame_provider`) deliberately allow the fish
//! ephemeral loop to opt out of any DB-bound feature — the plan explicitly
//! forbids tying the harness to SQLite.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::agent::tool::ToolRegistry;
use crate::policy::PolicyGate;
use crate::store::Database;

use super::{LayeredBudget, LayeredPrompt};

/// Upstream provider flavour. Used by the future [`RequestBuilder`] (p12)
/// to pick the right message transformation.
///
/// [`RequestBuilder`]: crate::agent::harness
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Anthropic,
    OpenAi,
    Other,
}

impl ProviderKind {
    /// Best-effort inference from a model id string.
    pub fn from_model_id(model: &str) -> Self {
        let lower = model.to_ascii_lowercase();
        if lower.contains("claude") || lower.contains("anthropic") {
            ProviderKind::Anthropic
        } else if lower.contains("gpt") || lower.contains("openai") || lower.contains("o1") {
            ProviderKind::OpenAi
        } else {
            ProviderKind::Other
        }
    }
}

/// Snapshot of a session's rolling summary (p7).
#[derive(Debug, Clone)]
pub struct SummarySnapshot {
    pub version: u32,
    pub summary_text: String,
    pub structured: Option<serde_json::Value>,
}

/// Persists rolling summaries. Implemented on top of the `sessions` table
/// in p7. Fish-style ephemeral runs pass `None` and skip async summary.
pub trait SummaryStore: Send + Sync {
    fn load_latest(&self, session_id: &str) -> Option<SummarySnapshot>;
    fn store(&self, session_id: &str, snapshot: SummarySnapshot) -> anyhow::Result<()>;
}

/// Supplies the "state frame" synthetic context (p6). Fish skips this.
pub trait StateFrameProvider: Send + Sync {
    /// Return the latest state frame JSON for the given session, if any.
    fn latest(&self, session_id: &str) -> Option<serde_json::Value>;
}

/// Compaction thresholds + per-tool-result cap + optional summary model
/// plumbed down from `store::settings::Settings`. Each scene factory
/// accepts this as a single parameter so the scope of additions per
/// call site stays small.
#[derive(Debug, Clone)]
pub struct CompactionSettings {
    pub micro_percent: u8,
    pub auto_percent: u8,
    pub full_percent: u8,
    pub max_tool_result_tokens: u32,
    pub summary_model: Option<String>,
}

impl Default for CompactionSettings {
    fn default() -> Self {
        Self {
            micro_percent: super::budget::DEFAULT_TIER_MICRO_PERCENT,
            auto_percent: super::budget::DEFAULT_TIER_AUTO_PERCENT,
            full_percent: super::budget::DEFAULT_TIER_FULL_PERCENT,
            max_tool_result_tokens: super::budget::DEFAULT_MAX_TOOL_RESULT_TOKENS,
            summary_model: None,
        }
    }
}

impl CompactionSettings {
    /// Pull from the store-level settings struct. Invalid orderings are
    /// clamped by `LayeredBudget::with_tier_percents` at apply time.
    pub fn from_settings(s: &crate::store::settings::Settings) -> Self {
        Self {
            micro_percent: s.compaction_micro_percent,
            auto_percent: s.compaction_auto_percent,
            full_percent: s.compaction_full_percent,
            max_tool_result_tokens: s.max_tool_result_tokens,
            summary_model: s.summary_model.clone(),
        }
    }
}

/// User confirmation preferences — identical shape to
/// [`crate::agent::loop_::ConfirmFlags`] but duplicated here to keep the
/// harness module free of cross-file coupling while p1 migrates the
/// existing structs.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConfirmFlags {
    pub confirm_shell: bool,
    pub confirm_file_write: bool,
}

/// Shared live confirmation preferences — re-exported from the agent loop.
pub type ConfirmFlagsHandle = crate::agent::loop_::ConfirmFlagsHandle;

impl From<ConfirmFlags> for crate::agent::loop_::ConfirmFlags {
    fn from(f: ConfirmFlags) -> Self {
        crate::agent::loop_::ConfirmFlags {
            confirm_shell: f.confirm_shell,
            confirm_file_write: f.confirm_file_write,
        }
    }
}

impl From<crate::agent::loop_::ConfirmFlags> for ConfirmFlags {
    fn from(f: crate::agent::loop_::ConfirmFlags) -> Self {
        ConfirmFlags {
            confirm_shell: f.confirm_shell,
            confirm_file_write: f.confirm_file_write,
        }
    }
}

/// Per-run harness configuration.
///
/// Intentionally non-`Clone` at the top level because the contained trait
/// objects (`SummaryStore`, `StateFrameProvider`) are not cheap to clone;
/// the inner
/// pieces are all `Arc` so downstream code can share.
pub struct HarnessConfig {
    /// Scene tag, e.g. `"main"`, `"koi"`, `"fish"`, `"scheduler"`, `"debug"`.
    pub scene: String,

    /// Primary model id.
    pub model: String,
    /// Models to try in order after rate-limit / overloaded / not-found.
    pub fallback_models: Vec<String>,
    /// Output cap (`max_tokens`).
    pub max_tokens: u32,
    /// Model context window (0 = auto).
    pub context_window: u32,

    /// Layered system prompt (L0..Lhint).
    pub layered_prompt: LayeredPrompt,
    /// Tool registry, registered by the call site before handing over.
    pub registry: Arc<ToolRegistry>,
    /// Scene policy gate.
    pub policy: Arc<PolicyGate>,
    /// User confirmation preferences (shared handle for live updates).
    pub confirm_flags: crate::agent::loop_::ConfirmFlagsHandle,
    /// User-configured vision override (None = auto from model id).
    pub vision_override: Option<bool>,
    /// Optional separate vision model client for image analysis delegation.
    pub vision_delegate: Option<Box<dyn crate::llm::LlmClient>>,
    /// The model id for the vision delegate client (e.g. "qwen3.6-plus", "gpt-4o").
    /// Only meaningful when `vision_delegate` is `Some`.
    pub vision_model: String,

    /// Derived token budget + tier classifier.
    pub budget: LayeredBudget,

    /// Provider flavour — stable through a run; p12 uses this to pick the
    /// per-provider request shape.
    pub provider_kind: ProviderKind,

    /// Base auto-compact threshold in cumulative input tokens — the
    /// user-configured knob that scene policy's `auto_compact_threshold_override`
    /// can shadow (see `ScenePolicy::effective_auto_compact_threshold`).
    /// `0` disables threshold-driven compaction.
    pub auto_compact_input_tokens_threshold: u32,

    /// When true, the agent loop asks the LLM client for SSE streaming
    /// (`LlmClient::stream`) and forwards text deltas as they arrive so
    /// the UI can render the response incrementally. When false (default)
    /// the loop calls `LlmClient::complete` and emits a single bundled
    /// `TextDelta` event per turn.
    pub enable_streaming: bool,

    // ── Optional wiring ─────────────────────────────────────────────────
    /// Sqlite handle — only wired when this harness persists messages.
    /// **None** for fish and any other ephemeral loop.
    pub persistence: Option<Arc<Mutex<Database>>>,
    /// Rolling-summary store (p7). None => async summary disabled.
    pub summary_store: Option<Arc<dyn SummaryStore>>,
    /// State frame provider (p6). None => no synthetic frame message.
    pub frame_provider: Option<Arc<dyn StateFrameProvider>>,
    /// Optional shared plan-state map. The kernel uses this to check for
    /// unfinished todo items when the model wants to exit. Hosts that have
    /// a planning UI (desktop) wire their `AppState.plan_state` here; hosts
    /// without a plan UI (CLI, fish) leave it as `None`.
    pub plan_state: Option<crate::agent::loop_::PlanStateHandle>,
    /// Optional override model used to produce rolling summaries (p7).
    /// `None` means "reuse the main model".
    pub summary_model: Option<String>,
    /// Optional host lifecycle hooks (tool before/after, context events).
    /// Wired by hosts that observe the loop (e.g. AgentZ's file journal).
    pub hooks: Option<Arc<dyn crate::agent::hooks::AgentHooks>>,
    /// Optional pluggable compaction policy. `None` uses the kernel's built-in
    /// [`crate::agent::loop_::DefaultCompaction`].
    pub compaction_strategy: Option<Arc<dyn crate::agent::compaction_strategy::CompactionStrategy>>,
    /// Optional pluggable project-context discovery strategy. `None` means the
    /// host injects context by other means (the legacy behaviour); when set,
    /// callers can resolve project instructions through this strategy.
    pub context_manager: Option<Arc<dyn crate::context::ContextManager>>,
    /// Optional pluggable long-term memory backend. `None` means no memory
    /// plugin is wired (ephemeral). When set, hosts can store / retrieve
    /// memories through a uniform interface.
    pub memory_plugin: Option<Arc<dyn crate::memory::plugin::MemoryPlugin>>,
    /// Optional template used to format retrieved memories into the system
    /// prompt. `{memories}` is replaced with the rendered hit list. `None`
    /// uses the loop's built-in default heading. Only meaningful when
    /// `memory_plugin` is set.
    pub memory_retrieval_prompt: Option<String>,
    /// Optional pluggable loop-control strategy. `None` uses the kernel's
    /// built-in ReAct control flow.
    pub loop_strategy: Option<Arc<dyn crate::agent::loop_strategy::LoopStrategy>>,
    /// Same-model transient retries. Cloud DimRouter hosts set this to 1.
    pub same_model_transient_retries: u32,
}

impl HarnessConfig {
    /// Start a builder from the minimum required fields. All optional
    /// wiring defaults to `None`, which is the fish-style configuration.
    pub fn builder(
        scene: impl Into<String>,
        model: impl Into<String>,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
    ) -> HarnessConfigBuilder {
        HarnessConfigBuilder::new(scene, model, registry, policy)
    }

    /// Render project-context instructions using the configured
    /// [`context_manager`](Self::context_manager), falling back to the default
    /// [`crate::context::ProjectContextManager`] when none is wired.
    ///
    /// This is behaviour-preserving: with no manager set the output is
    /// identical to the legacy [`crate::project_context`] code path, so hosts
    /// can adopt it without changing existing prompts.
    pub fn render_project_context(
        &self,
        root: &std::path::Path,
        budget_chars: usize,
    ) -> std::io::Result<String> {
        use crate::context::ContextManager as _;
        match &self.context_manager {
            Some(mgr) => mgr.render(root, budget_chars),
            None => crate::context::ProjectContextManager.render(root, budget_chars),
        }
    }

    // ── Scene factories ────────────────────────────────────────────────
    //
    // Each factory encodes the *defaults* that used to live as inline
    // struct literals at the call site. Call sites supply only the
    // runtime-varying pieces (system prompt, model, tokens, vision, etc.)
    // and the factory wires in the rest.

    /// Piscis main-chat harness. Persists to DB, reacts to UI, full
    /// confirmation flags, async summary + state frame enabled.
    #[allow(clippy::too_many_arguments)]
    pub fn for_main_chat(
        model: String,
        fallback_models: Vec<String>,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
        system_prompt: String,
        max_tokens: u32,
        context_window: u32,
        confirm_flags: crate::agent::loop_::ConfirmFlagsHandle,
        vision_override: Option<bool>,
        vision_delegate: Option<Box<dyn crate::llm::LlmClient>>,
        vision_model: String,
        auto_compact_input_tokens_threshold: u32,
        compaction: CompactionSettings,
        db: Arc<Mutex<Database>>,
        plan_state: crate::agent::loop_::PlanStateHandle,
    ) -> Self {
        HarnessConfigBuilder::new("main", model, registry, policy)
            .with_fallback_models(fallback_models)
            .with_layered_prompt(LayeredPrompt::from_monolithic(system_prompt))
            .with_max_tokens(max_tokens)
            .with_context_window(context_window)
            .with_shared_confirm_flags(confirm_flags)
            .with_vision_override(vision_override)
            .with_vision_delegate(vision_delegate)
            .with_vision_model(vision_model)
            .with_auto_compact_threshold(auto_compact_input_tokens_threshold)
            .with_compaction_settings(&compaction)
            .with_persistence(db)
            .with_plan_state(plan_state)
            .build()
    }

    /// Headless main-chat harness (no UI, no user confirmations).
    /// Used by `run_agent_headless` and similar server/trigger paths
    /// that still want the main-chat scene policy but without blocking
    /// on interactive confirmations.
    #[allow(clippy::too_many_arguments)]
    pub fn for_main_headless(
        model: String,
        fallback_models: Vec<String>,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
        system_prompt: String,
        max_tokens: u32,
        context_window: u32,
        vision_override: Option<bool>,
        vision_delegate: Option<Box<dyn crate::llm::LlmClient>>,
        vision_model: String,
        auto_compact_input_tokens_threshold: u32,
        compaction: CompactionSettings,
        db: Arc<Mutex<Database>>,
    ) -> Self {
        HarnessConfigBuilder::new("main_headless", model, registry, policy)
            .with_fallback_models(fallback_models)
            .with_layered_prompt(LayeredPrompt::from_monolithic(system_prompt))
            .with_max_tokens(max_tokens)
            .with_context_window(context_window)
            .with_vision_override(vision_override)
            .with_vision_delegate(vision_delegate)
            .with_vision_model(vision_model)
            .with_auto_compact_threshold(auto_compact_input_tokens_threshold)
            .with_compaction_settings(&compaction)
            .with_persistence(db)
            .build()
    }

    /// Koi sub-agent harness. Shares DB with piscis (so koi-produced
    /// messages persist in its own session), but usually no confirm
    /// prompts (koi runs semi-autonomously).
    #[allow(clippy::too_many_arguments)]
    pub fn for_koi(
        model: String,
        fallback_models: Vec<String>,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
        system_prompt: String,
        max_tokens: u32,
        context_window: u32,
        vision_override: Option<bool>,
        auto_compact_input_tokens_threshold: u32,
        compaction: CompactionSettings,
        db: Option<Arc<Mutex<Database>>>,
        plan_state: Option<crate::agent::loop_::PlanStateHandle>,
    ) -> Self {
        let mut b = HarnessConfigBuilder::new("koi", model, registry, policy)
            .with_fallback_models(fallback_models)
            .with_layered_prompt(LayeredPrompt::from_monolithic(system_prompt))
            .with_max_tokens(max_tokens)
            .with_context_window(context_window)
            .with_vision_override(vision_override)
            .with_auto_compact_threshold(auto_compact_input_tokens_threshold)
            .with_compaction_settings(&compaction);
        if let Some(db) = db {
            b = b.with_persistence(db);
        }
        if let Some(h) = plan_state {
            b = b.with_plan_state(h);
        }
        b.build()
    }

    /// Fish ephemeral harness. Zero persistence, zero confirmations,
    /// UI only for progress forwarding.
    #[allow(clippy::too_many_arguments)]
    pub fn for_fish(
        model: String,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
        system_prompt: String,
        max_tokens: u32,
        vision_capable: bool,
        auto_compact_input_tokens_threshold: u32,
        compaction: CompactionSettings,
        plan_state: crate::agent::loop_::PlanStateHandle,
    ) -> Self {
        HarnessConfigBuilder::new("fish", model, registry, policy)
            .with_layered_prompt(LayeredPrompt::from_monolithic(system_prompt))
            .with_max_tokens(max_tokens)
            .with_vision_override(Some(vision_capable))
            .with_auto_compact_threshold(auto_compact_input_tokens_threshold)
            .with_compaction_settings(&compaction)
            .with_plan_state(plan_state)
            .build()
    }

    /// Scheduled-task harness. Persists to DB under a dedicated
    /// scheduler session; no UI surface.
    #[allow(clippy::too_many_arguments)]
    pub fn for_scheduler(
        model: String,
        fallback_models: Vec<String>,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
        system_prompt: String,
        max_tokens: u32,
        context_window: u32,
        vision_override: Option<bool>,
        auto_compact_input_tokens_threshold: u32,
        compaction: CompactionSettings,
        db: Arc<Mutex<Database>>,
    ) -> Self {
        HarnessConfigBuilder::new("scheduler", model, registry, policy)
            .with_fallback_models(fallback_models)
            .with_layered_prompt(LayeredPrompt::from_monolithic(system_prompt))
            .with_max_tokens(max_tokens)
            .with_context_window(context_window)
            .with_vision_override(vision_override)
            .with_auto_compact_threshold(auto_compact_input_tokens_threshold)
            .with_compaction_settings(&compaction)
            .with_persistence(db)
            .build()
    }

    /// Debug / trial harness. May or may not persist depending on the
    /// scenario; caller provides `db` explicitly.
    #[allow(clippy::too_many_arguments)]
    pub fn for_debug(
        model: String,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
        system_prompt: String,
        max_tokens: u32,
        context_window: u32,
        vision_override: Option<bool>,
        compaction: CompactionSettings,
        db: Option<Arc<Mutex<Database>>>,
        plan_state: Option<crate::agent::loop_::PlanStateHandle>,
    ) -> Self {
        let mut b = HarnessConfigBuilder::new("debug", model, registry, policy)
            .with_layered_prompt(LayeredPrompt::from_monolithic(system_prompt))
            .with_max_tokens(max_tokens)
            .with_context_window(context_window)
            .with_vision_override(vision_override)
            .with_compaction_settings(&compaction);
        if let Some(db) = db {
            b = b.with_persistence(db);
        }
        if let Some(h) = plan_state {
            b = b.with_plan_state(h);
        }
        b.build()
    }

    /// Post-construction override for the streaming flag. The scene
    /// factories (`for_main_chat`, `for_koi`, …) accept a fixed argument
    /// list to stay readable; hosts that need to toggle streaming at
    /// runtime chain this setter between the factory and `into_agent_loop`.
    pub fn with_streaming(mut self, enabled: bool) -> Self {
        self.enable_streaming = enabled;
        self
    }

    /// Post-construction setter for host lifecycle hooks. Lets scene factories
    /// (`for_scheduler`, …) keep fixed argument lists while hosts attach hooks
    /// before `into_agent_loop`.
    pub fn with_hooks(mut self, hooks: Arc<dyn crate::agent::hooks::AgentHooks>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    /// DimRouter already failovers the same catalogue id across suppliers;
    /// cloud-gateway hosts pass 1. Direct official APIs keep the default of 3.
    pub fn with_same_model_transient_retries(mut self, retries: u32) -> Self {
        self.same_model_transient_retries = retries.max(1);
        self
    }

    /// Post-construction setter for the compaction policy (see `with_hooks`).
    pub fn with_compaction_strategy(
        mut self,
        strategy: Arc<dyn crate::agent::compaction_strategy::CompactionStrategy>,
    ) -> Self {
        self.compaction_strategy = Some(strategy);
        self
    }

    /// Bridge: turn a harness config into the legacy
    /// [`crate::agent::loop_::AgentLoop`] value that existing call sites
    /// consume. p1 uses this during the incremental migration; later
    /// phases (p12) move the per-provider request shaping into a
    /// dedicated `RequestBuilder` so the bridge becomes narrower.
    ///
    /// `notification_rx` and `confirmation_responses` are per-run
    /// plumbing and stay on the call site — they aren't baked into the
    /// config.
    pub fn into_agent_loop(
        self,
        client: Box<dyn crate::llm::LlmClient>,
        notification_rx: Option<tokio::sync::mpsc::Receiver<String>>,
        confirmation_responses: Option<crate::agent::loop_::ConfirmationResponseMap>,
    ) -> crate::agent::loop_::AgentLoop {
        crate::agent::loop_::AgentLoop {
            client,
            registry: self.registry,
            policy: self.policy,
            system_prompt: self.layered_prompt.render(),
            model: self.model,
            max_tokens: self.max_tokens,
            context_window: self.context_window,
            budget: self.budget,
            fallback_models: self.fallback_models,
            db: self.persistence,
            plan_state: self.plan_state,
            confirmation_responses,
            confirm_flags: self.confirm_flags.clone(),
            vision_override: self.vision_override,
            vision_delegate: self.vision_delegate,
            vision_model: self.vision_model,
            notification_rx: notification_rx.map(tokio::sync::Mutex::new),
            auto_compact_input_tokens_threshold: self.auto_compact_input_tokens_threshold,
            enable_streaming: self.enable_streaming,
            hooks: self.hooks,
            compaction_strategy: self
                .compaction_strategy
                .unwrap_or_else(|| Arc::new(crate::agent::loop_::DefaultCompaction)),
            memory_plugin: self.memory_plugin,
            context_manager: self.context_manager,
            memory_retrieval_prompt: self.memory_retrieval_prompt,
            loop_strategy: self.loop_strategy,
            same_model_transient_retries: self.same_model_transient_retries.max(1),
        }
    }
}

/// Builder. All `with_*` methods consume and return `self` for chaining.
pub struct HarnessConfigBuilder {
    inner: HarnessConfig,
    tier_percents: (u8, u8, u8),
}

impl HarnessConfigBuilder {
    fn new(
        scene: impl Into<String>,
        model: impl Into<String>,
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyGate>,
    ) -> Self {
        let model = model.into();
        let provider_kind = ProviderKind::from_model_id(&model);
        let inner = HarnessConfig {
            scene: scene.into(),
            model,
            fallback_models: Vec::new(),
            max_tokens: 4_096,
            context_window: 0,
            layered_prompt: LayeredPrompt::default(),
            registry,
            policy,
            confirm_flags: crate::agent::loop_::confirm_flags_handle(true, true),
            vision_override: None,
            vision_delegate: None,
            vision_model: String::new(),
            budget: LayeredBudget::default(),
            provider_kind,
            auto_compact_input_tokens_threshold: 0,
            enable_streaming: false,
            persistence: None,
            summary_store: None,
            frame_provider: None,
            plan_state: None,
            summary_model: None,
            hooks: None,
            compaction_strategy: None,
            context_manager: None,
            memory_plugin: None,
            memory_retrieval_prompt: None,
            loop_strategy: None,
            same_model_transient_retries: 3,
        };
        Self {
            inner,
            tier_percents: (
                super::budget::DEFAULT_TIER_MICRO_PERCENT,
                super::budget::DEFAULT_TIER_AUTO_PERCENT,
                super::budget::DEFAULT_TIER_FULL_PERCENT,
            ),
        }
    }

    /// Wire host lifecycle hooks into the loop.
    pub fn with_hooks(mut self, hooks: Arc<dyn crate::agent::hooks::AgentHooks>) -> Self {
        self.inner.hooks = Some(hooks);
        self
    }

    /// Wire a pluggable project-context discovery strategy.
    pub fn with_context_manager(
        mut self,
        manager: Arc<dyn crate::context::ContextManager>,
    ) -> Self {
        self.inner.context_manager = Some(manager);
        self
    }

    /// Wire a pluggable long-term memory backend.
    pub fn with_memory_plugin(
        mut self,
        memory: Arc<dyn crate::memory::plugin::MemoryPlugin>,
    ) -> Self {
        self.inner.memory_plugin = Some(memory);
        self
    }

    /// Set the template used to format retrieved memories into the prompt.
    /// Empty strings are treated as `None`.
    pub fn with_memory_retrieval_prompt(mut self, tpl: Option<String>) -> Self {
        self.inner.memory_retrieval_prompt = tpl.filter(|s| !s.trim().is_empty());
        self
    }

    /// Wire a pluggable loop-control strategy.
    pub fn with_loop_strategy(
        mut self,
        strategy: Arc<dyn crate::agent::loop_strategy::LoopStrategy>,
    ) -> Self {
        self.inner.loop_strategy = Some(strategy);
        self
    }

    /// Override the proactive compaction policy.
    pub fn with_compaction_strategy(
        mut self,
        strategy: Arc<dyn crate::agent::compaction_strategy::CompactionStrategy>,
    ) -> Self {
        self.inner.compaction_strategy = Some(strategy);
        self
    }

    pub fn with_fallback_models(mut self, models: Vec<String>) -> Self {
        self.inner.fallback_models = models;
        self
    }

    /// DimRouter already failovers the same model across suppliers; pass 1.
    pub fn with_same_model_transient_retries(mut self, retries: u32) -> Self {
        self.inner.same_model_transient_retries = retries.max(1);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.inner.max_tokens = max_tokens;
        self.refresh_budget();
        self
    }

    pub fn with_context_window(mut self, context_window: u32) -> Self {
        self.inner.context_window = context_window;
        self.refresh_budget();
        self
    }

    pub fn with_layered_prompt(mut self, lp: LayeredPrompt) -> Self {
        self.inner.layered_prompt = lp;
        self
    }

    pub fn with_confirm_flags(mut self, cf: ConfirmFlags) -> Self {
        self.inner.confirm_flags =
            crate::agent::loop_::confirm_flags_handle(cf.confirm_shell, cf.confirm_file_write);
        self
    }

    /// Wire a shared confirmation handle (e.g. from desktop `AppState`).
    pub fn with_shared_confirm_flags(
        mut self,
        handle: crate::agent::loop_::ConfirmFlagsHandle,
    ) -> Self {
        self.inner.confirm_flags = handle;
        self
    }

    pub fn with_vision_override(mut self, ov: Option<bool>) -> Self {
        self.inner.vision_override = ov;
        self
    }

    pub fn with_vision_delegate(mut self, client: Option<Box<dyn crate::llm::LlmClient>>) -> Self {
        self.inner.vision_delegate = client;
        self
    }

    pub fn with_vision_model(mut self, model: String) -> Self {
        self.inner.vision_model = model;
        self
    }

    pub fn with_auto_compact_threshold(mut self, tokens: u32) -> Self {
        self.inner.auto_compact_input_tokens_threshold = tokens;
        self
    }

    pub fn with_streaming(mut self, enabled: bool) -> Self {
        self.inner.enable_streaming = enabled;
        self
    }

    pub fn with_tier_percents(mut self, micro: u8, auto: u8, full: u8) -> Self {
        self.tier_percents = super::budget::normalize_tier_percents(micro, auto, full);
        self.inner.budget = self.inner.budget.with_tier_percents(
            self.tier_percents.0,
            self.tier_percents.1,
            self.tier_percents.2,
        );
        self
    }

    pub fn with_max_tool_result_tokens(mut self, tokens: u32) -> Self {
        self.inner.budget = self.inner.budget.with_max_tool_result_tokens(tokens);
        self
    }

    /// Apply a full `CompactionSettings` bundle — tier percents, per-
    /// tool-result cap, and the optional summary-model override — in a
    /// single call. Used by scene factories to keep their signatures
    /// narrow.
    pub fn with_compaction_settings(mut self, c: &CompactionSettings) -> Self {
        self.tier_percents =
            super::budget::normalize_tier_percents(c.micro_percent, c.auto_percent, c.full_percent);
        self.inner.budget = self
            .inner
            .budget
            .with_tier_percents(
                self.tier_percents.0,
                self.tier_percents.1,
                self.tier_percents.2,
            )
            .with_max_tool_result_tokens(c.max_tool_result_tokens);
        self.inner.summary_model = c.summary_model.clone().filter(|s| !s.trim().is_empty());
        self
    }

    pub fn with_persistence(mut self, db: Arc<Mutex<Database>>) -> Self {
        self.inner.persistence = Some(db);
        self
    }

    pub fn with_summary_store(mut self, store: Arc<dyn SummaryStore>) -> Self {
        self.inner.summary_store = Some(store);
        self
    }

    pub fn with_frame_provider(mut self, provider: Arc<dyn StateFrameProvider>) -> Self {
        self.inner.frame_provider = Some(provider);
        self
    }

    pub fn with_plan_state(mut self, h: crate::agent::loop_::PlanStateHandle) -> Self {
        self.inner.plan_state = Some(h);
        self
    }

    /// Set the optional summary-model override (p5a / p7). Empty strings
    /// and `None` both mean "reuse the main model".
    pub fn with_summary_model(mut self, m: Option<String>) -> Self {
        self.inner.summary_model = m.filter(|s| !s.trim().is_empty());
        self
    }

    pub fn build(self) -> HarnessConfig {
        self.inner
    }

    fn refresh_budget(&mut self) {
        // Re-derive caps from the current (context_window, max_tokens)
        // while preserving any previously-set tier percents / tool cap.
        let prev = self.inner.budget;
        self.inner.budget =
            LayeredBudget::from_context_window(self.inner.context_window, self.inner.max_tokens)
                .with_tier_percents(
                    self.tier_percents.0,
                    self.tier_percents.1,
                    self.tier_percents.2,
                )
                .with_max_tool_result_tokens(prev.max_tool_result_tokens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool::ToolRegistry;
    use crate::policy::gate::PolicyGate;
    use std::path::PathBuf;

    fn scaffolding() -> (Arc<ToolRegistry>, Arc<PolicyGate>) {
        let reg = Arc::new(ToolRegistry::new());
        let policy = Arc::new(PolicyGate::new(PathBuf::from(".")));
        (reg, policy)
    }

    #[test]
    fn builder_defaults_to_fish_style_no_persistence() {
        let (reg, policy) = scaffolding();
        let cfg = HarnessConfig::builder("fish", "claude-haiku-4.5", reg, policy).build();
        assert_eq!(cfg.scene, "fish");
        assert_eq!(cfg.provider_kind, ProviderKind::Anthropic);
        assert!(cfg.persistence.is_none());
        assert!(cfg.summary_store.is_none());
        assert!(cfg.frame_provider.is_none());
        assert!(cfg.plan_state.is_none());
        assert_eq!(cfg.fallback_models, Vec::<String>::new());
    }

    #[test]
    fn builder_respects_context_window_and_tier_overrides() {
        let (reg, policy) = scaffolding();
        let cfg = HarnessConfig::builder("main", "claude-sonnet-4.5", reg, policy)
            .with_context_window(200_000)
            .with_max_tokens(8_192)
            .with_tier_percents(55, 75, 92)
            .with_max_tool_result_tokens(6_000)
            .build();
        assert!(cfg.budget.total > 150_000, "total={}", cfg.budget.total);
        assert_eq!(cfg.budget.max_tool_result_tokens, 6_000);
        // Tier ordering preserved.
        assert!(cfg.budget.trigger_micro < cfg.budget.trigger_auto);
        assert!(cfg.budget.trigger_auto < cfg.budget.trigger_full);
    }

    #[test]
    fn builder_refreshes_exact_default_thresholds_for_context_and_max_output_changes() {
        let build = |context_window, max_tokens| {
            let (reg, policy) = scaffolding();
            HarnessConfig::builder("main", "test-model", reg, policy)
                .with_context_window(context_window)
                .with_max_tokens(max_tokens)
                .build()
                .budget
        };
        let budget_32k = build(32_000, 4_096);
        let budget_128k = build(128_000, 4_096);
        let budget_128k_larger_output = build(128_000, 16_384);

        for budget in [budget_32k, budget_128k, budget_128k_larger_output] {
            assert_eq!(budget.trigger_micro, budget.total * 60 / 100);
            assert_eq!(budget.trigger_auto, budget.total * 80 / 100);
            assert_eq!(budget.trigger_full, budget.total * 95 / 100);
        }
        assert!(budget_128k.trigger_micro > budget_32k.trigger_micro);
        assert!(budget_128k_larger_output.trigger_micro < budget_128k.trigger_micro);
        assert!(budget_128k_larger_output.trigger_auto < budget_128k.trigger_auto);
        assert!(budget_128k_larger_output.trigger_full < budget_128k.trigger_full);
    }

    #[test]
    fn builder_refresh_preserves_custom_tiers_and_tool_cap() {
        let (reg, policy) = scaffolding();
        let config = HarnessConfig::builder("main", "test-model", reg, policy)
            .with_context_window(32_000)
            .with_tier_percents(55, 75, 92)
            .with_max_tool_result_tokens(6_000)
            .with_context_window(128_000)
            .with_max_tokens(16_384)
            .build();
        let budget = config.budget;

        assert_eq!(budget.trigger_micro, budget.total * 55 / 100);
        assert_eq!(budget.trigger_auto, budget.total * 75 / 100);
        assert_eq!(budget.trigger_full, budget.total * 92 / 100);
        assert_eq!(budget.max_tool_result_tokens, 6_000);

        let agent = config.into_agent_loop(
            crate::llm::build_client("openai", "test-key", None),
            None,
            None,
        );
        assert_eq!(agent.budget.total, budget.total);
        assert_eq!(agent.budget.trigger_micro, budget.trigger_micro);
        assert_eq!(agent.budget.trigger_auto, budget.trigger_auto);
        assert_eq!(agent.budget.trigger_full, budget.trigger_full);
        assert_eq!(
            agent.budget.max_tool_result_tokens,
            budget.max_tool_result_tokens
        );
    }

    #[test]
    fn builder_refresh_preserves_compaction_settings_percents() {
        let (reg, policy) = scaffolding();
        let settings = CompactionSettings {
            micro_percent: 55,
            auto_percent: 75,
            full_percent: 92,
            max_tool_result_tokens: 6_000,
            summary_model: None,
        };
        let budget = HarnessConfig::builder("main", "test-model", reg, policy)
            .with_compaction_settings(&settings)
            .with_context_window(3)
            .with_max_tokens(1)
            .with_context_window(128_000)
            .with_max_tokens(16_384)
            .build()
            .budget;

        assert_eq!(budget.trigger_micro, budget.total * 55 / 100);
        assert_eq!(budget.trigger_auto, budget.total * 75 / 100);
        assert_eq!(budget.trigger_full, budget.total * 92 / 100);
        assert_eq!(budget.max_tool_result_tokens, 6_000);
    }

    #[test]
    fn builder_refresh_preserves_custom_percents_through_101_and_tiny_totals() {
        let (reg, policy) = scaffolding();
        let budget = HarnessConfig::builder("main", "test-model", reg, policy)
            .with_context_window(120)
            .with_max_tokens(1)
            .with_tier_percents(55, 75, 92)
            .with_context_window(3)
            .with_max_tokens(1)
            .with_context_window(32_000)
            .with_max_tokens(4_096)
            .with_context_window(128_000)
            .with_max_tokens(16_384)
            .build()
            .budget;

        assert_eq!(budget.trigger_micro, budget.total * 55 / 100);
        assert_eq!(budget.trigger_auto, budget.total * 75 / 100);
        assert_eq!(budget.trigger_full, budget.total * 92 / 100);
    }

    #[test]
    fn builder_refresh_preserves_default_percents_through_101_and_tiny_totals() {
        let (reg, policy) = scaffolding();
        let budget = HarnessConfig::builder("main", "test-model", reg, policy)
            .with_context_window(120)
            .with_max_tokens(1)
            .with_context_window(3)
            .with_max_tokens(1)
            .with_context_window(128_000)
            .with_max_tokens(16_384)
            .build()
            .budget;

        assert_eq!(budget.trigger_micro, budget.total * 60 / 100);
        assert_eq!(budget.trigger_auto, budget.total * 80 / 100);
        assert_eq!(budget.trigger_full, budget.total * 95 / 100);
    }

    #[test]
    fn provider_kind_inference_covers_common_models() {
        assert_eq!(
            ProviderKind::from_model_id("claude-sonnet-4.5"),
            ProviderKind::Anthropic
        );
        assert_eq!(
            ProviderKind::from_model_id("gpt-5-mini"),
            ProviderKind::OpenAi
        );
        assert_eq!(
            ProviderKind::from_model_id("o1-preview"),
            ProviderKind::OpenAi
        );
        assert_eq!(ProviderKind::from_model_id("qwen-max"), ProviderKind::Other);
    }

    #[test]
    fn confirm_flags_round_trip_with_agent_loop_flags() {
        let cf = ConfirmFlags {
            confirm_shell: true,
            confirm_file_write: false,
        };
        let loop_cf: crate::agent::loop_::ConfirmFlags = cf.into();
        assert!(loop_cf.confirm_shell);
        assert!(!loop_cf.confirm_file_write);
        let round: ConfirmFlags = loop_cf.into();
        assert!(round.confirm_shell);
        assert!(!round.confirm_file_write);
    }
}
