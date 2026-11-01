# 已知问题 — Agent 循环健壮性与工具

[English](known-issues.md) | **简体中文**

> 本文是 [known-issues.md](known-issues.md) 的中文翻译。两者不一致时以英文版为准。

状态：**1.0.0 工程基线已关闭（2026-10-19）**——所有宿主侧工程、WorkZ ↔ CodeZ 交接、脏缓冲区冲突保护、按轮撤销、旁路 token 计量、机密清理以及打包/E2E 闸门均已在 `1.0.0` 中解决并验证（见 `docs/upgrade/release-decision-1.0.0.md` 和 `docs/upgrade/adr/ADR-006-1.0-engineering-and-ux-baseline.md`）。外部硬件/证书事项在 `ADR-006` 下跟踪。下方的历史记录保留以供工程溯源。

负责人：Piscis。创建：2026-09-29。更新：2026-11-02。

## 状态索引（2026-11-02 针对 1.0.40 重新核实）

下方各节中的行号来自 2026-09-29 的快照，已经漂移。请用 `rg` 按符号查找代码，而不是按行号。此后，
`src-tauri/src/commands/chat_turn.rs` 已拆分为 `src-tauri/src/commands/chat_turn/` 模块（`mod.rs`、`post_turn.rs`、`tool_approval.rs`、`prompt_assembly.rs` 等），内核也不再是不透明的 git 依赖：根 `Cargo.toml` 锁定 `piscis-engine` `v0.8.71`，并通过 `[patch]` 块从 `vendor/piscis-engine/` 构建，因此“内核侧”事项现在可以在本仓库中检查和修复。支持范围（各平台验证了什么）在 [AZ-005 支持矩阵](upgrade/evidence/AZ-005-support-matrix.md) 中跟踪；这里的已知问题不改变该矩阵。

| # | 事项 | 2026-11-02 状态 | 核实位置 |
| --- | --- | --- | --- |
| 1 | 整轮 600 秒超时 | **已解决。** 不再有默认的整轮上限：仅当请求设置了 `task_timeout_secs` 时才限制该轮（否则为一年）。超时返回部分输出和 `timed_out` 状态 | `src-tauri/src/commands/chat_turn/mod.rs`（`task_timeout_secs`、`turn_timeout_secs`）、`post_turn.rs` |
| 2 | LLM 请求重试 | **已解决（内核）。** Agent 循环以有上限的指数退避和可中断的休眠重试瞬时 LLM 失败；永久错误（认证、4xx、余额）经 `error_class::is_retryable` 快速失败 | `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs`（`retry_backoff`）、`src/llm/error_class.rs` |
| 3 | 工具超时返回给 agent | **已解决（内核）。** 单个工具超时会变成回传给模型的 `ToolResult::err`；该轮继续 | `loop_.rs`（`Tool '{}' timed out after {}s`） |
| 4 | UAC 提示不得导致硬失败 | **已解决（内核）。** 权限等待上限为 60 秒，超时按“拒绝”处理；无人响应的提权对话框以工具错误（`Elevated command timed out ...`）结束，而不是整轮失败 | `loop_.rs`（权限 `tokio::time::timeout(60s)`）、`piscis-kernel/src/tools/elevate.rs` |
| 5 | CRLF 多行 `file_edit` | **已解决**（修复批次 5；现已在 vendored 内核中） | `piscis-kernel/src/tools/file_write.rs`（`adapt_eol`、`eol_tests`） |
| 6 | 本环境的验证缺口 | 对构建机**已解决**：2026-09-30 的审计用 node/cargo/MSVC 构建并运行了应用 | 见*工具验证审计* |
| 7 | 中止的轮次丢失计划连续性 | **部分未解决。** 宿主部分已修复；计划存储（`state.rs` 中的 `new_plan_store()`）仍在内存中，未完成的 todo 无法跨重启保留 | `src-tauri/src/state.rs` |
| 8 | `plan_todo` 卡在 `in_progress` | **未解决。** 会统计未完成 todo（`open_todos`）并自动续跑，但从不自动标记为 `cancelled`/`pending` | `chat_turn/post_turn.rs` |
| 21 | DeepSeek 工具调用配对 | **已解决（内核）。** `convert_messages` 跟踪 `pending_tool_ids` | `piscis-kernel/src/llm/openai.rs` |
| 22 | DeepSeek / “think” 兼容性 | **部分解决。** `is_deepseek_thinking_model` 不区分大小写，并匹配带网关前缀、`think` 和 `-r1` 的名称；推理过程仍未在 UI 中显示 | `piscis-kernel/src/llm/openai.rs` |

Windows 打包目标：见 README。已验证的产物是 `.github/workflows/release.yml` 上传的 NSIS `.exe` 文件。

---

## 1. `agent turn failed: timed out after 600s` 杀死整轮

**状态（2026-11-02）。** **已解决。** 不再有默认的整轮上限（`chat_turn/mod.rs`：仅在设置 `task_timeout_secs` 时才有上限），LLM 重试和工具超时在内核中按步骤处理（#2、#3）。

**状态（2026-09-29）。** 宿主侧部分**已修复**——见文末*修复批次 1*。Agent 循环内的按步恢复（把超时返回给模型）仍属内核侧。

**现象。** 较长的 agent 轮次（大量工具调用，或某一步很慢）以 `agent turn failed: timed out after 600s` 结束，用户看到硬错误。所有进行中的工具工作都被丢弃。

**发生位置。**

- `src-tauri/src/commands/chat_turn.rs:1741` — `default_timeout = 600s`。
- `src-tauri/src/commands/chat_turn.rs:2235-2252` — 整个 `agent.run(...)` future 被包在 `tokio::time::timeout(timeout, run_fut)` 中。遇到 `Err(_)` 时设置 `cancel`，返回 `Some("timed out after {}s")`，该轮被标记为 `failed`（`chat_turn.rs:2264`）。
- `src-tauri/src/commands/chat.rs:198` — `agent turn failed: {e}` 作为终止错误返回给前端。

**为什么不对。** 对*整轮*设置挂钟预算混淆了三种不同情况：

1. 某个工具调用很慢（应由工具自身的超时/结果处理，而不是整轮）。
2. 模型在等待某事（权限 / 人工验证）——这不是失败。
3. LLM HTTP 请求确实超时。

只有 (3) 才应作为整轮失败暴露，而且只能在重试之后。

**期望行为。**

