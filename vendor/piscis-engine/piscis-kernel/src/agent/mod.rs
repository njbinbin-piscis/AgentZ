pub mod bench_compact;
pub mod compaction;
pub mod compaction_strategy;
pub mod contrib;
pub mod file_journal;
pub mod harness;
pub mod hooks;
pub mod loop_;
pub mod loop_strategy;
pub mod message_utils;
pub mod messages;
pub mod plan;
pub mod plan_doc;
pub mod rule_preprocess;
pub mod state_frame;
pub mod summary_worker;
pub mod tool;
pub mod tool_completion;
pub mod tool_receipt;
pub mod vision;

#[cfg(test)]
mod compaction_eval;

// `live_smoke.rs` is a real-network/desktop-bound integration test — it needs
// the full desktop `build_registry` helper, the Chrome browser manager and
// `tracing_subscriber`. It therefore lives under the `piscis-desktop` crate
// (`src-tauri/src/live_smoke.rs`) after the kernel extraction.
