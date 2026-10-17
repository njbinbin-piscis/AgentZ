//! Pluggable agent-loop control strategy.
//!
//! Historically the agent loop hard-coded its ReAct control flow inline. This
//! trait lifts the *policy* decisions a host might want to vary — how the
//! request context is shaped before each call, whether to stop after a turn,
//! and which model / hints to use next — out of the loop so a host can supply
//! its own strategy (ReAct, Plan-and-Solve, Reflexion, …) without forking the
//! kernel.
//!
//! Boundary of responsibilities:
//! - **Strategy** decides loop-level *policy* at turn boundaries.
//! - **Loop** owns orchestration: LLM calls, tool execution, persistence,
//!   compaction, and event emission.
//!
//! Every method has a behaviour-preserving default, so wiring the built-in
//! [`ReActStrategy`] (or leaving the slot empty) changes nothing.

use std::sync::Arc;

use crate::llm::LlmMessage;

/// Read-only view of the just-finished turn, handed to the strategy at turn
/// boundaries so it can make stop / next-turn decisions.
#[derive(Debug, Clone)]
pub struct TurnContext {
    /// Zero-based iteration index of the turn that just completed.
    pub iteration: usize,
    /// Whether the model issued any tool calls this turn.
    pub had_tool_calls: bool,
    /// The assistant's text output for this turn (may be empty).
    pub last_text: String,
}

/// Hints the strategy can supply for the next turn.
#[derive(Debug, Clone, Default)]
pub struct NextTurnHints {
    /// Override the primary model for the next turn. `None` keeps the
    /// loop's configured model.
    pub model: Option<String>,
}

/// Host-pluggable agent-loop control policy.
pub trait LoopStrategy: Send + Sync {
    /// Stable identifier for diagnostics / config round-tripping.
    fn name(&self) -> &str;

    /// Inspect (and optionally rewrite) the request messages immediately before
    /// they are sent to the LLM. The default returns them unchanged.
    ///
    /// This runs after compaction, demotion and vision handling, so it sees the
    /// exact list that would otherwise go on the wire. Use it for steering
    /// preludes, scratchpad injection, message filtering, etc.
    fn transform_context(&self, messages: Vec<LlmMessage>) -> Vec<LlmMessage> {
        messages
    }

    /// Decide whether to stop the loop after a turn that issued tool calls.
    /// The default never forces an early stop (the loop's own
    /// no-more-tool-calls condition still applies).
    fn should_stop_after_turn(&self, _turn: &TurnContext) -> bool {
        false
    }

    /// Produce hints for the next turn (e.g. a model switch). The default
    /// supplies no hints.
    fn prepare_next_turn(&self, _turn: &TurnContext) -> NextTurnHints {
        NextTurnHints::default()
    }
}

/// The built-in ReAct strategy: reason + act with tool calls until the model
/// stops requesting tools. All hooks use the behaviour-preserving defaults.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReActStrategy;

impl LoopStrategy for ReActStrategy {
    fn name(&self) -> &str {
        "react"
    }
}

/// A named, loop-behaviour-identical strategy used for prompt-pattern variants
/// (Plan-and-Solve, Reflexion). The algorithmic difference these patterns need
/// today lives in the system prompt the host supplies; this carries the name so
/// experiments and telemetry can distinguish them, and gives hosts a concrete
/// type to extend with custom hook overrides later.
#[derive(Debug, Clone)]
pub struct PromptPatternStrategy {
    name: String,
}

impl PromptPatternStrategy {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl LoopStrategy for PromptPatternStrategy {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Resolve a named loop strategy to a shared trait object.
///
/// Built-in names resolve first; unknown names fall through to the runtime
/// [`contrib`](crate::agent::contrib) registry so host-registered strategies are
/// selectable without editing this `match`. Returns `None` only when neither
/// knows the name, so callers can still fail loudly on config typos.
/// `""` and `"react"` resolve to the built-in ReAct strategy.
pub fn resolve_loop_strategy(name: &str) -> Option<Arc<dyn LoopStrategy>> {
    match name {
        "" | "react" | "react_agent" => Some(Arc::new(ReActStrategy)),
        "plan_and_solve" | "plan-and-solve" | "plan_and_solve_agent" => {
            Some(Arc::new(PromptPatternStrategy::new("plan_and_solve")))
        }
        "reflexion" => Some(Arc::new(PromptPatternStrategy::new("reflexion"))),
        _ => crate::agent::contrib::resolve_loop_strategy(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{LlmMessage, MessageContent};

    #[test]
    fn resolves_known_strategies_and_rejects_unknown() {
        assert_eq!(resolve_loop_strategy("react").unwrap().name(), "react");
        assert_eq!(resolve_loop_strategy("").unwrap().name(), "react");
        assert_eq!(
            resolve_loop_strategy("plan_and_solve").unwrap().name(),
            "plan_and_solve"
        );
        assert_eq!(
            resolve_loop_strategy("reflexion").unwrap().name(),
            "reflexion"
        );
        assert!(resolve_loop_strategy("bogus").is_none());
    }

    #[test]
    fn defaults_are_behaviour_preserving() {
        let s = ReActStrategy;
        let msgs = vec![LlmMessage {
            role: "user".into(),
            content: MessageContent::text("hi"),
        }];
        // transform_context is identity.
        let out = s.transform_context(msgs.clone());
        assert_eq!(out.len(), 1);
        // never forces a stop, no model override.
        let turn = TurnContext {
            iteration: 0,
            had_tool_calls: true,
            last_text: String::new(),
        };
        assert!(!s.should_stop_after_turn(&turn));
        assert!(s.prepare_next_turn(&turn).model.is_none());
    }
}