- 整轮超时不应在步骤中途中止。应优先取消*当前步骤*，并向 agent 返回结构化、非致命的 `tool_result` 错误，让它决定（重试、换路径或停止并解释）。
- 应移除或大幅提高对整轮的单一挂钟上限；真正的保护应在单步层面（LLM 请求、工具执行）。
- 当该轮确实被中止时，应发出可恢复/部分结果，而不是光秃秃的 `failed`。

---

## 2. LLM 请求超时应先重试 3 次再失败

**状态（2026-11-02）。** **已在 vendored 内核中解决。** `piscis-kernel/src/agent/loop_.rs` 用 `retry_backoff`（1 秒、2 秒……）和可中断的休眠重试瞬时 LLM 错误；`llm/error_class.rs` 使认证/4xx/余额错误不可重试。

**状态（2026-09-29）。** **无法在本仓库修复**——LLM 调用位于外部 `piscis-kernel` crate 中。需要上游 `piscis-engine` 修改（重试包装）；宿主只能提供配置。

**现象。** LLM HTTP 读取超时直接冒泡为整轮失败，而不是被重试。

**当前状态。**

- `src-tauri/src/commands/chat_turn.rs:196-215` 用 `llm_read_timeout_secs`（`>= 30`）构建客户端。本仓库中 LLM 调用外**没有重试包装**。
- 重试逻辑只存在于无关路径：`clawhub.rs:73`（`clawhub_get_with_retry`，3 次尝试，指数退避）和 `gateway/wechat.rs:994`（3 次尝试）。Agent 的 LLM 调用没有对应机制。
- 注意：实际的 agent 循环位于外部 `piscis-kernel` crate（`Cargo.toml:23-25`，锁定 `v0.8.62`）——任何重试都必须加在那里，或加在我们拥有的包装层中。

**期望行为。** 用有上限的重试包装 LLM 请求（3 次尝试，指数退避 + 抖动）。只有全部尝试失败后该轮才报告错误，并且该错误应归因于 LLM 请求，而不是整轮。区分可重试（超时 / 5xx / 连接重置）与不可重试（4xx / 402 余额不足）——后者必须快速失败（`AssistantPanel.tsx` 已对 402 做特殊处理）。

---

## 3. 工具调用超时必须返回给 agent，而不是抛出

**状态（2026-11-02）。** **已在 vendored 内核中解决。** 单个工具超时时，`loop_.rs` 记录 `Tool '<name>' timed out after <N>s`，并向模型返回 `ToolResult::err`（一条中文消息，包含工具名和超时秒数，并提示简化或拆分命令），而不是结束该轮。返回体是人类可读的消息，而不是下文提议的机器可读 JSON。

**状态（2026-09-29）。** **内核侧，仍未解决。** 工具执行以及把结果回传的循环都在 `piscis-kernel` 中；宿主无法拦截工具超时。

**现象。** “有时是工具调用超时”——某个工具调用超时，该轮以错误结束，而不是告诉 agent 该工具超时了。

**期望行为。** 工具超时是一个普通的 `tool_result`，带 `is_error = true` 和机器可读的原因（例如 `{"error":"timeout", "timeout_secs":N}`）。Agent 循环必须把它作为该工具的输出回传，让模型可以重试、缩小命令范围或换方法。它绝不能终止该轮。非零退出码和被截断的输出同理。

**UI 说明。** 内联的 `ToolTrace` 组件（`src/components/ToolTrace.tsx`）已经能渲染带可展开消息体的 `error` 状态行——因此一旦后端不再把工具超时转换为整轮失败，返回的工具超时就能正确显示。

---

## 4. 触发 UAC 提示（用户未响应）的工具调用不得硬失败

**状态（2026-11-02）。** **已在 vendored 内核中解决。** `loop_.rs` 中的权限等待是对用户取消和 60 秒 `tokio::time::timeout` 的 `tokio::select!`；无响应按 “User denied this operation” 处理。提权工具（`tools/elevate.rs`）用其 `timeout_secs` 限制 UAC 等待，并返回错误（“The user may have cancelled the UAC dialog ...”），该错误成为工具错误，而不是整轮失败。

**状态（2026-09-29）。** **部分属于宿主侧，仍未解决。** 宿主拥有 `permission_responses` 映射（`interactive.rs`），可以加有界等待，但真正的提权等待发生在内核/工具中。需要内核修改加宿主侧超时策略。

**现象。** “甚至有时是工具调用触发 UAC 而用户没有响应”——某个 shell/工具调用请求提权，Windows UAC 对话框出现，用户没有响应，该轮报错。

**当前状态。**

- 权限管道是一次性的且在内存中：`src-tauri/src/commands/interactive.rs:23` 的 `respond_permission_request` 解析存放在 `state.permission_responses`（`state.rs:52`）中的 `oneshot::Sender<bool>`。如果没人响应，sender 永远不会被触发。
- `chat_turn.rs:2205-2211` 在收到 `PermissionRequest` 时发出 `waiting_permission`，但对它没有超时/处理策略。
- 提权本身发生在 shell 工具内部（外部内核）；本仓库只能看到由此产生的权限请求 / 工具结果。

**期望行为。**

- 无人响应的 UAC / 提权提示**不是**整轮失败。工具应返回 `is_error = true`，原因类似 `"elevation_prompt_unanswered"`（或“需要提权，用户拒绝”的结果），让 agent 可以不提权继续、选择非特权替代方案，或明确询问用户。
- 为 `permission_responses` 增加有界等待：若 N 秒内没有响应，按“未批准”处理并把结构化结果交回 agent，而不是挂起或中止该轮。
- 绝不让 UAC 无限期阻塞 agent 线程。

---

## 5. 工具：`file_edit` 多行替换在 CRLF 文件上失败

**状态（2026-11-02）。** **已解决**——见修复批次 5；该修复随 vendored 内核发布（`piscis-kernel/src/tools/file_write.rs`，`adapt_eol`）。

**状态（2026-09-29）。** **未解决**（工具）。本次会话中再次用一次性 Python 精确单次替换脚本绕过。

**现象（编辑本仓库时观察到）。** 带多行 `old_string` 的 `file_edit` 返回 `match_not_found`，即使片段明显存在，因为工作区文件在磁盘上是 CRLF（`git ls-files --eol` → `w/crlf`），而工具按 LF 规范化后的文本匹配。单行编辑和完整 `file_write` 可用；多行编辑不可靠。

**影响。** 拖慢多处编辑，并在本次会话中导致多次失败尝试（例如 `AssistantPanel.tsx`、`TaskPanel.tsx`）。

