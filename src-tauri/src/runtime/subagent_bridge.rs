//! Parent-turn facts the `task` tool needs, filled once the parent harness is built.
//!
//! Writing sub-agents are authorized by the user enabling the agent
//! (`permission_mode` + `delegable`), so no per-call approval chain is bridged.
//! Safety comes from isolation instead: see [`crate::runtime::subagent_worktree`].

use std::sync::{Arc, OnceLock};

#[derive(Clone)]
pub struct ParentBridge {
    /// True when the parent turn itself is read-only (plan mode).
    pub read_only: bool,
}

pub type BridgeSlot = Arc<OnceLock<ParentBridge>>;
