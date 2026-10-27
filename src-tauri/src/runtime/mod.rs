//! In-process agent runtimes for AgentZ.
//!
//! [`koi::DesktopInProcessSubagentRuntime`] implements the kernel
//! [`piscis_core::host::SubagentRuntime`] contract by running Koi turns inside
//! the already-running Tauri process (no subprocess / headless binary). It is
//! injected into the kernel pool coordinator so team (Pool) collaboration fans
//! out to member Koi without leaving the desktop process.

pub mod approval;
pub mod background_continuation;
pub mod background_store;
pub mod cancel_completion;
pub mod chat_cancel;
pub mod durable_plan;
pub mod durable_run;
pub mod durable_stream;
pub mod event_replay;
pub mod heartbeat;
pub mod journal_hooks;
pub mod koi;
pub mod owner_lease;
pub mod parent_wake;
pub mod patrol;
pub mod plan_intent;
pub mod project_scope;
pub mod task_activity;
pub mod subagent_bridge;
pub mod subagent_worktree;
pub mod subagent_policy;
pub mod task_budget;
pub mod task_details;
pub mod turn_inbox;
pub mod workflow;