**使用过的绕过办法。** (a) 逐帧单行编辑，或 (b) 一次性 Python 脚本：以 `newline=""` 读取，用文件检测到的 `\r\n` 拼接模式，再做精确一次的 `str.replace`。

**期望行为。** `file_edit` 应规范化换行（或同等接受 CRLF/LF），使多行 `old_string` 无论文件磁盘换行为何都能匹配。在此之前，本仓库优先使用单行锚点或 `file_write`。

---

## 6. 本环境的验证缺口

**状态（2026-09-30）。** 对构建机**已解决**——下方*工具验证审计*在安装了 node、cargo 和 MSVC 的环境下运行了完整的 `tauri dev` 构建。

**探测（2026-09-29）。** `node`、`npm`、`npx`、`cargo`、`rustc`、`rustup`、`cl`、`link`、`msbuild`、`winget`、`choco`、`scoop` **全部缺失**。存在：`git`（`C:\Program Files\Git\cmd\git.exe`）和 `python`（3.12）。没有 `~/.cargo`、没有 `~/.rustup`、没有 `C:\Program Files\nodejs`，也没有 Visual Studio 安装——因此**这台机器上前端和 Tauri 后端都无法构建**，也没有可用于安装它们的包管理器。

`node` / `npm` / `npx` **不在 PATH 中**，仓库也没有 `node_modules`，因此无法从 agent shell 运行 `npm run typecheck`、`eslint` 和 `vitest`。对 TS/TSX 的修改目前只能通过阅读 diff 来验证。建议 (a) 提交一份 vendored 工具链，或 (b) 记录确切命令并要求用户运行，使“已验证”的声明有真实输出支撑。

---

## 7. 中止的轮次（600 秒）使下一轮丢失计划/todo 连续性

**状态（2026-11-02）。** **仍部分未解决。** 计划存储仍由 `src-tauri/src/state.rs` 中的 `new_plan_store()` 创建并保存在内存中；重启后未完成的 todo 会丢失。

**状态（2026-09-29）。** 宿主侧部分**已修复**——见*修复批次 1*。存储仍在内存中；跨重启持久化尚未处理。

**现象。** 某轮因 600 秒超时结束，而 `plan_todo` 条目仍为 `pending` / `in_progress`。UI 显示 `plan_todo` 工具提示（*“请继续执行或将其标记为 cancelled … 只更新计划板，本身不算实际进展”*），但下一条用户消息被当作从未收到该提示来处理——agent 开始新请求，把旧 todo 晾在一边，而不是做出决定（继续 / 取消 / 澄清）。

**既有设计（已存在）。**

- `src-tauri/src/commands/system_prompt.rs:194-211` — `active_todo_context()` 渲染保留的 todo，并指示 agent 判断最新用户消息与其关系（保留并继续 / 用 `plan_todo merge=true` 取消 / 问一个澄清问题）。
- `src-tauri/src/commands/chat_turn.rs:2021-2031` — 每轮中，当 `chat_mode == "agent"` 且会话有 `pending`/`in_progress` todo 时，把该上下文注入系统提示。

因此“提醒送达 agent”的路径是存在的——但观察到的行为表明它在异常终止后没有生效。

**可能原因（待调查）。**

1. `plan_todo` UI 提示只是**装饰，不是通道**。唯一真正的再介入是上面的系统提示注入。若注入被跳过，就没有任何东西再询问 agent 这些 todo。
2. `plan_store` 在**内存中**（`state.rs:48`，`new_plan_store()`），按 `session_id` 键控。任何重启、会话切换或存储重置都会丢失 todo，于是 `active_todo_context` 无内容可注入（`chat_turn.rs:2022-2025` 返回 `unwrap_or_default()` → 空 → 不注入）。
3. 超时路径（`chat_turn.rs:2235-2253`）设置 `cancel`、返回 `failed`，并且**从不调和 `plan_store`**，也不发出明确的“中止时有 N 个未完成 todo”信号。被中止轮次的未完成 todo 与下一轮被告知的内容不一致。
4. `chat_mode` 门控：注入只在 `chat_mode == "agent"` 时运行。任何模式漂移都会悄悄禁用它。

**期望行为。**

- 当某轮异常终止且有未完成 todo 时，持久化保留的 todo（不只在内存中），并在紧接着的下一轮注入**强制续接指令**——或自动恢复——让 agent 必须明确继续、取消或询问。不能选择沉默。
- `plan_todo` 提醒应作为真正的轮次指令路由，而不是模型永远收不到的被动 UI 提示。
- 被中止的轮次应发出携带未完成 todo 数量的生命周期事件，以便宿主决定是否恢复。

---

## 8. 计划中途中止的轮次让 `plan_todo` 条目永远处于 `in_progress`

**状态（2026-11-02）。** **仍未解决。** `chat_turn/post_turn.rs` 把 `pending`/`in_progress` 条目计为 `open_todos`，CodeZ 面板会自动续跑，但没有代码在异常结束时把它们改为 `cancelled`/`pending`。

**状态（2026-09-29）。** **未解决。** *修复批次 1* 暴露了未完成 todo 数量，但有意保持条目打开，以便下一轮的调和能够触发；自动标记尚未实现。

这是 #7 的直接后果，单独记录是因为它在 Todo 面板中也可见：来自中止轮次的 `in_progress` 条目永远不会被改为 `cancelled`/`pending`。再加上 Todo 面板在为空时隐藏、默认折叠（`src/components/TaskPanel.tsx`），用户很容易错过仍被标为活动的陈旧条目。建议在异常结束时把未完成条目标为 `cancelled`（或 `pending`），并在面板徽标上显示“N 个未完成 todo”。

---

## 修复批次 1（2026-09-29）— 宿主侧轮次健壮性

范围：不触碰外部 `piscis-engine` 内核、**在本仓库内**可修复的部分。文件：`src-tauri/src/commands/chat_turn.rs`、`src-tauri/src/commands/system_prompt.rs`。

**修改内容。**

1. **轮次超时不再中止调用。** `run_agentz_turn` 现在记录 `turn_timeout_secs`，到期时返回带部分 `response_text` 的 `Ok(HeadlessCliResponse { ok: false, .. })`，而不是 `Err("timed out after 600s")`。这消除了 `chat.rs:198` 的 `agent turn failed: …` 硬错误，并保留会话/已流式输出的文本。
2. **生命周期如实报告。** 整轮超时过去被报告为 `cancelled`（因为超时路径设置了取消标志）。现在它是独立的 `"timed_out"` 状态，`agent_lifecycle` 事件携带 `open_todos`——该会话仍持有的 `pending`/`in_progress` 计划条目数。
3. **平和、可操作的提示。** 超时的 `agent_final` 错误现在是 *“This turn hit the Ns limit and stopped. Partial progress and open todos were kept — send a message to continue.”*，而不是光秃秃的 `timed out after Ns`。
4. **保留 todo 的指令加强。** `active_todo_context` 现在是一个**强制性**的“先调和再行动”块，说明上一轮被中断/超时，且任何未完成条目都不得被悄悄丢弃。

