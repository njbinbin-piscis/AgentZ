# AZ-082 过滤 LSP 结构化返回中的资源路径

日期：2026-10-06（Asia/Shanghai）  
状态：Windows 自动化验收通过；未执行真实语言服务器、GUI 或发行版验收。

## 变更

- `piscis-ide-tools::LspTool` 在执行结果格式化前，把结构化响应按 `ToolContext.workspace_root` 限定。
- 对 `uri`、`targetUri` 以及 WorkspaceEdit `changes` 映射键，只允许解析成功且 canonical 路径位于当前工作区的 `file:///` URI。未知 scheme、外部文件、失效文件、错误 URI 或路径解析失败均失败关闭。
- 对外部 Location/LocationLink/WorkspaceSymbol/DocumentChange 移除整个所属返回项；rename `changes` 仅留下区内文件。返回文本附带省略的 LSP 资源条目数量，避免路径消失后仍留下无来源资源名称。
- 一次性审批仍绑定原始完整工具输入、任务和环境。宿主审批集成用例验证受保护 preview 的 `toolInput` 与真实 LSP 调用参数完全一致；输出过滤发生在同一批准调用的 LSP 提供结果格式化阶段。
- 保存补丁 `docs/upgrade/patches/AZ-082-v0.8.68.patch`，由固定候选脚本基于 immutable revision 重建并应用；未修改旁边 `C:\Projects\piscis-engine` 工作树。

## 验证

- 固定候选测试 `structured_lsp_locations_are_filtered_to_the_authorized_workspace`：1 passed，覆盖区内/区外/非本地 URI、definition、rename 映射及 workspace symbol；外部 URI 不在渲染文本中，相关结果项被整体移除。
- 固定候选 `piscis-ide-tools` 完整测试：8 passed、0 failed；1 项真实 `rust_analyzer_symbols_and_reuse` live test ignored（依赖安装并启动真实 rust-analyzer）。
- `cargo test --manifest-path src-tauri/Cargo.toml --test approval_execution --locked`：18 passed、0 failed、1 ignored。其 `diagnostic_build_fallback_is_claimed_only_after_explicit_approval` 校验 LSP protected preview 与提交给工具的原始输入相等。
- 宿主 Clippy 在 `-D warnings` 下仅放行 AZ-080 记录的三类历史 lint（`collapsible_if`、`cmp_owned`、`redundant_closure`），通过。
- 固定候选 Clippy 首次遇到上游基线 `piscis-kernel` 的四类既有 lint（`manual_find`、`collapsible_match`、`chunks_exact_to_as_chunks`、`redundant_closure`）；使用这四类显式 allow、其余保持 `-D warnings` 的 `--all-targets` 检查通过。
- `rustfmt --edition 2021 --check`、`git diff --check`、候选重建脚本 `git apply --check`、台账校验通过。

## 固定候选指纹

- immutable revision：`527e55496caf4437e4265e02869f0a8c764549f6`
- AZ-082 补丁 SHA-256：`6667ff8588069bd08802081cab7a54b6510a19d9f22caa5eecf1ccdebe1bdac6`
- 包含 AZ-029/AZ-079/AZ-081/AZ-082 补丁的候选归档 SHA-256：`8850457adaa3d12bf5d20e3066c5496e8183d8ac69b02f8fc68e0fabe6216de4`

## 限制

URI 路径必须在测试时仍存在；不存在、非本地或不在工作区的目标被安全省略。此变更不声称语言服务器运行在操作系统沙箱中，也没有启动真实 LSP、GUI、真实模型或 release 包。过滤规则限制的是 Agent 获得的工具返回路径，不替代 LSP 进程隔离。
