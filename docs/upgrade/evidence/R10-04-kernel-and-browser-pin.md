# R10-04 Evidence: Kernel 5-Phase Contract & RobotZ Browser Snapshot Generation Pin

- **Task ID**: `R10-04` (`r10-04-kernel-browser-pin`)
- **Date**: 2026-10-19 (updated 2026-11-28 for AgentZ 1.0.81 / `robotz-browser` v0.1.15)
- **Status**: Completed

## 0. Navigation settle pin (AgentZ 1.0.81)

- **Root cause**: `chromiumoxide` `Page::goto` waits until the main frame and every child frame fire `load` (`REQUEST_TIMEOUT` 30s). Hanging images/iframes surfaced as `navigation failed` / `Request timed out.`
- **Pin**: `vendor/robotz/crates/robotz-browser` v0.1.15 exports `nav::navigate_until_interactive`. AgentZ panel navigate and RobotZ `BrowserTool::{navigate,new_tab}` call it instead of `goto`.
- **Verify**: `cargo test --manifest-path vendor/robotz/crates/robotz-browser/Cargo.toml --lib nav:: -- --ignored` and `cargo test --manifest-path src-tauri/Cargo.toml --lib browser::tests::navigate_slow_subresource_live -- --ignored`.

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