**仍未解决 / 内核侧（未改动）。**

- 工具调用超时 → 返回给 agent（问题 #3）。
- UAC / 提权提示无人响应 → 返回给 agent，而不是失败（问题 #4）。
- LLM HTTP 超时在失败前重试 3 次（问题 #2）。
- 按步恢复：最深层的修复——慢步骤不应耗尽整轮预算——需要 `piscis-kernel` 中的 agent 循环。
- 保留 todo 的跨重启持久化（存储在内存中）。

**验证。** 这些修改**未经编译**：本机 PATH 上没有 `cargo`/`rustc`，外部 git 依赖也未在本地 vendored。它们遵循文件内已有模式，但必须先运行 `cargo check` 才能信任（工作区禁止警告，因此一个多余的警告就是硬错误）。

---

## 修复批次 2（2026-09-29）— 聊天历史中最新消息被隐藏

**现象。** 在有上限/懒加载的消息历史下，最新消息丢失 / 不可见。

**根因。** `src/workspaces/codez/AssistantMessageList.tsx` 及其 CSS 中的两个独立问题：

1. **`content-visibility: auto` 破坏了底部锚定。** `.agentz-msg-virtual`（`AssistantPanel.css`）使用 `contain-intrinsic-size: auto 160px`。屏幕外的行按 160px 估算布局，因此较长的 markdown 回复被严重低估。`scrollTo({ top: scrollHeight })` 因而落点*偏短*，真实高度生效后真正的底部下移——把最新回复推到可视区外。重复的流式增量不断重新触发这一问题。
2. **窗口是可被重新锚定的绝对起始索引。** 裁剪 effect（`Math.max(0, len - 50)` / `Math.min(prev, next)`）可能以丢弃用户刚加载的行的方式调整 `visibleFrom`，而滚动补偿使用的是 `scrollHeight - prevTop`，而不是 `scrollTop + Δ`（只有 `scrollTop ≈ 0` 时才正确）。

**修复。**

- **按数量的底部锚定窗口。** `visibleCount`（默认 50）取代绝对的 `visibleFrom`；渲染的行是 `messages.slice(len - visibleCount)`。最新消息在结构上始终挂载，窗口只在向上滚动 / “加载更早”时向后增长。DOM 保持有界。
- **从消息行移除 `content-visibility: auto`**（挂载窗口才是真正的内存保护）。该 CSS 规则被替换为一条解释原因的 NOTE。
- **加载更早消息时的正确滚动锚定**：`scrollTop = prevScrollTop + (newScrollHeight - prevScrollHeight)`。
- **布局后贴底**（`AssistantPanel.tsx`）：自动滚动现在在 `requestAnimationFrame` 内运行，而不是同步运行，从而测量到稳定后的行高。

**验证。** 仅静态——见问题 #6（没有 `node`/`npm`，因此无法 `typecheck`）。通过阅读 diff 审查；`visibleFrom` 和 `agentz-msg-virtual` 在 `src/` 中已无引用。

---

## 修复批次 3（2026-09-29）— 超时后免点击自动恢复

**目标。** 整轮超时后，无需用户点击即可继续未完成的工作。

**设计。** 后端只负责*发信号*；由前端驱动续接。这使 Rust 侧改动仅为两个 JSON 字段（无签名变更——这很重要，因为这里什么都无法编译），并复用既有的队列/排空路径。

**后端**（`src-tauri/src/commands/chat_turn.rs`）
- 失败的 `agent_final` 负载现在在已有的 `ok` / `error` 之外携带 `timed_out: bool` 和 `open_todos: usize`。
- 超时提示为 *“Turn exceeded its Ns limit — continuing automatically with the unfinished task.”*，并在下一轮的 `setError(null)` 时清除。

**前端**（`src/workspaces/codez/AssistantPanel.tsx`）
- 带 `timed_out && open_todos > 0` 的 `agent_final` 会排入一个续接轮次（上限 `MAX_AUTO_RESUMES = 2`；截至 1.0.40，该逻辑位于 `src/workspaces/codez/useAssistantSession.ts`，`MAX_AUTO_RESUMES = 6`，且在任何 `timed_out` 最终事件上都会恢复）。由于该事件可能在 `busy=false` 提交前到达，它被推入既有的 `queueRef`，由 `finally` 块排空——没有新的调度路径。
- `QueuedTurn.displayText` 让气泡显示 *“（自动继续：上一轮超时）”*，而模型仍收到完整指令（`chat.autoContinuePrompt`）。所有 `setQueuedView` 调用点现在都使用 `queuedLabels()` 辅助函数。
- 计数器在每次用户发起的提交时重置，因此预算按用户请求计算。
- **停止仍然优先。** 用户取消会在不带 `timed_out` 的情况下结束该轮，因此自动恢复不会与明确的取消相冲突。
- 新 i18n 键：`chat.autoContinuePrompt`、`chat.autoContinueLabel`（中 + 英）。

**限制（设计如此）。**
- 自动恢复位于 CodeZ 助手面板中，因此只在该面板挂载时触发（不适用于后台 WorkZ/心跳运行）。
- 最多连续两次自动恢复；第三次超时会停止并报告。

**验证。** 仅静态（问题 #6）。通过 diff 审查；四处 `setQueuedView(queueRef.current.map((q) => q.text))` 均已替换（剩余为零），`autoResumeRef` 已声明、有保护并会重置。

---

## 上游补丁规格 — 内核相关事项（#2、#3、#4）

这些无法在本仓库修复：agent 循环、工具执行和 LLM 调用都在外部 `piscis-kernel` crate 中（`Cargo.toml:23-25`，锁定 `v0.8.62`）。以下是要在上游 `piscis-engine` 中应用的具体改动集。

### A. LLM 请求重试（问题 #2）

