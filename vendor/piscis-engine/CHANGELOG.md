# Changelog

## [0.8.63] - 2026-08-13

OpenAI 兼容链路鲁棒性（`model_not_found` / 截断工具参数）整改，见项目内 "DimWork OpenAI 兼容性整改计划"。

### Added

- **`llm::error_class`**：统一的 `ErrorClass`（`model_unavailable` / `tool_args_invalid` / `protocol_error` / `rate_limited` / `auth_failed` / `upstream_transient` / `unknown`）与 `classify_error(&str) -> ErrorClass`，供 fallback 判定、结构化日志、前端文案分类共用一套规则。
- **`llm::json_repair::try_conservative_repair`**：对流式/非流式 tool-call `arguments` 截断的极保守修复——只补齐未闭合的引号/括号，不做任何字段脑补；修复失败照常落回 `tool_args_invalid`。
- **`AgentEvent::Notice`**：非终止性提示事件（模型 fallback 切换、纠偏重试），不会清空前端的运行中状态。
- **`AgentEvent::Error` 结构化字段**：新增 `code` / `details`（均 `#[serde(default)]`，向后兼容旧客户端）。
- **一轮纠偏重试**：`tool_args_invalid` 且修复失败时，AgentLoop 向模型追加一条纠偏消息并重试一次，而不是直接判定为可绝望的失败。
- **`file_write` 超大 content 显式拒绝**：单次调用 `content` 超过 200,000 字符时返回 `content_too_large` 结构化 tool error（提示改用 `file_edit` 分批修改），不再尝试整段写入。
- **`llm::openai::api_error` 结构化日志**：非 2xx 响应记录 `model` / `http_status` / `error_code`；LLM 调用失败与模型 fallback 记录 `model` / `attempt` / `max_attempts` / `error_code` / `fallback_from`。
- **DimWork `formatChatError`**：按 `error.code` 分类展示用户可读摘要 + 可折叠技术详情；模型 fallback / 纠偏重试通过 `notice` 事件明示，不再是无声的行为变化。

### Fixed

- **工具参数 JSON 永不静默降级为 `{}`**：流式（`stream()`）与非流式（`complete()`）路径下，`arguments` 解析失败时不再用空对象兜底执行工具，而是走 `LlmChunk::Error` / `Result::Err`，避免用被截断的错误参数执行 `file_write` 等破坏性工具。
- **空 `choices` / 流未正常收尾 → `protocol_error`**：流结束但未见 `[DONE]` 且有未清空的 tool 参数缓冲区，或整个流未输出任何内容，均归类为 `protocol_error` 而不是被当作正常完成。

## [0.8.62] - 2026-06-13

### Added

- **`list_all_artifacts`**: global My Files aggregation across sessions.
- **Notification `pool_name` and session schema extensions** (from v0.8.61 line).

### Fixed

- **`is_allowed_plan_path`**: cross-platform plan path validation on Windows/macOS (merged from v0.8.60).

## [0.8.60] - 2026-06-13

### Fixed

- **`is_allowed_plan_path`**: cross-platform plan path validation on Windows/macOS (no longer requires the target file to exist; avoids `canonicalize` prefix mismatches).

## [0.8.59] - 2026-06-12

### Added

- **`pool_sessions.team_id` / `workflow_run_id`**：按团队与工作流 run 隔离 pool；`find_active_pool(project_dir, team_id, team_name)`。
- **`assign_koi` dependency waiting**：依赖未满足时 emit `TodoChanged` + `dependency_waiting` pool message。

### Changed

- **`pool_org` 工具描述**：补充 workflow_hint 与 depends_on 语义说明。

## [0.8.55] - 2026-06-06

### Fixed
- **OpenAI tool_calls pairing**: per-`tool_use_id` sanitize strips partially satisfied assistant tool calls (cancel / parallel interrupt / supersede collapse) so the API no longer returns 400 for orphaned `tool_call_id`s.
- **Agent loop cancel**: inject synthetic error `ToolResult`s for unexecuted tools before persisting; sanitize pairing after each tool round and in `build_request_messages` before every LLM call.

## [0.8.54] - 2026-06-08

### Added
- Journal turn diffs and improved agent loop error surfacing.

## [0.8.47] - 2026-06-07

### Added
- **`SceneKind::KoiPersona`**: main-chat persona mode with a dedicated tool registry profile and `koi_persona_*` prompt protocol.
- **`build_koi_persona_system_prompt`**: assembles Koi identity, instructions, and project context for direct user conversation (no pool collaboration tools).

### Changed
- **Vision delegation**: `delegate_vision_analysis` runs per attachment image instead of batching multiple images in one call.

## [0.8.38] - 2026-06-05

### Added
- **Pluggable loop strategies** (`LoopStrategy` trait) and runtime **contrib registry** for host-supplied loop/compaction strategies.
- Built-in compaction modes: `sliding_window`, `vector_retrieval`.
- `HarnessConfig` slots: `loop_strategy`, `memory_retrieval_prompt`.

### Changed
- Crate versions aligned to the **0.8.x** release line (supersedes the mistaken `v0.7.38` tag).
- Consumers should pin `rev = "v0.8.38"` (not `v0.7.38`).

## [0.8.25] - 2026-05-31

- Stricter heartbeat / org_spec convergence (consumed by openpiscis 0.8.25).
