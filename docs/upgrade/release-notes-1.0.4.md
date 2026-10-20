# AgentZ 1.0.4 发布说明

发布日期：2026-10-21。版本号与 `CHANGELOG.md` 的 1.0.4 条目一致。

AgentZ 1.0.4 全面补齐 VS Code 扩展宿主对 **Python (`ms-python.python`)** 与 **Python Debugger (`ms-python.debugpy`)** 所依赖的 Proposed API 及高阶运行时接口支持，并打通编辑器语言打开自动激活与标题栏运行按钮联动。

## 修复与增强

- **Python / Debugpy Proposed API 完备支持**：实装并纳入 `SUPPORTED_PROPOSALS` 白名单，覆盖 `contribEditorContentMenu`、`quickPickSortByLabel`、`testObserver`、`quickPickItemTooltip`、`terminalDataWriteEvent`、`terminalExecuteCommandEvent`、`terminalShellIntegration`、`codeActionAI`、`notebookReplDocument`、`notebookVariableProvider`、`portsAttributes`、`debugVisualization`、`contribViewsWelcome`。
- **终端 Shell Integration、测试观察器与语言模型工具链 (`vscode.lm`)**：
  - 实装 `window.onDidWriteTerminalData`、`onDidExecuteTerminalCommand`、`onDidStartTerminalShellExecution`、`onDidEndTerminalShellExecution`、`onDidChangeTerminalShellIntegration`。
  - 实装 `tests.testResults`、`tests.onDidChangeTestResults`、`tests.createTestObserver` 及测试覆盖率类型（`TestResultState`、`FileCoverage`、`StatementCoverage`、`BranchCoverage`、`DeclarationCoverage`、`TestCoverageCount`）。
  - 实装 `vscode.lm`（`registerTool`、`invokeTool`、`selectChatModels`）及 `LanguageModelTextPart`、`LanguageModelToolResult`、`LanguageModelChatMessage`、`env.createTelemetryLogger`、`DebugConfigurationProviderTriggerKind`、`PortAttributes`、`DebugVisualization`。
- **语言事件自动激活与标题栏菜单匹配**：在 `CodeZ` 打开或切换文件语言时自动触发 `onLanguage:<langId>` 激活对应插件，并支持 `editorLangId == python`、`resourceLangId == python` 等 `when` 条件表达式。