- 在内核的 LLM 调用路径中，用有上限的重试包装每个请求：**3 次尝试**，指数退避（例如 1 秒、2 秒）加抖动。
- 仅在以下情况重试：读取超时、连接重置、5xx、429（遵循 `Retry-After`）。**不要**重试 4xx（认证/校验）或 402（余额不足）——它们必须快速失败；UI 已对 402 做特殊处理（`AssistantPanel.tsx` → `chat.llmBalanceError`）。
- 最后一次尝试失败后，返回归因于 **LLM 请求**的错误，由宿主决定（它已在 `chat_turn.rs` 中为超时返回部分结果）。
- 本仓库中已有可参考形态的实现：`src-tauri/src/commands/clawhub.rs:73` `clawhub_get_with_retry`（3 次尝试、退避、`retry-after` 处理）。
- **当下**无需内核改动即可用的宿主侧缓解：调高 `llm_read_timeout_secs` 设置（在 `chat_turn.rs:1966` 等处使用）；宿主拥有客户端，但不拥有重试语义。

### B. 工具超时返回给 agent（问题 #3）

- 在内核的工具分发路径中，工具超时必须解析为普通工具结果，带 `is_error = true` 和机器可读的消息体，例如 `{"error":"timeout","timeout_secs":N}`，并作为该工具的输出回传给模型。
- 它绝不能终止该轮。非零退出码和被截断的输出同样处理。
- UI 已能正确渲染：`src/components/ToolTrace.tsx` 显示 `error` 状态行，消息体在可展开区域中。

### C. UAC / 提权提示无人响应（问题 #4）

- 为确认通道增加**有界等待**。宿主拥有该映射（`src-tauri/src/commands/interactive.rs` → `state.permission_responses`，`state.rs:52`），但 `await` 发生在内核的工具内部。
- 到期时按**未批准**处理，并返回 `is_error = true` 与 `{"error":"elevation_prompt_unanswered"}`，让 agent 可以不提权继续、选择替代方案或明确询问用户。
- 绝不让 UAC 对话框无限期阻塞 agent 线程。
- 必须遵守的宿主侧管道：`chat_turn.rs:2205-2211` 发出 `waiting_permission`；内核必须让该等待超时并继续。

### D. 按步恢复 / 移除整轮预算（问题 #1，最深层）

- 慢步骤应消耗**该步骤**的预算，而不是整轮的。把保护移到单步层面（LLM 请求超时、工具超时），并去掉整个 `agent.run()` 上的单一挂钟上限。
- 在此之前，宿主行为已有改善：上限不再中止调用，UI 会自动恢复（修复批次 1 和 3）。

---

## 修复批次 4（2026-09-30）— 未完成的 `chat_ui` 卡片泄漏到新会话

**现象。** 屏幕上有一个 `chat_ui` 表单（例如 `plan_mode_ui` 的向导 / 任何交互式工具），点击 **＋**（CodeZ *新建聊天*）或 **＋ 新建**（WorkZ *新建任务*）后，同一个表单又出现在全新的、其余为空的会话中。

**根因。** 交互式卡片不存储在消息列表中——它们位于 `src/hooks/useInteractiveCards.ts`（`cards` 状态），由面板组件拥有，因此在会话切换时**得以保留**（面板不会重新挂载）。每条重置路径都清除了消息 / 计划条目 / 产物，却没有清除卡片，唯一会丢弃它们的地方是 `projectDir` 变更 effect。具体来说：

- `src/workspaces/codez/AssistantPanel.tsx` — `newSession`（`:896`）和 `switchSession`（`:913`）调用了 `clearSessionArtifacts()`，但从未调用 `clearCards()`。
- `src/workspaces/workz/index.tsx` — `newTask`（`:761`）和 `openTask`（`:789`）同样在没有 `clearCards()` 的情况下重置视图。

WorkZ 还会把传入事件过滤到绑定的前台会话（`:446`），这就是为什么那里的泄漏只通过新建/打开任务显现，而不是通过后台运行。

**修复。** 在每个放弃当前对话视图的处理函数中，与 `clearSessionArtifacts()` 一起调用 `clearCards()`：

- `AssistantPanel.tsx`：`newSession`、`switchSession`（它也支撑 `fork` / `forkFromCheckpoint`），并把 `clearCards` 加入两者的依赖数组。
- `index.tsx`：`newTask`、`openTask`，并把 `clearCards` 加入两者的依赖数组。

`restoreToCheckpoint` 有意保持不变——它停留在同一会话中，因此仍属于该会话的卡片应保持可应答。

**已知权衡（后续）。** 即使后端轮次仍*阻塞*等待 `chat_ui` 响应（`state.rs` 中的 `interactive_responses`），卡片也会被清除，因此该孤立请求只能以轮次超时结束。更完整的设计是按会话 id 键控每张卡片（事件信封已携带 `session_id`），只渲染活动会话的卡片，或在会话被放弃时显式取消挂起的请求。超出本次修复范围；泄漏本身已修复。

**验证。** `tsc --noEmit -p tsconfig.json` → exit 0；`eslint src/workspaces/codez/AssistantPanel.tsx src/workspaces/workz/index.tsx` → exit 0。手动复现（打开 `chat_ui` 表单 → ＋）仍需在运行中的应用里确认。

---

## 工具验证审计（2026-09-30）— 在重新构建的应用上运行

**构建结果。** 完整的 `tauri dev` 冷构建成功：node v24.19.0、cargo/rustc 1.98.1（msvc）、VS 2022 BuildTools，下载 437 个 crate，Rust 构建 **0 错误 / 0 警告**（这很重要——`Cargo.toml:6` 禁止警告），`tsc --noEmit` 干净。应用启动，实时日志显示 agent 循环运行在新二进制上（`agent loop starting` → `LLM response: ... tool_calls=5` → `executing tool ...`），即本次审计是在修复后的构建*内部*运行的。

### 验证可用

`file_read` / `file_write` / `file_edit`（有 CRLF 注意事项，#5）、`file_search`（glob + grep）、`code_run`、`shell`、`process_control`、`recall`、`chat_ui`、`web_fetch`/`web_search`。`call_fish` 返回 3 个子 agent（scout / summarizer / extractor）。`delegate` 可用。IDE 终端关闭时，`terminal_read` 正确报告 “no terminal sessions are running”。

### 新确认的缺陷

**9. LSP 桥接不可达。** 对 Rust 文件执行 `lsp diagnostics` 返回 `Failed to connect to LSP bridge on port 60770: IO error 10061 (connection refused)`。桥接进程没有在监听；工具泄漏原始 OS 错误，且没有重试或重启。

