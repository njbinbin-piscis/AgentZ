# R10-13 Evidence: Unified Side-Channel LLM Usage Accounting & Status Bar Model/Cost Pill

- **Date**: 2026-10-19
- **Task**: R10-13 (`r10-13-budget-and-statusbar`)
- **Status**: Completed

## Summary of Changes

1. **Unified Side-Channel LLM Usage Accounting (`src-tauri/src/runtime/task_budget.rs`, `src-tauri/src/commands/chat_turn.rs`, `src-tauri/src/commands/post_turn.rs`, `src-tauri/src/commands/edit.rs`, `src-tauri/src/commands/curator.rs`, `src-tauri/src/commands/memory_consolidate.rs`)**:
   - Added `BudgetScope::guard_with_source(&self, inner, source)` and `record_side_channel_usage(source, input_tokens, output_tokens)` / `query_side_channel_usage()` in `task_budget.rs`.
   - Tagged auto-title (`maybe_autotitle_session_from_first_prompt`), memory extraction (`auto_extract_memories`), repo notes (`auto_extract_repo_notes`), skill curator (`curator.rs`), and memory consolidation (`memory_consolidate.rs`) with `source: "background"`.
   - Tagged `inline_edit` and `ai_inline_completion` in `edit.rs` with `source: "inline_edit"`.
2. **CodeZ Status Bar Model Switcher & Token/Cost Pill (`src/workspaces/codez/IdeStatusBar.tsx`)**:
   - Added quick model switcher button (`[data-testid="statusbar-model-switcher"]`) cycling configured LLM providers and persisting via `saveScopedModelId("codez", ...)`.
   - Added human-readable Token & estimated USD cost pill (`[data-testid="statusbar-token-cost-pill"]`) subscribing to `context_usage` events (`formatCompactTokens`, `estimateTokenCostUsd`).

## Verification

- `vitest run src/workspaces/codez/IdeStatusBar.test.tsx`: **2 passed**.
- `cargo test --manifest-path src-tauri/Cargo.toml --lib runtime::task_budget::tests::usage_accounting_records_turn_background_and_inline_edit_sources`: **passed**.
