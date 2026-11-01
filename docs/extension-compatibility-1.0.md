# AgentZ 1.0 VS Code Extension Host Compatibility Matrix (Top 8 Extensions)

**English** | [简体中文](extension-compatibility-1.0.zh-CN.md)

> Status (2026-11-02): re-checked against AgentZ 1.0.40. `HOST_VSCODE_VERSION` is still `1.96.0` (`extension-host/src/common/protocol.ts`, mirrored in `src/extensions/common/protocol.ts`). The "Top 8" test in `DiagnosticsSection.test.tsx` asserts a static catalog (IDs, `engines.vscode` ranges, tiers) against the host version; it does not install or run the real VSIX packages, so the tiers below are a compatibility claim, not a recorded live run. Two rows were corrected: `terminalShellIntegration` is now accepted as a proposal, and there is no `MainThreadScm.$registerHistoryProvider` method.

- **Version baseline**: `AgentZ 1.0.0` (re-checked on 1.0.40)
- **Implemented protocol version (`HOST_VSCODE_VERSION`)**: `1.96.0`
- **Automated verification**: `src/workspaces/codez/settings/DiagnosticsSection.test.tsx` (matrix assertions); compatibility evaluation itself lives in `extension-host/src/host/extensionService.ts` (`engines.vscode` + `enabledApiProposals` against `SUPPORTED_PROPOSALS`)

---

## 1. MainThread / ExtHost protocol surface implemented by the host

The AgentZ extension host (`extension-host`) implements the following 14 core main-thread interfaces (`MainContext`) over JSON-RPC 2.0:

1. `MainThreadCommands` (command registration and execution)
2. `MainThreadMessageService` (notification popups and option prompts)
3. `MainThreadStatusBar` (status bar items)
4. `MainThreadOutput` (Output channels in the bottom panel)
5. `MainThreadQuickOpen` (QuickPick and InputBox)
6. `MainThreadDocuments` / `MainThreadEditors` (document sync, WorkspaceEdit, cursor navigation)
7. `MainThreadDiagnostics` (aggregated problems list)
8. `MainThreadLanguageFeatures` (completion, hover, definition, references, highlights, symbols, formatting, CodeAction, signature help)
9. `MainThreadWorkspace` (workspace configuration, file read/write and glob search)
10. `MainThreadTreeViews` (custom sidebar/bottom-panel tree views)
11. `MainThreadWebviews` (sandboxed Webview panels with two-way messaging)
12. `MainThreadTerminal` (terminal creation and sending text)
13. `MainThreadScm` (source control group state)
14. `MainThreadTask` / `MainThreadDebug` / `MainThreadTesting` / `MainThreadNotebook` (tasks, DAP debugging, test trees and `.ipynb` serialization)

---

## 2. Compatibility matrix for the Top 8 most-used VS Code extensions

| # | Extension ID | Name | Engine requirement (`engines.vscode`) | 1.0 compatibility tier | Verified core capabilities | Degradations / unsupported proposed APIs |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `esbenp.prettier-vscode` | Prettier - Code formatter | `^1.90.0` | **Full** | Document formatting (`MainThreadLanguageFeatures.$registerFormattingProvider`), reading `.prettierrc`, status bar indicator, Output log | None |
| 2 | `dbaeumer.vscode-eslint` | ESLint | `^1.89.0` | **Full** | Live Diagnostics (`MainThreadDiagnostics`), Quick Fix CodeAction (`$registerCodeActionProvider`), Output channel | None |
| 3 | `yzhang.markdown-all-in-one` | Markdown All in One | `^1.74.0` | **Full** | Shortcut commands (`MainThreadCommands`), TOC generation (`WorkspaceEdit`), outline symbols (`$registerDocumentSymbolProvider`) | None |
| 4 | `redhat.vscode-yaml` | YAML | `^1.82.0` | **Full** | JSON Schema validation, completion (`$registerCompletionSupport`), hover (`$registerHoverProvider`) | None |
| 5 | `bradlc.vscode-tailwindcss` | Tailwind CSS IntelliSense | `^1.86.0` | **Full** | Class-name completion, CSS preview hover, syntax diagnostics (`MainThreadDiagnostics`) | None |
| 6 | `ms-python.python` | Python | `^1.94.0` | **Degraded** | Interpreter selection (`QuickPick`), built-in Pylance/Pyright LSP channel, test discovery (`MainThreadTesting`), debug configuration (`MainThreadDebug`), `.ipynb` notebooks | The `terminalShellIntegration` proposal is accepted (listed in `SUPPORTED_PROPOSALS`), but `Terminal.shellIntegration` is always `undefined` (`extension-host/src/host/extHostWindow.ts`), so the extension falls back to plain `MainThreadTerminal.$sendText` execution |
| 7 | `rust-lang.rust-analyzer` | rust-analyzer | `^1.92.0` | **Degraded** | Go-to-definition, completion, macro expansion and Cargo diagnostics provided by AgentZ's native built-in LSP session (`src/services/tauri/lsp.ts`) together with the extension's commands/TreeView | Proprietary `experimental/externalDocs` and custom inline InlayHints decorations degrade to standard Hover / CodeAction |
| 8 | `eamodio.gitlens` | GitLens — Git supercharged | `^1.93.0` | **Degraded** | SCM tree views (`MainThreadTreeViews`), command-palette history queries, Webview settings page (`MainThreadWebviews`) | The graphical Commit Graph depends on the `scmHistoryProvider` proposal, which is not in `SUPPORTED_PROPOSALS`; the host reports it as an unsupported proposal in the compatibility report. `MainThreadScm` has no history-provider method. Basic Blame and history commands are unaffected |

---

## 3. Diagnostics and observation of unimplemented APIs

- When an extension declares `enabledApiProposals` outside the host's `SUPPORTED_PROPOSALS`, the host reports them in its compatibility verdict; when the host invokes a `MainThread*` method the renderer does not implement, `src/extensions/compatStore.ts` records it in `missingApis` (as `MainThreadX.method`). Both are shown in the compatibility report of the Extensions manager (`src/workspaces/codez/ExtensionsManager.tsx`) instead of crashing the renderer or the extension host.
