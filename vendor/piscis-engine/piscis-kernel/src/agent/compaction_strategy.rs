//! Pluggable context-compaction strategy.
//!
//! Historically the agent loop hard-coded its proactive Level-2 (rolling
//! summary) compaction inline. This trait lifts that *policy* out of the loop
//! so a host can supply its own algorithm — keep-everything, aggressive
//! summarise, semantic dedup, external vector recall, etc. — without forking
//! the kernel.
//!
//! Boundary of responsibilities:
//! - **Strategy** decides *whether and how* to shrink the in-memory history
//!   before the next LLM call, and (optionally) produces a new rolling summary.
//!   It may call the LLM (via the supplied client) for summarisation.
//! - **Loop** owns orchestration: persistence (DB rolling-summary / state
//!   frame), token accounting, and the `ContextUsage` UI event. It applies the
//!   [`CompactionResult`] and persists only when `changed` is true.
//!
//! The built-in [`crate::agent::loop_::DefaultCompaction`] reproduces the
//! previous inline behavior exactly, so wiring it changes nothing by default.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::agent::harness::LayeredBudget;
use crate::llm::{LlmClient, LlmMessage, ToolDef};

/// Why the loop is asking the strategy to compact. Lets one `compact()` entry
/// own both compaction paths so a custom strategy is never bypassed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionTrigger {
    /// Pre-call, estimate-driven. The strategy MAY decide not to compact
    /// (returns `changed = false`) when the request already fits the budget.
    Proactive,
    /// The provider rejected the request as too large. The strategy MUST
    /// shrink aggressively; `changed = false` here means "cannot recover" and
    /// the loop aborts the turn.
    Overflow,
}

/// How one compaction attempt ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionOutcomeKind {
    /// The summariser ran and the request shrank.
    Ok,
    /// Nothing could be compacted, or the result would not have been smaller.
    Noop,
    /// The summariser was unavailable; history was cut by rule instead.
    Fallback,
    /// The summariser failed and the history was left as it was.
    Failed,
}

impl CompactionOutcomeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompactionOutcomeKind::Ok => "ok",
            CompactionOutcomeKind::Noop => "noop",
            CompactionOutcomeKind::Fallback => "fallback",
            CompactionOutcomeKind::Failed => "failed",
        }
    }
}

/// Result of one compaction attempt, for the log and the context indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactionReport {
    pub outcome: CompactionOutcomeKind,
    /// Tier that triggered the attempt (`auto`, `full`) or `overflow`.
    pub tier: &'static str,
    /// Estimated request tokens before and after the attempt.
    pub before_tokens: usize,
    pub after_tokens: usize,
}

/// State a strategy carries from one attempt to the next within a run. The
/// loop hands back whatever the previous attempt returned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompactionGuard {
    /// Summariser failures in a row.
    pub consecutive_summary_failures: u32,
    /// After an attempt that achieved nothing: do not try again until the
    /// request estimate reaches this value.
    pub retry_at_estimate: Option<usize>,
}

/// Facts about the current turn that must survive compaction word for word.
#[derive(Debug, Clone, Default)]
pub struct CompactionPins {
    /// The user's request for this turn.
    pub user_request: String,
    /// Todo items, one line each.
    pub todos: Vec<String>,
    /// Files read in this run, one line each (path, ranges, count).
    pub files_read: Vec<String>,
}

/// Inputs handed to a [`CompactionStrategy`] before each LLM call.
///
/// Borrows from the loop's call frame except `messages`, which is moved in (the
/// strategy may return it unchanged or a compacted replacement).
pub struct CompactionRequest<'a> {
    /// Why compaction is being requested (proactive vs overflow recovery).
    pub trigger: CompactionTrigger,
    /// Model-derived safe input budget and compaction tier thresholds.
    pub budget: LayeredBudget,
    /// Full in-memory conversation history (newest last).
    pub messages: Vec<LlmMessage>,
    /// Current rolling summary (empty when none yet).
    pub rolling_summary: &'a str,
    /// System prompt — needed for accurate request-token estimation.
    pub system_prompt: &'a str,
    /// Primary model id (used as the summariser model by the default strategy).
    pub model: &'a str,
    /// Max *output* tokens — feeds the input-budget computation.
    pub max_tokens: u32,
    /// Configured input context window (0 = auto).
    pub context_window: u32,
    /// Tool definitions in this turn (request overhead estimation).
    pub tool_defs: &'a [ToolDef],
    /// Map of tool_use_id → minimal receipt, for the demoted request view.
    pub tool_minimals: &'a HashMap<String, String>,
    /// Session id (vision-context injection keys off this).
    pub session_id: &'a str,
    /// Cumulative input tokens billed so far this session.
    pub cumulative_input_tokens: i64,
    /// Threshold at which cumulative-token-driven compaction triggers.
    pub next_auto_compact_threshold: i64,
    /// Threshold step (0 disables cumulative-token-driven compaction).
    pub threshold_step: i64,
    /// LLM client the strategy may use for summarisation.
    pub client: &'a dyn LlmClient,
    /// State returned by the previous attempt in this run.
    pub guard: CompactionGuard,
    /// Turn facts to keep verbatim next to the summary.
    pub pins: &'a CompactionPins,
}

/// What a [`CompactionStrategy`] decided. When `changed` is false the loop
/// keeps its existing state and skips persistence.
#[derive(Default)]
pub struct CompactionResult {
    /// Whether the strategy actually compacted (and the rest is meaningful).
    pub changed: bool,
    /// Replacement message history (the original when unchanged).
    pub messages: Vec<LlmMessage>,
    /// New rolling summary (only meaningful when `changed`).
    pub rolling_summary: String,
    /// Prompt tokens billed to the summariser this call.
    pub summary_input_tokens: u32,
    /// Completion tokens billed to the summariser this call.
    pub summary_output_tokens: u32,
    /// Updated cumulative-token compaction threshold.
    pub next_auto_compact_threshold: i64,
    /// p7 structured plan items extracted by the summariser (for state frame).
    pub structured_plan_items: Vec<String>,
    /// p7 next-step hint extracted by the summariser (for state frame).
    pub structured_next_step_hint: Option<String>,
    /// State to hand to the next attempt. A strategy that keeps none returns
    /// the default.
    pub guard: CompactionGuard,
    /// Set when the strategy attempted a compaction, whether or not it
    /// changed anything. `None` when the request simply fit.
    pub report: Option<CompactionReport>,
}

/// Host-pluggable proactive compaction policy.
#[async_trait]
pub trait CompactionStrategy: Send + Sync {
    /// Inspect the pending request context and optionally compact it.
    async fn compact(&self, req: CompactionRequest<'_>) -> CompactionResult;
}
