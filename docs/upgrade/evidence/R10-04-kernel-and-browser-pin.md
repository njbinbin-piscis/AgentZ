# R10-04 Evidence: Kernel 5-Phase Contract & RobotZ Browser Snapshot Generation Pin

- **Task ID**: `R10-04` (`r10-04-kernel-browser-pin`)
- **Date**: 2026-10-19
- **Status**: Completed

## 1. Changes Implemented

1. **Agent Kernel 5-Phase Contract Gate (`crates/agentz-host/tests/kernel_contract.rs`)**:
   - Added contract integration tests verifying strict phase ordering (`prepare -> llm_turn -> tool_dispatch -> verify -> finalize`) and terminal outcomes (`Completed`, `Cancelled`, `MaxIterations`, `BudgetExceeded`).
2. **RobotZ Browser Snapshot Generation & Stale Ref Guard (`src-tauri/src/browser/mod.rs`, `src-tauri/src/browser/activity.rs`, `src-tauri/src/commands/chat_turn.rs`, `src/services/tauri/browser.ts`)**:
   - Implemented `BrowserGenerationTracker` and `GuardedBrowserTool`, tagging snapshots with `[snapshot_generation=<gen>]` and failing closed (`StaleElementRef`) when an element ref from a previous generation is clicked/filled after navigation.
   - Upgraded `BrowserActivity` to reference-counted `active_turns: Arc<AtomicUsize>` so concurrent multi-task turns do not prematurely release `agent_active` in `close_guard()`.

## 2. Verification Commands & Outputs

- `cargo test --manifest-path crates/agentz-host/Cargo.toml --test kernel_contract`:
  - `test result: ok. 4 passed; 0 failed; 0 ignored`
- `cargo test --manifest-path src-tauri/Cargo.toml --lib browser::`:
  - `test browser::tests::snapshot_generation_rejects_stale_element_refs_after_navigation ... ok`
  - `test browser::activity::tests::concurrent_turns_preserve_agent_active_guard ... ok`
  - `test browser::tests::guarded_browser_tool_fails_closed_on_stale_ref_without_launching_browser ... ok`
  - `test result: ok. 3 passed; 0 failed; 1 ignored`
