//! Agent lifecycle hooks.
//!
//! Hooks let a host observe and influence the agent loop without coupling the
//! kernel to any particular host capability. The kernel invokes these at fixed
//! extension points; a host supplies an [`AgentHooks`] implementation through
//! [`crate::agent::harness::config::HarnessConfig`].
//!
//! Design goals:
//! - **Optional**: the loop runs identically when no hooks are wired (`None`).
//! - **Cheap when unused**: default trait methods are no-ops.
//! - **General**: tool lifecycle (before/after) and context-management events
//!   are first-class, so hosts can build journaling, auditing, policy plugins,
//!   undo/replay, telemetry, etc. on top of the same surface.
//!
//! The first consumer is AgentZ's file journal, which snapshots file contents
//! before `file_write` / `file_edit` so the host can offer Cursor-style
//! "Undo All" / replay recovery.

use async_trait::async_trait;
use std::path::Path;

use crate::agent::tool::ToolResult;

/// Information about a tool invocation, shared by the before/after hooks.
///
/// All fields borrow from the loop's call frame, so hook implementations must
/// not hold the event past the `await`. Copy out anything that needs to live
/// longer (e.g. into a journal row).
pub struct ToolHookEvent<'a> {
    /// Session the tool runs under.
    pub session_id: &'a str,
    /// LLM tool-use id for this specific call.
    pub tool_use_id: &'a str,
    /// Tool name (e.g. `"file_write"`).
    pub tool_name: &'a str,
    /// Raw tool input as sent by the model.
    pub input: &'a serde_json::Value,
    /// Workspace root the agent operates in.
    pub workspace_root: &'a Path,
}

/// Context-management lifecycle events (turn boundaries, compaction).
///
/// These let a host delimit work units (for undo grouping) and react to
/// context-window compaction without reaching into loop internals.
pub enum ContextHookEvent<'a> {
    /// A new agent turn is about to start for this session.
    TurnStart { session_id: &'a str },
    /// The current agent turn has finished.
    TurnEnd { session_id: &'a str },
    /// Rolling-summary / context compaction is about to run.
    BeforeCompact {
        session_id: &'a str,
        message_count: usize,
    },
    /// Context compaction has completed.
    AfterCompact {
        session_id: &'a str,
        message_count: usize,
    },
}

/// Outcome of a [`AgentHooks::before_tool`] call.
pub enum HookDecision {
    /// Proceed with the tool call as normal.
    Continue,
    /// Skip the tool call and return this message to the model as an error.
    Deny(String),
}

/// Host-supplied observer / interceptor for the agent loop.
///
/// Every method has a no-op default so hosts implement only what they need.
#[async_trait]
pub trait AgentHooks: Send + Sync {
    /// Called immediately before a tool executes (after policy approval).
    ///
    /// Returning [`HookDecision::Deny`] short-circuits execution. Use this to
    /// capture pre-state (e.g. snapshot a file before it is overwritten).
    async fn before_tool(&self, _ev: &ToolHookEvent<'_>) -> HookDecision {
        HookDecision::Continue
    }

    /// Called immediately after a tool returns, before the result is emitted.
    async fn after_tool(&self, _ev: &ToolHookEvent<'_>, _result: &ToolResult) {}

    /// Called on context-management lifecycle transitions.
    async fn on_context_event(&self, _ev: &ContextHookEvent<'_>) {}
}