**10. `lsp` 宣称支持它无法服务的语言。** 同一工具报告 `Supported languages: rust, typescript, python, c/c++`，但对 `.ts` 和 `.tsx` 执行 `lsp diagnostics` 返回 `No LSP server available for file`。`.tsx` 是 `src/` 下的主要文件类型。该列表由 `LspManager::supported_languages()`（外部 `piscis-ide-tools`）生成，在 `src-tauri/src/commands/ide.rs:1536` 使用——因此宣称的集合与实际集合不一致。

**11. `lsp hover` 从未完成初始化。** 返回 `LSP init: no response received`——初始化握手超时。

**12. `read_lints` 对 TS/TSX 无效。** 返回 `no LSP server available for this file type`。它从外部 crate 重新导出（`src-tauri/src/tools/mod.rs:18`），但 agent 系统提示却*指示*在编辑后使用它（`system_prompt.rs:44`、`:103`）。指引承诺了工具无法提供的东西。

**13. `codebase_search` 把文档排在代码之前。** 查询 *“agent turn timeout handling and auto resume”* 返回的**前 5 个结果全是 `docs/known-issues.md`**，排在 `chat_turn.rs` 中的实现之前。嵌入索引包含文档且重度加权散文，并且不提供类型/路径过滤。

**14. `graph_explore` 对 Rust 没有影响半径。** 对 `chat_turn.rs`（被 `chat.rs` 和 `heartbeat.rs` 导入）它报告 _No indexed importers in 1-hop (Phase 0: import edges only)_。影响分析——该工具的核心功能——没有覆盖 Rust 后端，而那里恰恰是风险最高的编辑发生之处。与图谱警告 “many isolated modules (no cross-deps)” 一致。

**15. 实时观察到的内核运行时警告**（上游问题，但真实存在）：
- `sanitize_tool_use_result_pairing: stripping unsatisfied ToolUse ids [...] (satisfied={})`——**每次请求重复约 4 次，且 `satisfied={}`（从未匹配到任何东西）**。工具调用/工具结果配对实际上是坏的，因此清理器在每次请求时都从历史中剥离工具调用。
- `Checkpoint stale for session ... (base hash/count mismatch); ignoring`——检查点悄无声息地恢复失败。

**17. 两个互不相连的 LSP 桥接；前端那个是死代码。** 前端维护一个 WebSocket LSP 会话池（`src/services/tauri/lspSession.ts`），其单元测试断言 `ws://127.0.0.1:9999`（全部 38 个 `vitest` 测试通过）。而 agent 的 `lsp` 工具拨号的是端口 **60770** 上的 Rust 桥接，并被拒绝连接。所以有两条 LSP 路径——一个指向背后空无一物端口的 TS WebSocket 池，以及一个从未启动的 Rust 桥接——两者都不为 agent 服务。编辑器可能驱动自己的路径；面向 agent 的那条不可用。

**16. `delegate` 子 agent 受限于工作区。** 调查 LSP 桥接需要读取 `~/.cargo/git/checkouts` 中的 `piscis-ide-tools` crate，而子 agent 无法访问。对于引擎是 git 依赖的仓库，这是一个结构性盲区：最重要的代码对委派研究不可见。

### Agent 为何悄悄回避这些工具（根因）

系统提示指示每次实质性编辑后使用 `read_lints`（`system_prompt.rs:44,103`），并推荐 `graph_explore`/`codebase_search` 用于理解结构。但是：

- `read_lints` 对 TS/TSX **没有任何有用输出**（缺陷 12）。
- `lsp` 对 Rust 以原始 OS 错误失败（缺陷 9），对 TS 自相矛盾（缺陷 10）。
- `graph_explore` 对 Rust 的影响半径为空（缺陷 14）。
- `codebase_search` 返回文档而不是代码（缺陷 13）。

一个 agent 尝试这些工具一次、得到错误或空结果后，就学会了绕开它们——这正是观察到的行为（退回到 `grep` / `file_read` / `file_edit`）。**这种回避是工具损坏或信号太弱的症状，而不是抽象意义上工具设计不佳。** 修好这些工具（见下）才能让 agent 真正使用它们。

### 编程支持改进待办

**P1 — 让 IDE 工具值得信赖**
1. LSP 桥接生命周期：按需启动、健康检查、重试/重启；把原始的 `os error 10061` 替换为可操作的消息。
2. 接入真正的 TypeScript 服务器，并把 `.ts/.tsx/.js/.jsx/.mjs/.cjs` 映射到 `typescript` / `typescriptreact`。在此之前，把 `typescript` 从 `supported_languages()` 中移除，使宣称的能力与现实一致。
3. 为 `read_lints` 提供回退：没有 LSP 服务器时，运行项目自己的检查器（`tsc --noEmit`、`eslint`）并返回其诊断。它是被指示最多的工具，目前对 TS 毫无产出。
4. 修复 `lsp` 初始化握手（缺陷 11）。

**P2 — 让搜索和影响分析有用**
5. `codebase_search`：对代码查询排除/降权 `docs/**`；增加 `kind` / `path_filter` 参数。
6. 索引 Rust `use` 边，使 `graph_explore` 影响半径对后端可用，并解决 “isolated modules” 警告。
7. 以 TS 已有的同样形态为 Rust 暴露“谁导入了这个文件”。

**P3 — Agent 循环正确性（内核）**
8. 修复 `sanitize_tool_use_result_pairing`：`satisfied={}` 意味着配对映射从未被填充；剥离每个 `tool_use` 会破坏上下文。
9. 修复陈旧检查点检测，使恢复不会悄悄变成空操作。

**P4 — 易用性**
10. 当任务结果量大时提示 `delegate` / `call_fish`（目前没有任何东西建议使用它们）。
11. 允许 `delegate` 只读访问依赖源码（`~/.cargo/git/checkouts`），以便进行上游调查。
12. 允许 `lsp diagnostics` 在没有 `line` / `character` 的情况下运行（它们对该操作没有意义）。

---

## 问题 18–21（2026-09-30）— 流式 UI 与 DeepSeek 协议

### 18. 每个流式 token 被渲染两次（已修复）

**现象。** 回复流式输出时，每个 token 出现两次（`消息消息输出输出时时…`）。该轮结束、会话从数据库重新加载后，文本是正确的。

**根因。** 数据库副本是对的，所以后端对每个 `text_delta` 只发出一次（`chat_turn.rs` 中唯一的 `emit_session` 位置）。重复来自前端**泄漏的重复事件监听器**：`onChatEvent(...)` 返回 `Promise<UnlistenFn>`，而 effect 写的是 `onChatEvent(cb).then(fn => { unlisten = fn }); return () => unlisten?.()`。如果 effect 在 promise 解析*之前*清理（开发环境下 React StrictMode 双重挂载，或因 `handleAgentEvent` 变化导致 `applyEvent` 身份变化），`unlisten` 仍为 `undefined`，清理成了空操作，而晚解析的订阅永远不会被移除。两个活跃监听器 => 每个增量被追加两次。

