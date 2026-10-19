# R10-17 Evidence: Rust `chat_turn.rs` Modular Split

- **Date**: 2026-10-19
- **Task**: `R10-17` — Split `src-tauri/src/commands/chat_turn.rs` (3,249 lines) into 4 cohesive submodules under `src-tauri/src/commands/chat_turn/` (`< 900` lines each).

## 1. Module Decomposition & Line Counts

Replaced the monolithic `src-tauri/src/commands/chat_turn.rs` (3,249 lines) with `src-tauri/src/commands/chat_turn/`:

| Module | Lines | Responsibility |
|--------|------:|----------------|
| `src-tauri/src/commands/chat_turn/mod.rs` | 885 | `SessionRunningGuard`, `run_agentz_turn` main turn orchestration, and crate-level re-exports (`build_tool_registry`, `load_installed_skills`, `materialize_headless_llm_settings`, `resolve_koi_llm_provider_for_turn`, `resolve_llm_runtime`, `resolve_team_coordinator_model_id`, `run_subagent_research`, `run_subagent_with_prompt`, `skills_context`, `LlmRuntime`, `SettingsSnapshot`, `SkillManifest`) |
| `src-tauri/src/commands/chat_turn/prompt_assembly.rs` | 855 | `SkillManifest`, `load_installed_skills`, `skills_context`, `project_rules_context*`, `wiki_agent_coding_context`, `active_plan_excerpt`, `expand_file_refs*`, `expand_browser_element_refs`, `expand_terminal_snippets`, `build_turn_system_prompt`, and `remote_context_binding_tests` |
| `src-tauri/src/commands/chat_turn/tool_approval.rs` | 836 | `LlmRuntime`, `SettingsSnapshot`, `resolve_llm_runtime`, `auto_route_model`, `materialize_headless_llm_settings`, `resolve_koi_llm_provider_for_turn`, `resolve_team_coordinator_model_id`, `build_tool_registry`, `register_mcp_and_connector_tools`, `run_subagent_research`, `run_subagent_with_prompt`, and `durable_devenv_wiring_tests` |
| `src-tauri/src/commands/chat_turn/post_turn.rs` | 730 | `model_supports_vision`, `resolve_attachment`, `inject_image_block`, `maybe_llm_rename_session_title`, `spawn_turn_event_collector`, and `finalize_turn_and_spawn_background` |

All 4 submodules are strictly below the 900-line ceiling (`max = 885` lines).

## 2. Verification Results

- `cargo test -p agentz-desktop --lib commands::chat_turn`:
  - `commands::chat_turn::tool_approval::durable_devenv_wiring_tests::turn_constructor_has_no_legacy_plan_mutation_capability` — `ok`
  - `commands::chat_turn::prompt_assembly::remote_context_binding_tests::prompt_without_file_refs_needs_no_remote_connection` — `ok`
  - `commands::chat_turn::prompt_assembly::remote_context_binding_tests::context_rejects_a_workspace_outside_its_bound_authority` — `ok`
  - Result: `3 passed; 0 failed`.
