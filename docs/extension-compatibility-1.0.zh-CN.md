# AgentZ 1.0 VS Code 扩展宿主兼容性矩阵（Top 8 扩展实测档案）

[English](extension-compatibility-1.0.md) | **简体中文**

> 本文是 [extension-compatibility-1.0.md](extension-compatibility-1.0.md) 的中文翻译。两者不一致时以英文版为准。

> 状态（2026-11-02）：已对照 AgentZ 1.0.40 复核。`HOST_VSCODE_VERSION` 仍为 `1.96.0`（`extension-host/src/common/protocol.ts`，`src/extensions/common/protocol.ts` 同步）。`DiagnosticsSection.test.tsx` 中的 “Top 8” 测试是对静态目录（扩展 ID、`engines.vscode` 范围、兼容级别）与宿主版本的断言，并不安装或运行真实 VSIX，因此下表是兼容性声明，而非实际运行记录。更正两行：`terminalShellIntegration` 提案现已被接受；不存在 `MainThreadScm.$registerHistoryProvider` 方法。

- **版本基线**：`AgentZ 1.0.0`（已在 1.0.40 复核）
- **实现协议版本（`HOST_VSCODE_VERSION`）**：`1.96.0`
- **自动化验证测试**：`src/workspaces/codez/settings/DiagnosticsSection.test.tsx`（矩阵断言）；兼容性判定本身位于 `extension-host/src/host/extensionService.ts`（以 `SUPPORTED_PROPOSALS` 评估 `engines.vscode` 与 `enabledApiProposals`）

---

## 一、 宿主已实现的 MainThread / ExtHost 协议面

AgentZ 扩展宿主（`extension-host`）通过 JSON-RPC 2.0 实现了以下 14 个核心主线程接口（`MainContext`）：

1. `MainThreadCommands`（命令注册与触发）
2. `MainThreadMessageService`（通知弹窗与选项交互）
3. `MainThreadStatusBar`（状态栏条目挂载）
4. `MainThreadOutput`（底部面板 Output 通道）
5. `MainThreadQuickOpen`（QuickPick 与 InputBox）
6. `MainThreadDocuments` / `MainThreadEditors`（文档同步、WorkspaceEdit、光标跳转）
7. `MainThreadDiagnostics`（诊断问题列表聚合）
8. `MainThreadLanguageFeatures`（补全、悬停、定义、引用、高亮、符号、格式化、CodeAction、签名帮助）
9. `MainThreadWorkspace`（工作区配置、文件读写与 glob 搜索）
10. `MainThreadTreeViews`（自定义侧边栏/底栏树视图）
11. `MainThreadWebviews`（隔离沙盒 Webview 面板与双向消息）
12. `MainThreadTerminal`（终端创建与指令发送）
13. `MainThreadScm`（源代码管理分组状态）
14. `MainThreadTask` / `MainThreadDebug` / `MainThreadTesting` / `MainThreadNotebook`（任务、DAP 调试、测试树与 `.ipynb` 序列化）

---

## 二、 Top 8 高频 VS Code 扩展兼容性矩阵

| 序号 | 扩展 ID | 扩展名称 | 引擎要求 (`engines.vscode`) | 1.0 兼容级别 | 已验证核心能力 | 降级项 / 不支持的 Proposed API 说明 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `esbenp.prettier-vscode` | Prettier - Code formatter | `^1.90.0` | **完全支持 (Full)** | 文档格式化（`MainThreadLanguageFeatures.$registerFormattingProvider`）、`.prettierrc` 读取、状态栏指示器、Output 日志 | 无降级 |
| 2 | `dbaeumer.vscode-eslint` | ESLint | `^1.89.0` | **完全支持 (Full)** | 实时 Diagnostics（`MainThreadDiagnostics`）、Quick Fix CodeAction（`$registerCodeActionProvider`）、Output 通道 | 无降级 |
| 3 | `yzhang.markdown-all-in-one` | Markdown All in One | `^1.74.0` | **完全支持 (Full)** | 快捷键命令（`MainThreadCommands`）、目录生成（`WorkspaceEdit`）、大纲符号（`$registerDocumentSymbolProvider`） | 无降级 |
| 4 | `redhat.vscode-yaml` | YAML | `^1.82.0` | **完全支持 (Full)** | JSON Schema 校验、自动补全（`$registerCompletionSupport`）、Hover 提示（`$registerHoverProvider`） | 无降级 |
| 5 | `bradlc.vscode-tailwindcss` | Tailwind CSS IntelliSense | `^1.86.0` | **完全支持 (Full)** | 类名自动补全、CSS 预览 Hover、语法诊断（`MainThreadDiagnostics`） | 无降级 |
| 6 | `ms-python.python` | Python | `^1.94.0` | **降级运行 (Degraded)** | 解释器选择（`QuickPick`）、Pylance/Pyright 内置 LSP 通道、测试发现（`MainThreadTesting`）、调试配置（`MainThreadDebug`）、`.ipynb` Notebook | `terminalShellIntegration` 提案已被接受（列于 `SUPPORTED_PROPOSALS`），但 `Terminal.shellIntegration` 恒为 `undefined`（`extension-host/src/host/extHostWindow.ts`），扩展因此回退至标准 `MainThreadTerminal.$sendText` 执行 |
| 7 | `rust-lang.rust-analyzer` | rust-analyzer | `^1.92.0` | **降级运行 (Degraded)** | 由 AgentZ 原生内置 LSP 会话（`src/services/tauri/lsp.ts`）+ 扩展命令/TreeView 协同提供跳转、补全、宏展开与 Cargo 诊断 | 专有 `experimental/externalDocs` 与内联 InlayHints 自定义装饰器降级为标准 Hover / CodeAction |
| 8 | `eamodio.gitlens` | GitLens — Git supercharged | `^1.93.0` | **降级运行 (Degraded)** | SCM 树视图（`MainThreadTreeViews`）、命令面板历史查询、Webview 设置页（`MainThreadWebviews`） | 图形化 Commit Graph 依赖 `scmHistoryProvider` 提案，该提案不在 `SUPPORTED_PROPOSALS` 中，宿主在兼容性报告中将其列为不支持的提案；`MainThreadScm` 没有 history provider 方法。不影响基础 Blame 与历史命令 |

---

## 三、 诊断与未实现 API 观测机制

- 扩展声明了 `SUPPORTED_PROPOSALS` 之外的 `enabledApiProposals` 时，宿主会在兼容性判定中报告；宿主调用渲染进程未实现的 `MainThread*` 方法时，`src/extensions/compatStore.ts` 将其记录到 `missingApis`（形如 `MainThreadX.method`）。两者都展示在扩展管理器（`src/workspaces/codez/ExtensionsManager.tsx`）的兼容性报告中，绝不导致渲染进程或扩展宿主崩溃。