**修复。** 在每个 `onChatEvent` 订阅中加入 `disposed` 标志：若清理先运行，则新解析出的 `fn` 会被立即调用。文件：`codez/AssistantPanel.tsx`、`workz/index.tsx`、`workz/WorkflowRunPanel.tsx`、`workz/CollabBoard.tsx`。

### 19. 较大的 UI 字号缩放在右侧/底部留下空白条（已修复）

**根因。** `.agentz-app` 使用 CSS `zoom: var(--ui-font-scale)`，然后设置 `width/height: calc(100% / scale)`。在标准化 zoom（当前 WebView2/Chromium）下，百分比*不会*乘以缩放系数，因此 `100% / 1.25 = 80%` 的父元素尺寸在缩放后仍是 80% 宽/高——外壳缩小并露出背景。（较早的 `100vw/scale` 声明反正也被后面的百分比声明覆盖了。）

**修复。** 在 `src/index.css` 中使用 `width: 100%; height: 100%;`。编辑器/终端/浏览器包装层上的反向缩放保持不变。

**手动验证**：在 112.5% / 125% 以及窗口缩放后。

### 20. 工具调用应与文本交错显示（已修复）

**之前。** 整个回复上方只渲染一个 `ToolTrace` 块。

**现在。** 每个工具调用记录 `textOffset`（`tool_start` 到达时已流式输出的助手文本字符数）。`interleaveTools(text, tools)`（`components/ToolTrace.tsx`）在这些偏移处切分回复，使消息读起来是 `文本 → 工具行（灰色结果预览）→ 文本 → …`，运行中的工具（转圈）位于末尾。折叠时，行现在显示一行灰色结果预览；点击展开完整输入/输出。应用于 CodeZ（`AssistantMessageList.tsx`、`AssistantPanel.tsx` 经 `streamLenRef`）和 WorkZ（`agentTools.ts` `applyToolStart(…, last.text.length)`、`workz/index.tsx`）。

**限制。** 工具步骤只在当前轮次的 React 状态中存在；从数据库重新加载后，历史只显示文本（与之前相同）。持久化每条消息的工具轨迹是后续工作。

### 21. DeepSeek：`tool_call_ids did not have response messages`（已解决——内核）

**状态（2026-11-02）。** **已解决。** 修复批次 7 的改动已在 vendored 内核中：`piscis-kernel/src/llm/openai.rs` 中的 `convert_messages` 使用 `pending_tool_ids` 集合。

**错误。** `An assistant message with 'tool_calls' must be followed by tool messages responding to each 'tool_call_id'. The following tool_call_ids did not have response messages: call_00_…`（HTTP 400，`invalid_request_error`）。

DeepSeek 使用 `piscis-kernel` 的 OpenAI 兼容客户端（`DeepSeekClient` 包装 `OpenAiClient`），因此这是引擎缺陷。阅读 `OpenAiClient::convert_messages`：

- “防御：跳过孤立的 tool_result” 分支只在 `result.last()` 是*带 `tool_calls`* 的助手消息时才接受工具结果。当一个助手轮次发出 **N 个并行工具调用**，而结果以多条历史消息到达（或一条消息中有多个 `ToolResult` 块，被发出为多条 `role:"tool"` 条目）时，第 2..N 个结果看到 `last = tool`，于是**被当作“孤立”丢弃**。助手消息保留全部 N 个 `tool_calls` => DeepSeek 拒绝。这与 #15 中的实时警告吻合（`stripping unsatisfied ToolUse ids … satisfied={}`）。
- 预处理也只把*紧随其后*的 `Blocks` 消息视为已满足；任何插入的消息（例如注入的 system/user 文本、视觉刷新）都会使其变为“未满足”。

**上游补丁规格（piscis-engine `openai.rs`）。**
1. 用 `pending_tool_ids` 集合取代 `result.last()` 启发式：发出助手 `tool_calls` 消息时填充，发出 `tool` 消息时消耗；当且仅当工具结果的 id 处于挂起状态时才接受。
2. 转换后，对每条发出的助手 `tool_calls` 消息，验证所有 id 都有 `tool` 回复；若没有，要么合成 `{"role":"tool","content":"[tool result missing: interrupted]"}` 回复，要么丢弃该助手的 `tool_calls`（保留其文本）。绝不发送未配对的调用。
3. 让工具回复紧接在助手消息之后连续出现；把视觉/用户消息移到最后一条回复之后。
4. 在轮次取消/超时时也合成缺失的回复（本仓库新的 `timed_out` 路径会在历史中留下半完成的调用）。

### 22. DeepSeek / “think” 兼容性审计（部分解决——内核）

**状态（2026-11-02）。** 模型匹配已在 vendored 内核中修复（`is_deepseek_thinking_model` 转小写后匹配 `deepseek` + `v4` / `reasoner` / `think` / `-r1` / `deepseek-flash`；要求重放 `reasoning_content` 的提供方会关闭 thinking 并重试一次）。在 UI 中显示推理过程未重新核实。

- `is_deepseek_thinking_model()` 只匹配 `deepseek-v4*` 和 `deepseek-reasoner`。通过其他名称/网关访问的支持思考的 DeepSeek 模型（`deepseek-v3.x` 思考变体、OpenRouter/SiliconFlow `deepseek-ai/DeepSeek-…`、`*-think`）**未被覆盖**；它们的 `reasoning_content` 不会被重放 => 多轮工具使用时 400。建议：对 `deepseek` 做不区分大小写的匹配，加上按模型的 `supports_reasoning` 设置；除非持久化推理轨迹，否则对 DeepSeek 始终发送 `thinking: {type: disabled}`。
- 推理轨迹既不持久化也不显示。流式的 `reasoning_content` 增量必须被忽略（绝不追加到可见文本），某些网关返回的内联 `<think>…</think>` 块应被剥离，或路由到可折叠的“思考”块而不是回复。
- DashScope Qwen（已处理 `enable_thinking:false`）和其他提供方属于同一类问题；目前完全没有显示思考的 UI（`chat.thinking` 只是占位符）。
- 宿主侧缓解：在本仓库中不可能（客户端在内核中）。在补丁落地前，用户可以选择 `deepseek-chat`（非思考）。

---

