# AgentZ 1.0.2 发布说明

发布日期：2026-10-20。版本号与 `CHANGELOG.md` 的 1.0.2 条目一致。

AgentZ 1.0.2 修复 `rust-analyzer` 与 `rust-analyzer-no-server` 在扩展宿主中激活失败、并被兼容性报告误标为版本不兼容的问题。

## 修复

- 补齐 `vscode-languageclient` 与 `rust-analyzer` 在加载和激活阶段依赖的 `vscode` 类与语言 API，包括 `CodeLens`、`InlayHint`、`CallHierarchyItem`、`window.tabGroups`、语言特性注册、树视图与测试控制器。
- 同名命令改为后注册覆盖，避免同时安装 `rust-analyzer` 与 `rust-analyzer-no-server` 时激活中断。
- 扩展进程的 `console.log` 改写到 stderr，避免污染扩展宿主的 stdout JSON-RPC。
- 扩展面板把激活异常与引擎版本不兼容分开显示。
