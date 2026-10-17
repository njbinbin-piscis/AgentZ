# AZ-080 递归搜索/列举路径替换复核

日期：2026-10-06（Asia/Shanghai）  
状态：Windows junction 时序验收通过；未声称原生 GUI 或原子句柄遍历完成。

## 变更

- `ApprovalHooks` 对 `file_search` 与 `file_list` 先执行授权根检查和有界链接预检；下游 hooks 返回 Continue 后、Agent 调 provider 前，再对起始路径执行一次授权根复核。
- 后台只读 `BackgroundReadTool` 解析工作区或显式额外授权读取根，执行有界链接预检，并在 provider 调用前再次验证同一选定根。额外授权根仍需绝对路径且匹配预先 canonical 化的根。
- 新增仅宿主单元测试使用的扫描后注入点。在递归链接扫描完成后替换 `src` 目录为指向工作区外 canary 的 junction，复核失败；`file_search` 和 `file_list` 均不进入 provider 路径。
- 新增 Windows approval integration fixture：初始预检通过后，下游 hook 把 workspace `src` 换成外部 junction；`ApprovalHooks::before_tool` 在返回 Continue 前拒绝这次工具调用。测试同时确认原目录与 outside canary 仍在原位。

## 验证

- `cargo test --manifest-path src-tauri/Cargo.toml --lib search_scope_revalidation_rejects_junction_replacement_after_preflight --locked -- --nocapture`：1 passed。此测试将替换精确插入在受限递归扫描完成后、最终路径复核前，覆盖 `file_search` 与 `file_list`。
- `cargo test --manifest-path src-tauri/Cargo.toml --test approval_execution --locked`：18 passed、0 failed、1 ignored。既有嵌套 junction 拒绝、provider 前检查、审批和撤销测试保持通过；忽略项依赖尚未替换的 AZ-079 固定内核版本。
- `rustfmt --edition 2021 --check` 对 `approval.rs` 与 `approval_execution.rs` 通过；相关文件 `git diff --check` 通过。
- 严格 `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --locked -- -D warnings` 未通过：当前工作区报告 4 条既有诊断，分别位于 `commands/symbols.rs`（`collapsible_if`）、`runtime/approval.rs` 既有 `cmp_owned` / `redundant_closure`、`skills/service.rs`（`collapsible_if`），均不在此次新增路径复核行。重跑时仅允许这三类既有 lint，其余保持 `-D warnings`，Clippy 通过。

## 边界

本次验证针对 Windows 本机 junction；未运行原生 GUI、模型、release、Linux target 或其他文件系统测试。路径复核缩小了递归预检到 provider 调用间的替换窗口，但 provider 仍按路径名打开对象，因此不把实现表述为句柄绑定的原子无 TOCTOU 遍历。涉及被复核后立即并发替换的更窄窗口，需要 provider 级 no-follow 句柄遍历单独处理。