## 修复批次 5（2026-09-30）— CRLF `file_edit`（#5）与恢复顺序 / 懒加载历史

### #5 CRLF 多行 `file_edit` — 已在上游修复（本地提交）

`file_edit` 位于 `piscis-engine`（`piscis-kernel/src/tools/file_write.rs`），因此在 `C:\Projects\piscis-engine` 的本地克隆中修复并提交（`19357cd`，“fix(file_edit): tolerate CRLF/LF mismatch…”）。如果 `old_string` 无法逐字匹配，会把它的换行转换为文件的风格（CRLF 或 LF）后重试，`new_string` 也做同样转换，使文件保持一致的 EOL。精确匹配不受影响。单元测试（`eol_tests`）通过。**未推送 / 本仓库尚未使用：** 本仓库仍锁定 `v0.8.62`；要使用它，需推送并打标签引擎，然后提升根 `Cargo.toml` 中的 `rev`，或启用被注释的 `[patch]` 路径块。

### 恢复顺序 / 懒加载历史 — 已修复

- 新命令 `chat_get_messages_page(session_id, project_dir, limit, offset)`（`session.rs`，在 `lib.rs` 中注册）通过 `Database::get_messages_older` **从新到旧**读取（按 rowid 排序，不受时钟偏差影响），并以从旧到新的顺序返回每一页，附带 `next_offset` / `has_more`。页面在其较旧一端被裁剪，使其从一条用户消息开始，从而一个助手轮次永远不会被拆分到两页。
- `AssistantPanel.tsx`：恢复会话时只加载最新一页（`HISTORY_PAGE = 120` 条原始行），设置贴底并滚动到最新消息。轮次结束后的 `syncMessagesFromDb` 至少重新读取已加载的内容，因此更早的页面得以保留。
- `AssistantMessageList.tsx`：滚动到顶部（或“加载更早”）时先显示已加载的行，再获取下一页更早的内容并前置插入，同时保持滚动锚定。
- `chat_get_messages`（完整历史）不变，仍被 WorkZ / PoolActivityFeed 使用——它们尚未分页。

---

## 修复批次 6（2026-10-01）— 出错轮次后工作丢失；卡住的 toast

**现象。** 一个红色错误中断某轮后，自上一次完成的轮次以来产生的工具调用和 LLM 输出，在用户发送下一条消息时消失了。

**根因。**
1. 内核在每个 LLM 响应*完成*时才持久化它；出错时正在流式输出的回复从未被存储（在轮次后的数据库同步中丢失）。
2. 工具调用/结果只是 UI 状态：历史 DTO 丢弃了工具行（`messages_for_ui`），因此任何数据库重新同步——下一轮总会执行——都会抹掉所有工具行。
3. 如果新会话的*第一*轮出错，`chatSend` 会在返回会话 id 之前 reject，于是下一条消息打开了一个**新**会话，旧的工作看起来就没了。

**修复。**
- `chat_turn.rs`：收集器跟踪自上一个工具/响应边界以来流式输出的文本；在错误 / 超时 / 用户停止时，将其持久化为助手消息（若内核已存储则跳过）。
- `session.rs`：`messages_rich` 根据 `tool_calls_json` / `tool_results_json` 重建**带工具调用 + 结果**的气泡（`MessageDto.tools`，结果上限 20k 字符）。`chat_get_messages` 和分页命令都使用它。没有已存储结果的调用（被中断）显示为错误行。
- 前端：`messageFromDto` 映射 `tools`；`AssistantMessageList` 按 `text_offset` 交错渲染已持久化的工具（实时步骤仍驱动进行中的轮次）。当 `chatSend` 在返回会话 id 前失败时，`AssistantPanel` 从事件流中采用该会话 id。
- WorkZ toast 现在使用单个计时器 effect（陈旧的超时再也不能关闭或卡住较新的 toast）。应用级退出/设置 toast 已在更早时修复（批次 5+）。

**手动验证。** 在轮次中途强制出错（例如第 2 步使用错误的密钥），再发送一条消息：之前的工具行和流式文本必须保留。`cargo check`（0 警告）和 `tsc --noEmit` 通过；未在运行中的应用里实际演练。

---

## 修复批次 7（2026-10-01）— DeepSeek 工具调用配对（#21/#22），本地引擎提交

已在本地 `piscis-engine` 克隆中修复（`C:\Projects\piscis-engine`，提交 `af31d8d`）；尚未推送，本仓库也尚未使用：

- `openai.rs` `convert_messages`：孤立检查现在使用挂起的 `tool_call_id` 集合，而不是“上一条消息有 tool_calls”，因此并行批次的第 2..N 个结果不再被丢弃。视觉刷新不再拆分一批工具回复。已添加回归测试（18 个 openai 测试通过）。
- `is_deepseek_thinking_model` 也匹配带网关前缀 / `think` / `r1` 的 DeepSeek 名称。
- 使用方法：推送并打标签 `piscis-engine`，提升根 `Cargo.toml` 中的 `rev`（或启用被注释的 `[patch]` 块）。包含更早的 CRLF `file_edit` 提交（`19357cd`）。

---

## 修复批次 8（2026-10-01）— 代码智能

- #9–#14、#16：已修复（真正的 LSP 客户端、`read_lints` 检查器回退、搜索排序/过滤、图谱工具 “index building” 消息、提示工作流、委派子 agent 的依赖读取权限）。
- #17：LSP 会话管理器会淘汰失效会话；Monaco WebSocket 桥接不变。
- #15（`sanitize_tool_use_result_pairing` 报告 `satisfied={}` / 陈旧检查点）：**需要复现**——未复现，无代码改动。
- 新增：代码地图面板和 `symbol_search` / `impact` 工具（基于名称的调用解析，因此常见名称可能被高估）。尚未在运行中的桌面应用中验证；由单元测试和一个索引本仓库的测试覆盖。

---

## 修复批次 9（2026-10-01）— WorkZ 项目/会话、浅色主题代码块

- 浅色主题下 Markdown 代码块是黑底黑字（硬编码的深色背景加上仅深色的 highlight.js 配色）：通过主题变量 `--code-bg` / `--code-border` 以及 `components/hljs-light.css` 中的浅色覆盖修复。
- WorkZ 不再要求打开文件夹。普通会话存储在配置目录下的内部 `free-sessions` 项目文件夹中；它们的工作目录是按会话的设置（会话状态帧中的 `cwd`）。
- 限制：普通会话无法使用 worktree 隔离或仓库 Wiki；团队仍绑定到该范围的数据库。未在运行中的桌面应用中验证。
