# AgentZ 1.0.1 发布说明

发布日期：2026-10-20。版本号与 `CHANGELOG.md` 的 1.0.1 条目一致。

AgentZ 1.0.1 在 1.0.0 正式基线之上修复了状态栏国际化占位符与 VS Code 扩展宿主在 Windows 下的兼容性与交互闭环问题。

## 修复与改进

- **状态栏模型切换提示与 Codicon 渲染：**
  - 补齐 `src/i18n/zh.ts` 与 `src/i18n/en.ts` 中的 `chat.modelTitle` 和 `chat.defaultModel`，消除状态栏模型名悬停显示 `chat.modelTitle` 占位符的问题。
  - 在 `IdeStatusBar.tsx` 与 `QuickInput.tsx` 中支持解析 VS Code `$(codicon)` 图标标记并清理未插值的 `$Version` 占位符。
- **VS Code 扩展系统深度完善：**
  - 修复 Windows 下 `Uri.fsPath` 与 `RelativePattern` 前导斜杠问题（`/c:/...` -> `c:\...`），确保扩展以工作区路径作为 `cwd` 调用 `node`/`npm`/`pnpm` 等子进程时正常执行。
  - 实装 `FileSystemWatcher`（`onDidCreate` / `onDidChange` / `onDidDelete`）与 `workspace.fs.stat` 的 `FileSystemError.FileNotFound` 异常语义，解决扩展在 `activate()` 阶段因监听器缺失提前中断导致状态栏点击无响应及锁文件误判问题。
  - 将 `StatusBarItem` 属性升级为响应式 setter，支持 `Memento`（`globalState` / `workspaceState`）磁盘持久化，并补齐 `vscode.window.terminals`、`onDidCloseTerminal`、`createQuickPick`、`createInputBox`、`vscode.env.openExternal` 等 API。
  - 在 `MainThreadCommands` 中打通 `onCommand:<id>` 按需激活及 `workbench.action.reloadWindow`、`workbench.action.openSettings`、`vscode.open`、`workbench.action.terminal.*` 等内置命令，联动编辑器标签栏运行按钮（`editor/title/run`）与底部可见终端面板，并将运行期激活异常实时回传至扩展兼容状态。
- **工作台与权限交互细节打磨：**
  - 优化 `WorkZ` 产物路径解析与预览、权限审批卡片（`PermissionCard`）及项目侧栏面板交互体验。
