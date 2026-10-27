# Changelog

All notable changes to AgentZ are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.27] - 2026-10-27

### Fixed

- **User messages were rejected with `task queued/running or goal mutation active` while a background-task continuation turn was running.** That turn was invisible to the UI, so the session looked idle. The backend now announces it (`host_turn_started`); the session shows as running, sends queue behind it, and Stop targets it. A send that still hits a busy gate is retried instead of failing, and a continuation that loses the race to a user turn is re-parked rather than dropped.
- The busy error now reports the holder (`task_key`, phase, age) to make future occurrences diagnosable.

## [1.0.26] - 2026-10-26

### Fixed

- **BotZ members were forced read-only and had no pool tools.** Member turns went through the background read-only restriction, which removed `shell`, file writes, `pool_chat` and `pool_org` (so the 1.0.25 wiring fix did not take effect). Members now keep full tooling in their own worktree; only delegation tools (`delegate`, `call_fish`, `task`) stay off.
- **Searches failed as a whole on protected entries.** A `file_search` / `file_list` scope containing `.agentz`, `.codez`, `.piscis`, `.git`, `.env*` or `.secret_key` was rejected with `protected metadata or secrets`. Those entries are now skipped, and the search tool itself never descends into tool-state directories or reads `.env` / `.secret_key` contents.

## [1.0.25] - 2026-10-26

### Fixed

- **BotZ members had no `pool_chat` / `pool_org`.** The member tool registry never bound a pool event sink, so both tools were not registered and members could not post deliverables, hand off with `@!` or call `complete_todo`. Members now get the same wiring as the main agent.
- **1.0.24 could not build.** The release bump also rewrote the `ryu` entry in `Cargo.lock` to a nonexistent 1.0.24; it is restored to 1.0.23.

### Added

- **Per-room concurrency.** Each BotZ room can cap how many members work at once (0 = global default of 3).
- **Member token budgets.** Member turns count toward a bot's daily and new monthly token budget; 80% shows a warning and 100% pauses the bot (no new turns start).
- **Parent/child tasks.** A subtask whose description has a `Parent: <todo id>` line shows a `?` marker on the board; when all subtasks of an open parent finish, its owner is woken once to summarize and complete it.
- **Unread mentions.** Desks show a count of unread messages that `@` mention the member; opening the desk marks them read.
- **Open PR.** The review panel can push a todo's branch and open a pull request through `gh`.

## [1.0.24] - 2026-10-26

### Fixed

- **BotZ: sent messages vanished and tool calls showed only a name.** The room drawer kept the user's message locally until the transcript reloaded, then dropped it; it now stays until the backend returns it. Tool calls (history and live stream) render with the shared tool trace: icon, summary, expandable input/output, edit diffs and sub-agent cards, interleaved with text.

## [1.0.23] - 2026-10-26

### Fixed

- **400 errors from gateways that reject repeated tool calls.** Qwen on DashScope returns "Repetitive tool calls detected" when the same tool call (name and arguments) appears in several consecutive rounds. The request view now collapses such runs to the last round and notes the repeat count in its result; stored history is untouched, so sessions stuck on this error recover on the next send.

### Changed

- **Earlier, stronger loop protection.** The same call several rounds in a row now gets a nudge from the 3rd repeat (polling tools excluded). `code_run` producing an equivalent outcome again and again (digits such as timings and the internal operation id are ignored) warns from the 4th time and is blocked from the 8th, telling the model to report the blocker. A call whose warnings were ignored 3 times is blocked. The detector window is rebuilt from the saved session history, so a new turn no longer starts counting from zero.

## [1.0.22] - 2026-10-26

### Added

- **Show a background task in a terminal tab.** Each background task row has a "Show in terminal" button that opens a read-only "Background output" tab in the bottom panel. It replays the captured output and follows new output until the task ends, with colors, a status line and a truncation notice.

### Fixed

- **Chinese status bar labels.** The 1.0.21 session/project usage labels (and the 1.0.21 release docs) were written with corrupted characters and showed as question marks in the Chinese UI.

## [1.0.21] - 2026-10-26

### Added

- **Per-session and per-project usage in the status bar.** The status bar shows tokens and cost for the current session and for the whole project, e.g. `Session 12.0k tok ? $0.045 | Project 241.0k tok ? $0.900+`. The kernel now records usage per (session, model) in a new `model_usage` table, so the numbers survive restarts and project switches. New `chat_usage_summary` command.
- **Model prices in Settings.** A new "Model prices" section sets input/output price per million tokens and a currency (USD, CNY, EUR, JPY, GBP) for the default model and every provider entry. Cost is each model's usage times its own price, summed; a trailing `+` means some used models have no price. Without any price only tokens are shown. The hardcoded $3/$15 estimate is removed.

### Notes

- Usage made before this version is not broken down by model and is not included in the totals.

## [1.0.20] - 2026-10-26

### Changed

- **File edit diffs are process info.** Only the newest turn of the current run shows edit cards and the per-turn diff; they disappear when the next task is sent and are not shown when a session is restored. Earlier turns' edit tool calls no longer appear as cards or batch rows, and the stored turn diffs are released on send. Per-turn rollback is available for the newest turn only.
- **Diffs take at most five rows.** Expanded edit cards and turn diffs are a five-row scroll area instead of growing to 220–320 px.
- **Readable colors on the edit card.** Added lines are green on green, removed lines red on red, with a colored left bar and bold `+`/`-`, in dark and light themes (the 1.0.18 change had only covered the turn-diff card).

## [1.0.19] - 2026-10-26

### Fixed

- Sandbox loopback tests no longer fail intermittently on Windows. Their local responder now reads the request before closing the socket, so curl is not reset mid-response. This changes tests only; application behavior is the same as 1.0.18.

## [1.0.18] - 2026-10-26

### Changed

- **Readable file edit diffs in chat.** Added lines are green text on a green background and removed lines are red text on a red background, with a colored bar on the left edge and bold `+`/`-` markers, in both dark and light themes.
- **Redirected background output is pointed out.** When a background task has no captured output because its command writes to a file (`> out.log`, `2>> err.txt`, `| Tee-Object x`, …), *View output* names the file and opens it in the editor. The `shell` and `code_run` tool descriptions now suggest leaving background output unredirected, since it is captured automatically; redirection is still allowed.

## [1.0.17] - 2026-10-26

### Added

- **Sub-agent delegation.** The main agent can hand work to sub-agents through the `delegate`, `call_fish` and `task` tools. Settings has a new *Sub-agent delegation* section (Off / Research only / Auto, max concurrency 1–16); registered agents can opt in to being delegated to. Chat shows sub-agent cards with live activity. Write-capable tasks run in isolated worktrees and are reviewed before merging.
- **BotZ workspace (preview).** A new workspace for running routine bots with rooms, avatars and a review panel. See `docs/botz-design-2026-10-25.md`.
- **Rooms own their members.** A new room is created by picking bots directly; a team template only pre-fills the selection. Existing rooms add or remove members from the toolbar, and bot cards have a *Join room* action. Members who still own active todos stay until their work settles. Rooms created from a template take over its members and contract on first open and keep their pool.
- **Run location and remote boundary.** The title bar shows where the current project runs (💻 local, 🔌 SSH / container / WSL) with a capability table on hover. Remote projects support the editor, terminal, LSP, debugging, extensions and single-agent chat; WorkZ team pools and BotZ rooms are local-only for now. Those modes show a banner with *Open a local project* and *Back to CodeZ* for remote projects, and the backend rejects remote directories for pools and rooms.
- **Show hidden files in the explorer.** A toggle in the Explorer header lists dotfiles, `.agentz` and gitignored entries; `.git`, `node_modules`, `target` and similar stay hidden. The choice is remembered.
- **Browser tool for vision models (RobotZ 0.1.14).** Visual actions (navigate, click, type, scroll…) automatically attach a compressed viewport screenshot when the model supports images; identical consecutive frames are skipped. `screenshot` can capture a single element (`ref`/`selector`) or an `x,y,width,height` region, is downscaled to at most 1568 px (2000 px for full page, capped at 6000 CSS px tall), and is saved to `.agentz/screenshots/` by default.

### Changed

- `.agentz` is now always visible in the explorer (only its internal databases are hidden), even when `.gitignore` lists it.
- Vision-model detection is table-driven and provider-independent (GPT-5, Gemini, Grok, GLM-4.5V, Kimi K2.5/K3, Qwen3-VL, …). Extra patterns can be added with the `AGENTZ_VISION_MODELS` environment variable.
- Windows UI-automation `type_text` fails loudly instead of silently injecting nothing when the target cannot accept keyboard input.
- Kernel file tools print paths through one locale-independent helper (`path_text`), so Chinese paths stay intact on non-Chinese Windows locales and `\\?\` prefixes no longer leak into tool output.
- OpenAI-compatible streaming drops tool calls with an empty name, synthesizes missing call ids, and assembles streamed calls in a deterministic order.

## [1.0.16] - 2026-10-25

### Added

- **Session export and import.** The chat header has export (⇪) and import (⇩) buttons. Export writes the whole conversation, including tool calls and results, to `.agentz-sessions/<title>.agentz-session.json` in the project by default, so it can be committed with the project. Import creates a new session on another machine; absolute paths under the original project root are rewritten to the local root.
- **File edit cards in chat.** Agent `file_edit` and `file_write` calls are shown as cards with the file name, `+N −M` line counts and a collapsible colored diff. Clicking the file name opens it in the editor.

### Changed

- The editor tab bar's horizontal scrollbar is shown only while the pointer is over the tabs.

### Fixed

- **Agent turns stopped on tool arguments containing code.** Raw newlines, tabs, control characters and lone backslashes inside JSON strings are now repaired before parsing. If arguments still cannot be parsed, the model is told and asked to retry (twice, then to split large writes) instead of the tool being run with empty arguments.
- **Tool output with ANSI escapes or control characters** is cleaned before it is sent to the model; progress-bar redraws are collapsed to their final state.
- **Output mixing GBK and UTF-8 lines** on Chinese Windows is decoded per line.
- **Python scripts run from `code_run` and the shell print Chinese text**: `PYTHONUTF8=1` and `PYTHONIOENCODING=utf-8` are set unless the caller already set them. The tool descriptions now explain cmd.exe quoting and the Windows PowerShell 5.1 rule for passing quotes to native programs.

## [1.0.15] - 2026-10-25

### Changed

- **Remote workspaces run code features next to the files, like VS Code Remote.** Fast Wiki, DeepWiki, the repository-memory panel, the code map, symbol search and the call graph now accept `agentz-remote://` projects. Indexes are computed on the local source mirror, which is used as a cache, and generated documents (`.agentz/REPO_WIKI.md`, `WIKI_DEEP.md`, `AGENT_CODING_BRIEF.md`, `REPO_MEMORY.md`) are written back to the remote repository. Code-map churn is read with `git log` on the remote.
- **Agent `lsp`, `read_lints`, `code_run` and `file_diff` work in remote workspaces.** Language servers are started on the remote host, lints fall back to `cargo check` / `tsc --noEmit` there, commands run on the remote, and diffs read remote files. Previously these tools were removed from remote sessions.

### Fixed

- **Initialize Git repository** now works for remote workspaces.
- The title-bar chat button tooltip now says it shows or hides the chat panel; it no longer mentions a graph view.
- **Shell output on Windows kept only the last line.** Collapsing repeated lines dropped every line that differed from the next one, so multi-line PowerShell and cmd output looked truncated. Every line is now kept and only consecutive duplicates are folded.
- **Long command output keeps its ending.** The 200 KB capture now holds the first and last halves with a "bytes not captured" marker, so the final lines (where errors usually appear) are visible. A capture cut no longer leaves a broken character.
- **cmd prints Chinese and other non-ANSI text.** Built-ins such as `echo`, `cd` and `dir` ignored `chcp 65001` and wrote `?`. The command now runs in a nested cmd started after the code page switch, and quotes and `&` inside the command are passed through unchanged; previously quotes reached cmd as `\"`. Lines that are not UTF-8 are decoded with the system code page.
- **Grep finds matches in UTF-16 and GBK files.** Files that were not UTF-8 were skipped silently. Grep now decodes them the same way file reads do and strips a leading BOM from matched lines.
- **Git paths with Chinese characters** are shown as names instead of octal escapes (`core.quotepath=false`) in source control, agent task review, approvals and the code map.
- **Web fetch decodes GBK/GB2312 pages** using the charset from the response header or `<meta charset>`.

## [1.0.14] - 2026-10-24

### Added

- **Initialize a Git repository from Source Control.** When the open folder has no repository, the panel offers a button that runs `git init` there. If the folder is already inside a parent repository, initialization is refused and the parent path is shown.

### Fixed

- **Agent turns no longer stop with `run store: database is locked`.** Execution history, budgets, and reservations share `.agentz/runs.db`. Rollback-journal mode let a reader block a writer until the 5 second busy timeout aborted the turn. The database now uses WAL so readers and writers proceed together, and the busy wait is 15 seconds. Existing databases switch the next time they are opened.

## [1.0.13] - 2026-10-24

### Fixed

- **Editor tabs no longer shrink to a few letters.** Tabs size to the full filename instead of sharing a 72–200px slot. When they no longer fit, the tab strip scrolls horizontally with the mouse wheel and a thin scrollbar. The tooltip still shows the full path.

## [1.0.12] - 2026-10-24

### Changed

- **Changes panel is an agent-edit review, separate from git.** It lists journaled `file_write` / `file_edit` changes still awaiting Keep or Undo across the whole session, netted against the pre-edit baseline and reconciled with disk. A file the agent created and later deleted, or edited back to its original bytes, no longer appears. Git status stays in the source-control view.
- **Keep and Undo persist.** Per-file and keep-all / undo-all write a `kept` or `undone` mark in the file journal (older journal databases gain the column on open), so a refresh or restart does not bring accepted edits back.
- **Artifacts only come from write-type tools** (`file_write`, `file_edit`, and similarly named producers). Reads, listings, and searches no longer add a directory or file. Refresh drops paths that are no longer regular files, and deleted change entries cannot be opened.

### Fixed

- Reading a project subdirectory no longer lists that directory as an artifact.
- Temporary test files the agent wrote and then deleted stayed in both Changes and Artifacts, and Refresh did not clear them.
- Clicking a change whose file was already gone opened the editor and reported that the file could not be found.

## [1.0.11] - 2026-10-22

### Added

- **`wait` tool**: actively wait for long-running background tasks — one task by `bg_id`, or this session's tasks with `scope=any|all`, `until=exit|ready`; falls back to a bounded plain delay (`seconds`) when nothing is running. All waiting happens host-side in a single call, complementing `process_wait`.

### Changed

- **Balanced tool-output truncation** (`shell` / `code_run` / `process_control`): per-line cap of 1000 chars with middle elision (head/tail of the line kept), whole-line head/tail selection (55/45) instead of mid-line byte slicing, and — whenever output is truncated — the full output is spilled to `.agentz/logs/tool-output/` with its path named in the result so the agent can page through it with file_read/file_search instead of re-running. Presentation budgets are tightened (shell 24KB/12KB, code_run 16KB/8KB, process_control 24KB/12KB) while code_run's capture cap rises from 8KB to 64KB per stream.

### Fixed

- **Ctrl+S in the editor closed the file tab** (and could claim the file "was deleted on disk"): saves use temp-file + atomic rename, which the Windows watcher reported as a removal. The watcher backend now reclassifies Remove events for paths that still exist as modifications, and the frontend verifies a "deleted" event (debounced read-back) before closing anything; genuine deletions behave as before.
- **Project template dialog**: the radio button sat alone on a centered line; it now shares a left-aligned row with the template name, with the description indented beneath.

## [1.0.10] - 2026-10-22

### Added

- **Background long tasks**: `shell` / `code_run` accept `background=true` (plus `notify_on`, `ready_port`, `ready_log` readiness conditions). Tasks are hosted by a kernel `BackgroundProcessRegistry` (Windows keeps the existing sandbox strength and Job process-tree semantics), stream into a capped ring buffer plus `.agentz/logs/bg/<bg_id>.log`, and are driven with the new `process_wait` (host-side blocking wait for exit/readiness), `process_output` (cursor-based incremental reads) and `process_kill` (confirmed whole-tree termination) tools. When a task finishes or becomes ready, the host automatically resumes the originating session with the result (parked while busy, deduplicated, capped at 6 consecutive auto-resumes per session). CodeZ gains a background-task panel with output/kill/session-jump, session and WorkZ rows show background-running badges, the IDE status bar shows a project background count, and diagnostic exports include a background-task summary.
- **Real parallel sessions in CodeZ**: running state, send queues and stop are now tracked per session (`sessionTurnBookkeeping`), so multiple sessions can run at once without queue or stream cross-talk, and each session row shows a live status LED.

### Changed

- **Foreground `shell` / `code_run` timeout now detaches to the background instead of killing the process**; the tool returns a `bg_id` and the agent decides whether to wait (`process_wait`) or continue other work.
- **`process_control` background starts are now managed**: `start` with `wait=false` registers the process (output capture, session binding, whole-tree kill) instead of a bare untracked spawn.
- Background tasks do not survive app restarts (Job ownership); on startup previously running tasks are marked `interrupted` while parked continuations are re-dispatched once. Per-project background concurrency defaults to 4 (`AGENTZ_BG_MAX_PER_WORKSPACE`), single-task runtime cap defaults to 3600s (`AGENTZ_BG_MAX_RUNTIME_SECS`). On non-Windows platforms foreground commands remain refused without an OS sandbox, while `background=true` runs managed via `/bin/sh -c` (setsid process group).

### Fixed

- WorkZ: viewing a background-running session now clears its busy state and reloads history when that session's turn finishes elsewhere.

## [1.0.9] - 2026-10-21

### Changed

- **Sandbox strength now follows the security level**: `strict` keeps the zero-network AppContainer, `balanced` runs shell/code_run as the current user with network, loopback and toolchain caches while the working directory stays pinned to the project, and `dev` disables the sandbox entirely (working directory may leave the project). The sandbox mode comes only from Settings, never from tool input.
- **Settings reorganized**: a new first tab “General” holds language, the security level, the default WorkZ directory and project list, app change history, config status/path, and diagnostics export. The Models tab now only contains provider, model and MCP configuration.
- **Settings theme styling**: the diagnostics export section now sits inside the padded settings body like every other section, and a zero-specificity base theme layer styles every bare input/textarea/select/button across all settings tabs (rounded corners, theme background and borders in light and dark themes).

## [1.0.8] - 2026-10-21

### Fixed

- **Windows sandbox crash on Nutstore-synced workspaces when running `code_run` / `shell`**: Commands such as `node -v && echo OK` no longer walk the whole workspace (including `node_modules`) to rewrite DACLs, recurse on the default 1 MB thread stack, or dereference unaligned directory records from cloud filter drivers. Developer toolchains run in the pinned Job Object path before any ACL lease, large trees fail closed at 256 entries, and both tools execute on an 8 MB worker thread.
- **Typed terminal commands unreadable in light theme**: The xterm theme now ships a full ANSI palette. Light mode maps PowerShell `brightYellow` input to `#713f12` and `yellow` to `#854d0e`, and the terminal enforces a 4.5 minimum contrast ratio. Light theme `--warning`, `--success`, and `--error` tokens are set for the same backgrounds.

## [1.0.7] - 2026-10-21

### Fixed

- **Windows sandbox (`shell` tool) failing with `NTSTATUS 0xC0000022` on cloud-synced directories (Nutstore / OneDrive / Cloud Files API) and developer toolchain binaries (`node.exe`, `python.exe`, `cargo.exe`)**:
  - Added `FILE_OPEN_FOR_BACKUP_INTENT` (`0x00004000`) to `NtCreateFile` relative handle opens in `proc_windows_sandbox.rs`, and allowed `open_workspace_root_handle` to fall back to `FILE_SHARE_DELETE` when a cloud sync daemon holds a delete-shared handle on the workspace root.
  - Added `FileAttributeTagInfo` inspection (`is_benign_cloud_reparse_tag` / `ensure_not_redirection_reparse_point`) so benign cloud sync reparse tags (`IO_REPARSE_TAG_CLOUD*`, `IO_REPARSE_TAG_PROJFS`, `IO_REPARSE_TAG_DEDUP`, `IO_REPARSE_TAG_WOF`) and cloud minifilters denying `FILE_OPEN_REPARSE_POINT` are permitted while symlink and mount-point redirections continue to fail closed.
  - Added read-only DACL workspace pinning (`verify_and_pin_readonly_workspace`) and developer toolchain ACL detection (`command_requires_unprivileged_toolchain_fallback`) so workspaces where cloud-mapped folders deny `WRITE_DAC` (`0xC0000022`) or commands invoking `PATH` toolchains lacking `ALL APPLICATION PACKAGES` (`S-1-15-2-1`) execute inside a pinned, kill-on-close Windows Job Object after validating all workspace handle-chain, reparse-point, hard-link, and non-null DACL invariants.
- **Popups, context menus, titlebar popovers, and modals clipped or off-screen under UI font scaling (`zoom: var(--ui-font-scale, 1)`)**:
  - Unified viewport-aware positioning via `src/utils/popupPosition.ts` (`computeFixedPopupPosition` and `computeContextMenuPosition`), converting client coordinates into CSS zoom-normalized space and anchoring from `right` / `bottom` whenever a popup or context menu overflows the right or bottom window boundary.
  - Portaled and clamped `TagMultiSelect`, `NotificationInbox`, `ChatComposer` `@`/`/` mention popups, `Titlebar` `Aa` / Workspace / Quick Start popovers, and all right-click context menus (`FileTree`, `EditorTabs`, `Terminal`, `ExtensionsPanel`), and normalized `max-height` / `max-width` across all modal overlays under UI zoom.

## [1.0.6] - 2026-10-21

### Fixed

- **Composer dropdown menus in `WorkZ` and `CodeZ` clipped, rendered off-screen, or obscured by other UI**:
  - Portaled `DropdownSelect` and `DropdownMultiSelect` popups to `document.body` (`z-index: 100100`) so they escape parent `overflow: hidden` / `overflow: clip` containers and ancestor stacking contexts (`.agentz-composer-wrap`, `.agentz-workz-isolate-bar`).
  - Fixed `computeFixedPopupPosition` so upward-opening composer menus (`placement="up"`) anchor directly above the trigger via `bottom`, clamp `maxHeight` to the available viewport space instead of flipping off-screen below the bottom window edge, clamp `left` inside the right window edge for `CodeZ`'s right-hand chat panel, and normalize coordinates when `.agentz-app` uses CSS `zoom`.
  - Rendered skill hints inline (clamped to 2 lines) inside `DropdownMultiSelect` instead of spawning an overlapping toast at `bottom: 120px`.
- **Editor and terminal text unaffected by font size setting**:
  - `CodeEditor`, `DiffEditor`, `NotebookEditor`, `Terminal`, and `WorkZ`'s file preview/editor now scale their font sizes proportionally with `UI_FONT_SCALES` (`88%`–`150%`) and update live when the `Aa` font size setting changes.
  - Added an explicit Editor Font Size stepper (`−` / `px` reset / `+`, `10px`–`32px`) to the titlebar `Aa` popover, enabled `Ctrl + MouseWheel` zoom (`mouseWheelZoom: true`), and bound `Ctrl/Cmd + =`, `Ctrl/Cmd + -`, and `Ctrl/Cmd + 0` inside the editor.

## [1.0.5] - 2026-10-21

### Fixed

- **Editor language server stayed unavailable after opening a file**:
  - The Monaco WebSocket bridge now forwards `initialize` to the real language server and reads stdout with `Content-Length` frames, so rust-analyzer and typescript-language-server complete the handshake.
  - The bridge listener is bound before the port is returned, removing the race where the editor connected before the socket was listening.
  - LSP sessions start only for languages with a backend server (`rust`, `typescript`, `python`, `cpp`). JSON, Markdown, and other syntax-only files no longer show “语言服务不可用”.
  - Python falls back to the bundled Jedi language server inside an installed `ms-python.python` extension when `pyright-langserver` is not on `PATH`.

## [1.0.4] - 2026-10-21

### Fixed

- **Python (`ms-python.python`) & Python Debugger (`ms-python.debugpy`) VS Code Extension Compatibility**:
  - Implemented and whitelisted all 9 proposed APIs declared by `ms-python.python` (`contribEditorContentMenu`, `quickPickSortByLabel`, `testObserver`, `quickPickItemTooltip`, `terminalDataWriteEvent`, `terminalExecuteCommandEvent`, `codeActionAI`, `notebookReplDocument`, `notebookVariableProvider`) as well as `ms-python.debugpy`'s proposed APIs (`portsAttributes`, `debugVisualization`, `contribViewsWelcome`) and `terminalShellIntegration`.
  - Added full runtime support for `window.onDidWriteTerminalData`, `window.onDidExecuteTerminalCommand`, `window.onDidStartTerminalShellExecution`, `window.onDidEndTerminalShellExecution`, `window.onDidChangeTerminalShellIntegration`, `Terminal.shellIntegration`, `QuickPick.sortByLabel` / `buttons` / `onDidTriggerButton` / `onDidTriggerItemButton`, `QuickInputButtons.Back`, `tests.testResults` / `onDidChangeTestResults` / `createTestObserver`, `TestResultState`, `FileCoverage` / `StatementCoverage` / `BranchCoverage` / `DeclarationCoverage` / `TestCoverageCount`, `NotebookController.variableProvider`, `NotebookVariablesRequestKind`, `window.showNotebookDocument`, `workspace.openNotebookDocument`, `CodeAction.isAI`, `DebugConfigurationProviderTriggerKind`, `workspace.registerPortAttributesProvider`, `PortAutoForwardAction`, `PortAttributes`, `debug.registerDebugVisualizationProvider` / `registerDebugVisualizationTreeProvider`, `DebugVisualization`, `env.createTelemetryLogger`, and `vscode.lm` (`registerTool`, `invokeTool`, `LanguageModelTextPart`, `LanguageModelToolResult`, `LanguageModelChatMessage`).
  - Wired automatic `onLanguage:<langId>` extension activation when opening or switching editor models in `CodeZ`, and extended editor title action `when` evaluation (`editorLangId`, `resourceLangId`, `!virtualWorkspace`, `shellExecutionSupported`).

## [1.0.3] - 2026-10-21

### Fixed

- **Project templates (`superpowers` and all bundled templates)**: Fixed `.gitignore` unanchored `.agentz/` patterns that previously excluded template rule files from the repository, and added concrete `.agentz/rules/*.md` workflows for all 9 bundled project templates (`superpowers`, `agent-browser`, `atuin`, `cloudbase`, `hookify`, `plugin-finder`, `ralph-loop`, `security-guidance`, `tsmart`). Updated `project_apply_template` and `project_has_agentz`, added an "Apply template…" button in `Settings -> Project Rules`, removed external `CodeBuddy` wording from the template dialog and metadata, and aligned `using-superpowers/SKILL.md` with AgentZ's `file_read` + `plan_todo` workflow.
- **Coding tool self-healing & reliability (`file_edit`, `file_search`, `shell`, `terminal_read`)**:
  - `file_edit` now embeds numbered closest-matching file context in `MatchNotFound` errors and 1-based match line numbers in `MatchAmbiguous` errors so models can self-heal without an extra `file_read` round-trip.
  - `file_search` now supports `include_ignored: true` to search inside excluded directories (`node_modules`, `target`, etc.) and lists skipped excluded directories when returning 0 matches.
  - Windows `shell` (`powershell` / `powershell32`) automatically rewrites top-level unquoted `&&` command chains into Windows PowerShell 5.1-compatible conditional blocks.
  - `terminal_read` now supports an optional `wait_ms` (`0..=15000`) parameter to wait briefly before snapshotting PTY output.

## [1.0.2] - 2026-10-20

### Fixed

- **rust-analyzer activation**: The extension host now exports the `vscode` classes and language APIs that `rust-analyzer` and `vscode-languageclient` extend or call during activation (`CodeLens`, `InlayHint`, `CallHierarchyItem`, `tabGroups`, language providers, tree views, and test controllers). Duplicate command ids from `rust-analyzer` and `rust-analyzer-no-server` no longer abort activation, and extension `console.log` output stays off the JSON-RPC stdout stream.
- **Extensions panel**: Activation failures are listed separately from engine version mismatches, instead of all being labeled as skipped incompatible versions.

## [1.0.1] - 2026-10-20

### Fixed

- **Status Bar Model Switcher i18n & Codicon Rendering**:
  - Added missing `chat.modelTitle` and `chat.defaultModel` translations in `src/i18n/zh.ts` and `src/i18n/en.ts` so hovering over the status bar model pill shows the localized tooltip instead of raw key placeholders.
  - Added VS Code `$(codicon)` token parsing (`formatCodiconText`) in `IdeStatusBar.tsx` and `QuickInput.tsx` so status bar items and QuickPick entries render clean icons (`⧉`, `▶`, `✓`, `↻`, etc.) instead of raw `$(versions)` or `$Version` text.
- **VS Code Extension Host Compatibility & Interactive UI**:
  - Normalized Windows `Uri.fsPath` and `RelativePattern` in `extension-host/src/host/types-impl.ts` (stripping leading `/` on Windows drive paths) so extensions spawning child processes with `workspaceFolders[0].uri.fsPath` as `cwd` succeed on Windows.
  - Implemented `FileSystemWatcher` (`onDidCreate`, `onDidChange`, `onDidDelete`, `dispose`) and `FileSystemError.FileNotFound` on `workspace.fs.stat` in `extension-host/src/host/extHostWorkspace.ts`, preventing extension `activate()` crashes before command registration and fixing lockfile detection.
  - Made `StatusBarItem` properties reactive after `.show()`, persisted `Memento` (`globalState` / `workspaceState`) to disk, and added `vscode.window.terminals`, `onDidOpenTerminal`, `onDidCloseTerminal`, `createQuickPick`, `createInputBox`, `vscode.env.openExternal`, and `vscode.extensions.getExtension`.
  - Wired built-in workbench commands (`workbench.action.reloadWindow`, `workbench.action.openSettings`, `vscode.open`, `workbench.action.terminal.*`) and on-demand `onCommand:<id>` activation in `MainThreadCommands`, editor title run actions in `EditorTabs.tsx`, and live terminal command execution in `Terminal.tsx`.
  - Propagated runtime extension activation failures back to `compatStore` so the Extensions panel reflects actual activation errors.
- **Workspace & Approval UX Refinements**:
  - Fixed `WorkZ` free-mode (`projectless`) task turns failing with `isolated task has no registered environment; explicit recovery/binding required` by propagating `projectless_app_data` into `DurablePlanTodoTool` and aligning `chat_send`, `chat_turn`, `chat_plan`, and `task_view` binding checks, while syncing `isolate` state with restored session environments and non-Git folders.
  - Fixed `WorkZ` team selector tooltips showing workflow team hints on all options by preventing outer container `title` inheritance onto `.agentz-dropdown-popup` items and attaching per-option mode-specific hints (`workflow` vs `swarm`) in `teamOptions`.
  - Improved `WorkZ` artifact path extraction and file preview resolution, interactive permission card layout, and project edge panel interactions.

## [1.0.0] - 2026-10-19

### Added

- **Cross-View WorkZ ↔ CodeZ Collaboration Loop (`R10-05`–`R10-09`)**:
  - Dirty buffer conflict guard in `CodeZ` (`EditorConflictTracker` + `[data-testid="editor-conflict-banner"]`) with one-click Diff view, Keep Mine, or Accept Disk resolution.
  - One-click worktree handoff (`在 CodeZ 中打开`) from `AgentTaskReview` and `AgentFilePreview`, binding `CodeZ` file tree, editor, terminal, and search to the isolated worktree with a top return pill (`worktree-handoff-bar`).
  - Human edit feedback ledger (`recordUserRecentEdit`, `consumeUserRecentEdits`) that injects `<user_recent_edits>` into the next Agent turn after saving in `CodeZ` or `WorkZ`.
  - Automatic staging/committing of uncommitted worktree changes during `AgentTaskReview` merge (`commit_uncommitted_task_changes`) and non-Git folder task snapshot rollback (`agent_task_rollback_snapshot`).
  - Editable preview canvas in `WorkZ` (`AgentFilePreview.tsx`) with `Preview / Edit / Diff` modes, `Ctrl+S` save, and `[基于最新修改继续对话]`.
- **Vibe Coding & Office Starter Experience (`R10-10`–`R10-14`)**:
  - Global Command Palette (`Ctrl+Shift+P`) and Quick Open (`Ctrl+P`) in `CodeZ` (`CommandPalette.tsx`).
  - Per-turn file modification rollback (`[撤销本轮文件修改]`, `journalUndoTurn`) on assistant bubbles in both `CodeZ` and `WorkZ`.
  - 4 high-frequency starter scenario cards in `WorkzEmptyState.tsx` and streamlined sandbox controls (isolated by default, advanced SHA controls collapsed).
  - Unified side-channel LLM token/cost accounting (`background`, `inline_edit`) and status bar model switcher + Token/USD cost pill (`IdeStatusBar.tsx`).
  - 3-step first-run onboarding wizard (`OnboardingModal.tsx`) and `Beta` gating for Swarm / Workflow / Gateway surfaces.
- **Architecture Modularity, Security & Release Gates (`R10-01`–`R10-04`, `R10-15`–`R10-24`)**:
  - Decomposed monolithic frontend components (`workz/index.tsx`, `codez/index.tsx`, `AssistantPanel.tsx`) and Rust `commands/chat_turn/` (`mod.rs`, `prompt_assembly.rs`, `tool_approval.rs`, `post_turn.rs`).
  - Removed 18 isolated bureaucratic compliance UI files (`R10-03`) in favor of natural main-flow UX (`ADR-006`).
  -Pinned 5-phase kernel contract tests and `BrowserGenerationTracker` (`snapshot_generation` stale-ref guard).
  - Complete theme CSS token convergence, zero hardcoded Chinese in production `.tsx` (`i18n.test.ts`), full-outlet secret scrubbing, historical `v0.9.0`/`v0.9.12` migration fixtures, writer-quiescent encrypted backups, manual chunk splitting (`index-*.js < 1.2 MB`), cross-view E2E suite (`e2e/agentz.spec.tsx`), and Top 8 VS Code extension compatibility archive (`docs/extension-compatibility-1.0.md`).

## [0.9.19] - 2026-10-19

### Added

- The code graph now groups files into logical subsystems, extracts Tree-sitter symbols and call edges, and links frontend `invoke` calls to Rust `#[tauri::command]` handlers. PageRank ranks hub files.
- `graph_explore` returns symbol definitions, call and IPC sites, and matching repository notes. CamelCase symbols are included.
- Source-anchored repository memory (`repo_memory`, `.agentz/REPO_MEMORY.md`) records invariants and pitfalls. Editing a cited line range marks the note stale until it is verified.
- Dual-track wiki: a fast architecture wiki (AST cards, PageRank tour, IPC diagram) and an incremental DeepWiki with per-module content-hash cache. The Wiki menu can open either file or the memory notes.
- Built-in `repo-cartographer` skill. Markdown `path:line` anchors open the cited file. Task review can sort changed files by importer count.

### Known limits

- This release has not passed the upgrade acceptance gates. The unaccepted items and user test cases remain in `docs/upgrade/acceptance-0.9.12.md`.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.18 still apply.

## [0.9.18] - 2026-10-19

### Changed

- WorkZ chat bubbles use the conversation column instead of a fixed pixel cap. Agent replies span 80% of the column (10% gutter on each side) and grow when the window is maximized. User messages stay right-aligned and leave at least 10% empty on the left.
- Mermaid diagrams scale to the bubble width, so a wider window enlarges the figure.

### Known limits

- This release has not passed the upgrade acceptance gates. The unaccepted items and user test cases remain in `docs/upgrade/acceptance-0.9.12.md`.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.17 still apply.

## [0.9.17] - 2026-10-19

### Fixed

- WorkZ session LED no longer keeps blinking after a turn has stopped. Session status is finalized when a turn ends (including early exits), listing heals stale `running` rows that have no live task gate, and the sidebar only pulses for sessions that are actually in `runningIds`.

### Known limits

- This release has not passed the upgrade acceptance gates. The unaccepted items and user test cases remain in `docs/upgrade/acceptance-0.9.12.md`.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.16 still apply.

## [0.9.16] - 2026-10-18

### Fixed

- Mermaid diagrams in chat no longer render as empty boxes. Sanitization kept Mermaid's `<style>` and `<foreignObject>` labels, and parsing switched from strict XML to HTML so `<br>` and CSS `>` selectors no longer blank the SVG.
- Session forks get numbered titles such as `(1) Title (fork @ checkpoint)` and `(2) Title (fork)`. Nested fork suffixes are stripped before numbering, and the next free index is `max + 1` across existing sessions that share the same root title.
- The stop-button ring keeps spinning when the OS reports `prefers-reduced-motion` (for example remote desktop or accessibility settings). Progress spinners are exempt from the one-shot animation rule.
- Remote connect, project template, task review, and skill revision panels follow the light and dark theme tokens instead of hardcoded dark fallbacks from undefined `--agentz-*` variables.

### Known limits

- This release has not passed the upgrade acceptance gates. The unaccepted items and user test cases remain in `docs/upgrade/acceptance-0.9.12.md`.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.15 still apply.

## [0.9.15] - 2026-10-18

### Added

- Shared recent-projects list across CodeZ and WorkZ. Clicking the titlebar folder button or project path opens a menu with "Choose Folder…" and the recent projects from `workz.json` (including projects added in WorkZ). The CodeZ empty-project screen also lists recent projects for one-click opening.

### Changed

- Clicking a CodeZ-origin session inside WorkZ now opens and continues it directly in WorkZ instead of switching the window to CodeZ. Both modes act as synchronized views over the same project and session database; only WorkZ's projectless sessions remain specific to WorkZ.

### Known limits

- This release has not passed the upgrade acceptance gates. The unaccepted items and user test cases remain in `docs/upgrade/acceptance-0.9.12.md`.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.14 still apply.

## [0.9.14] - 2026-10-18

### Fixed

- On Windows, the WorkZ session list no longer shows the same project twice. Versions up to 0.9.11 saved project paths with the `\\?\` prefix in `workz.json`, and the list treated them as different from the plain path. Saved projects are cleaned and merged on load. The merged list is written back the next time a project is added or removed.
- WorkZ also strips the prefix from the saved scope, the recent working directories, and the per-project last task, so those no longer point at a second copy of the project.

### Known limits

- This release has not passed the upgrade acceptance gates. The unaccepted items and user test cases remain in `docs/upgrade/acceptance-0.9.12.md`.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.13 still apply.

## [0.9.13] - 2026-10-18

### Changed

- Removed the extra panels that 0.9.12 left inside the CodeZ and WorkZ chat windows: durable plan refresh, run history, task details, delivery review, artifact and task-file viewers, and the WorkZ reminder bar. Those controls repeated the existing task list, the header usage ring, and the top-right changes and artifacts drawer.
- The persisted task plan still loads into the collapsible task list when a session opens. There is no manual refresh button.
- WorkZ team, agent, and isolate controls sit in the bottom bar again. The unstyled "advanced options" disclosure is gone.

### Known limits

- This release has not passed the upgrade acceptance gates. The unaccepted items and user test cases remain in `docs/upgrade/acceptance-0.9.12.md`.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.12 still apply.

## [0.9.12] - 2026-10-18

### Added

- CodeZ shows a single line above the first message when no model API key is configured, with a button that opens Settings.

### Changed

- Removed the configuration summary and the first-run checklist from the CodeZ and WorkZ chat windows. They took layout space and did not explain what they controlled.
- Task details, run history, and the notification inbox refresh on window focus at most once every 2 seconds. A burst of focus, online, and visibility events costs one reload.

### Fixed

- On Windows, project paths returned to the interface no longer carry the `\\?\` prefix. Files under the project open again.
- On Windows, git child processes no longer open console windows. Focus-triggered refreshes were starting git, which opened a console window that took focus and triggered the next refresh.

### Known limits

- This release has not passed the upgrade acceptance gates. See `docs/upgrade/acceptance-0.9.12.md` for the unaccepted items and the user test cases.
- Installers are unsigned. macOS and Linux builds were not run on real machines.
- Other limits listed under 0.9.11 still apply.

## [0.9.11] - 2026-10-17

### Security

- OAuth connector tokens (`access_token`, `refresh_token`) are stored sealed in `credentials.json` instead of as plaintext. The sealing key is the app's master key, which the operating system protects: DPAPI on Windows, Keychain on macOS, Secret Service on Linux.
- Legacy plaintext tokens written by 0.9.8 to 0.9.10 are still readable. They are rewritten sealed the next time the connector's tokens are updated, cleared, or saved.
- Diagnostics export redacts the plaintext of sealed tokens.
- Fails closed: if the OS credential store is unavailable, OAuth token storage returns an error and the file is left unchanged. API key connectors are not affected.

### Known limits

- API key connectors (`api_key`) are still stored as plaintext in `credentials.json`.
- On Linux, OAuth token storage needs a running Secret Service (for example GNOME Keyring or KWallet). This is verified by CI only, not on a real Linux desktop.
- Other limits listed under 0.9.8 and 0.9.10 still apply.

## [0.9.10] - 2026-10-17

### Fixed

- Linux and macOS release builds compile again. Windows-only code paths in the desktop crate (`task_view_read` and the terminal resource snapshot in `ide`) no longer leave unused items on other platforms. The vendored `piscis-kernel` fix from 0.9.9 is included.

### Known limits

- The Linux and macOS compile fixes are verified by the release pipeline, not by a local build.
- Other limits listed under 0.9.8 still apply.

## [0.9.9] - 2026-10-17

### Fixed

- Attempted Linux and macOS release builds. The vendored `piscis-kernel` shell imports are compiled only on Windows. The release still failed on other platforms because of unused items in the desktop crate, so 0.9.9 shipped Windows installers only.

## [0.9.8] - 2026-10-17

### Added

- CodeZ and WorkZ share project sessions. Opening the same project in both shows the same sessions. Team sessions stay read-only in CodeZ.
- Mode-switch shortcuts: Ctrl/Cmd+Shift+N switches to CodeZ and Ctrl/Cmd+Alt+N switches to WorkZ.
- Connector OAuth2 authorization. The connector row has Authorize and Refresh buttons. Authorization uses PKCE, checks `state` on the loopback callback, and stores tokens in the connector's credential file.

### Changed

- Failing agent turns record `task_id` and `environment_id` in diagnostic logs, so an export can locate the failing task.
- The first-run checklist re-reads configuration when the settings panel closes.
- Backup verification refuses unsupported schema versions with explicit reasons.

### Known limits

- Desktop journeys for the new sharing, shortcut, and OAuth flows have not been run by hand. Automated tests cover them.
- The OAuth flow has been tested only against a local fake token endpoint, not against real providers.
- The Windows installer is unsigned. macOS and Linux bundles have not been verified on real machines.

## [0.9.7] - 2026-10-06

### Fixed

- The stop-button ring now spins on software-rendered WebKit. The previous comet was a filtered `::before`, which painted once and then stayed still. It is a real border arc, the same kind of spinner as the tool-call indicator.

## [0.9.6] - 2026-10-04

### Changed

- WorkZ no longer shows the locked-mode banner above the composer. Hover the locked isolate, team, or agent control for the same explanation.
- While a task is running, the composer stop button carries a rotating light ring instead of a spinner beside it. The ring goes out when the run stops, and it keeps spinning in reduced-graphics mode.

## [0.9.5] - 2026-10-03

### Fixed

- Progress spinners keep spinning on software-rendered machines. The reduced-graphics mode was stopping every infinite animation after one cycle, including the tool-call spinner and the spinner beside the send button. Decorative pulses still play once.
- Assistant messages render as Markdown during a turn. CodeZ was showing the whole in-progress reply as raw source, and WorkZ did the same for the live tail. Text still arrives in 80 ms batches, so Markdown is not reparsed on every token.

### Changed

- README rewritten to describe the 0.9.x app: CodeZ, WorkZ, the extension host, and remote development.

## [0.9.4] - 2026-10-03

### Added

- Status-bar remote entry: a ⇄ Remote button (also shown when no folder is open) opens a dialog to pick an SSH, Dev Container, Docker or WSL target, install an SSH public key with a one-time password, browse the remote folder and open it. Non-default SSH ports are saved as a `~/.ssh/config` alias.

### Fixed

- Idle CPU: an unchanged language switch no longer re-fired every listener and rebuilt file watchers until the web process was killed. Tauri event subscriptions are dropped if the component unmounts first, WorkZ streaming flushes in batches and renders plain text until the turn ends, closed editor tabs release their Monaco models, and graph-index and browser-screenshot polling back off while idle or hidden.
- File listing and git-repo discovery run off the async runtime, and project-edge updates re-render only the components that read the changed field.
- SSH probes time out after 15 seconds instead of hanging on an unreachable host.

### Changed

- On software-rendered machines (virtual GPUs such as vmwgfx), backdrop blur is removed and infinite animations play once. Override with localStorage `agentz-graphics` set to `low` or `full`.

## [0.9.3] - 2026-10-03

### Changed

- Tool calls between assistant text stay in one block: collapsed to the current call, and at most five lines when expanded (further calls scroll inside the block). The next assistant text leaves that block in place and starts a new one.

## [0.9.2] - 2026-10-03

### Fixed

- Git status no longer piles up one process per refresh: concurrent requests for the same repo share a single run, and the UI coalesces refreshes that arrive while one is in flight. A refresh storm was pinning the CPU and making the terminal unresponsive.
- Terminal resize now changes the PTY size. The previous command was a no-op and its argument names did not match the frontend.

## [0.9.1] - 2026-10-02

### Fixed

- Linux and macOS builds (broken since 0.8.0): a Windows-only helper in dev environment detection was dead code elsewhere, and the workspace treats warnings as errors.

## [0.9.0] - 2026-10-02

### Added

- Remote development (preview) over SSH, Docker and WSL: open `agentz-remote://` projects; files, search, git (incl. nested repos), file watching and terminals run on the target through a deployed agentz-server. A pinned Node runtime is bundled and uploaded when the remote has none.
- Remote language servers (bridged with URI rewriting), remote debugging (debug adapter runs on the target, paths mapped both ways), port detection with auto-forward and a Ports panel, SSH key setup for password hosts.
- Code index, code graph and symbol tools work on remote projects via an incremental local mirror that honours `.gitignore`.

### Fixed

- Debug sessions with debugpy no longer hang: launch/attach now follows the `initialized` → `configurationDone` handshake.
- A hung extension host can no longer block every other request (separate stdin lock, 30s write timeout).
- The extension host exits when its broker disconnects instead of spinning at 100% CPU; processes it spawned are stopped.
- Linux file watching no longer walks `node_modules` (per-directory watcher with ignore list and cap).

## [0.8.0] - 2026-10-02

### Added

- Dev environment clues: on an agent turn the project is scanned (Rust, Node/TS, Python, Go, Java/Kotlin, .NET, C/C++, Tauri, PHP, Ruby, Dart/Flutter) and missing or outdated toolchains are injected as an implicit `devenv-setup` todo plus a system-prompt hint. The agent configures them via `shell` (user confirms) and settles the todo with the new `devenv` tool (`check` / `resolve` / `dismiss`; dismissals persist in `.agentz/devenv.json`).
- `app_control` is now a tiered, audited settings tool: `describe_settings`, free / privileged / locked fields (secrets and authorization switches are locked; safety, harness and routing fields prompt every time), field-level diffs, `append_instructions` (append-only), assistant/team `get` / `update` / `delete`, `list_changes` and `rollback`, plus a per-session write rate limit.
- Settings panel: "Agent change log" with one-click rollback.

## [0.7.3] - 2026-10-01

### Fixed

- Chat now stays pinned to the newest message through tool calls (CodeZ and WorkZ); content growth is no longer mistaken for the user scrolling up.
- Light theme: settings-saved toast, file diff cards, tool status colors, tool trace output, Markdown fallback blocks, mermaid blocks, collaboration board CLI output and the notebook extension are now readable.
- LSP tool: rust-analyzer / typescript-language-server / pyright / clangd are now found in `~/.cargo/bin`, the npm global dir and other per-user tool dirs even when the app inherits a trimmed PATH (piscis-engine v0.8.67).

## [0.7.2] - 2026-10-01

### Added

- Model settings: temperature, top_p and thinking mode for the main model and each multi-model entry; per-model streaming override.
- Model settings: base URL is auto-filled with the provider default when switching provider (also in the multi-model editor); OpenAI now has a base URL field.

### Fixed

- Custom base URL is now honored for every provider (DeepSeek, Qwen, Kimi, MiniMax, Zhipu, Anthropic); previously the engine ignored it for all but OpenAI/custom. Requires piscis-engine v0.8.66.

## [0.7.1] - 2026-10-01

### Added

- WorkZ sidebar: "Open this project in CodeZ" button on each project (switches the CodeZ project and shows CodeZ).

## [0.7.0] - 2026-10-01

### Added

- **WorkZ projects and sessions**: the sidebar is organized as Projects (directory or repository) -> sessions, plus "Sessions without project". Add or remove projects from the sidebar; projects opened in CodeZ are registered automatically.
- **Plain sessions work without opening a folder**: they run in a default working directory (configurable in Settings, falls back to a `workspace` folder in the app data directory). The Composer has a working-directory picker (default, recent, choose folder); the choice is remembered per session.
- Settings: default working directory and the list of registered projects.

### Fixed

- Light theme: Markdown code blocks no longer render as black on black (themed code background and a light highlight.js palette).

### Known limitations

- Plain sessions have no worktree isolation and no repo Wiki; the agent journal for them is written under the chosen working directory.

## [0.6.11] - 2026-10-01

### Added

- **Code map**: new title-bar entry with a module dependency graph (expand to files), a size/90-day-churn treemap and a detail sidebar (imports, imported-by, open in editor, send to agent).
- **`symbol_search` / `impact` tools**: tree-sitter symbol and call index (Rust, TS/JS, Python, Go), persisted in `.agentz/graph.db`, for finding definitions and the callers/files affected by a change.
- **Call graph view** in the Code map: pick a function to see its callers and callees.
- Code map dependency graph now uses ELK layered layout.
- **Real LSP client** for the `lsp` tool; `read_lints` falls back to `cargo check` / `tsc` when no language server exists.
- Delegated sub-agents can read dependency sources (cargo registry, node_modules).

### Changed

- Search ranking favors code over docs; graph tools explain when the index is still building; the system prompt now teaches a locate -> pin down -> impact -> verify workflow.
- Turns have no default wall clock; a timed-out turn resumes automatically. Closing the window while turns run asks for confirmation and cancels them cleanly.
- Wiki menu: "Code map" plus "Generate architecture doc".

## [0.6.10] - 2026-10-01

### Fixed

- **Session restore**: CodeZ and WorkZ reopen the last active session/task after a restart.
- **Timeout notice**: clearer wording; partial output is kept.

## [0.6.9] - 2026-10-01

### Fixed

- **Streaming**: every token rendered twice because an `onChatEvent` subscription leaked when its effect was cleaned up before the listener registered; subscriptions are now disposal-safe.
- **Layout**: raising the UI font scale left blank strips on the right/bottom (percent size was divided by the zoom factor).
- **Interrupted turns**: a reply streaming when an error/timeout/stop hit is now persisted, tool calls and results are rebuilt from history, and the session id is adopted from events so the next message no longer starts a new session.
- **Toasts**: the always-empty capsule at the bottom (exit/settings toast) now shows its text; stuck toasts in CodeZ/WorkZ are cleared by a single timer.
- **Turn timeout**: no longer a hard failure; partial output and open todos are kept and the turn auto-resumes (max 2).
- **Interactive cards**: a pending `chat_ui` card no longer leaks into a new/switched session.

### Changed

- **Engine**: upgrade `piscis-engine` to v0.8.64 (RobotZ v0.1.8). Fixes DeepSeek `tool_call_ids did not have response messages` for parallel tool calls, makes `file_edit` tolerate CRLF/LF mismatches in multi-line edits, and broadens DeepSeek thinking-mode detection.
- **Tool calls** are shown inline, interleaved with the reply text, with a grey one-line result preview; persisted history keeps the tool trace.
- **Session restore** loads newest-first in pages, opens on the latest message and lazy-loads older messages on scroll-up.

## [0.6.8] - 2026-09-29

### Performance

- **Session history**: mount only the newest 50 chat turns and restore 10 older turns per upward-scroll request, keeping persisted history intact while bounding WebKit DOM work.
- **Tool output**: cap the duplicate UI-side result retained for a tool invocation at 64 KiB, preserving diagnostic head/tail text without allowing one command to exhaust renderer memory.

## [0.6.7] - 2026-09-29

### Performance

- **Chat / Composer**: isolate completed Markdown turns from Composer keystrokes, virtualize offscreen chat layout and batch textarea autosizing to eliminate long-history input lag and code-block flicker.
- **Editor**: retain Monaco file models between edits and enable large-file rendering safeguards, avoiding controlled-buffer scroll jitter and expensive minimap/soft-wrap work for large documents.

## [0.6.6] - 2026-09-28

### Fixed

- **CI / desktop host**: make the workspace pass the current stable Rust Clippy gate, preventing release builds from failing on needless map iteration, a single-pattern match, and a large `ToolResult` error value.

## [0.6.5] - 2026-09-28

### Fixed

- **WebView security**: enable a restrictive Tauri CSP while retaining required local LSP, model API, asset and Monaco worker sources.
- **Inline edit**: discard stale async edit proposals after a tab, Monaco model or request generation changes.
- **Git**: report per-file failures from Discard All instead of silently hiding them.

## [0.6.4] - 2026-09-28

### Fixed

- **Agent harness**: restore real permission confirmation for main chat, replace blocking browser dialogs with IDE approval cards, preserve active Todo state, raise the configurable iteration default to 200, and emit lifecycle diagnostics for text-only exits, cancellations and failures.
- **LSP/editor**: stabilize document URI/session handling and protect inline-edit reject from accidentally undoing subsequent user edits.
- **AI completion**: cancel stale per-document completion requests in both the editor and host process.
- **Git**: use NUL-delimited porcelain status parsing and validate selected nested repository roots for diff/add/reset/discard operations.
- **Markdown security**: use one lazy strict Mermaid loader, sanitize rendered SVG in chat and preview surfaces, and isolate external links.
- **Plan mode**: remove duplicate Plan/Agent decisions and preserve unfinished Todo work across follow-up messages.

## [0.6.2] - 2026-06-25

### Fixed

- **Windows Release 构建**：新增 `tauri.windows.conf.json`，Windows 仅打 NSIS `.exe`，跳过 WiX MSI（preinstall 资源过多导致 `light.exe` 失败）。

## [0.6.1] - 2026-06-25

### Added

- **Wiki 图谱索引状态指示**：标题栏 Wiki 图标按索引状态变色（琥珀=构建中、绿=已完成、红=失败）；菜单项显示节点数与状态标签。

### Fixed

- **重建图谱索引闪退**：Wiki 改为后台排队，不再阻塞 UI 轮询；`graph.db` 写入加锁与 `busy_timeout`；跳过 `bundled/` 扫描；worker `catch_unwind` 防 panic。

## [0.6.0] - 2026-06-28

### Added

- **CodeGraph 式后台图谱索引**：移除 React Flow 图谱 UI；`graph_index` 单 worker + debounce 自动重建 `.agentz/graph.db`；Agent 工具 `graph_explore`（源码 + blast radius）与 `@graph` 改调 `explore_graph`。
- **云市场源配置**：资源库可设置官方市场后端 URL；开发构建默认 `http://localhost:8137`，发布构建默认 `https://www.dimnuo.com`。
- **Monaco 语言检测**：按文件扩展名自动切换语法高亮（TS/RS/Py 等）。
- **CodeBuddy 预装资源**：首次启动将 bundled/preinstall 复制到全局配置（write-if-absent）；新增 100+ 智能体变体、40+ 斜杠命令与若干技能；兼容性审计流水线（`lint:preinstall` / `verify:preinstall`）。
- **斜杠命令**：Composer `/` 补全；后端展开 prompt 并可选限制工具集。
- **项目模板**：打开尚无 `.agentz` 的文件夹时可选应用 CodeBuddy 规则/钩子模板。

### Changed

- **IDE 侧栏**：仅保留 Chat，移除图谱 Tab 与 Wiki「打开图谱」入口。
- **Wiki「重建图谱」**：改为排队后台索引（`graph_index_rebuild`），不再打开 `graph.json`。
- **资源库**：发现页增加云市场源设置；样式与布局优化。
- **Kernel `piscis-engine v0.8.62`**、**RobotZ `v0.1.6`**：对齐合并后的 engine 发布。
- 预装资源经 AgentZ 规范化；排除 142 条不兼容/重复资源（见 `bundled/codebuddy/exclude.json`）。

### Fixed

- **CI**：eslint 忽略 `bundled/`、`scripts/`、`.cache/`；marketplace `dead_code` clippy。

## [0.5.10] - 2026-06-13

### Fixed

- **Composer 技能说明 toast**：CodeZ / WorkZ 悬停技能项时的提示改用主题 CSS 变量（背景、文字、边框），浅色模式下不再显示黑底金字。

## [0.5.9] - 2026-06-13

### Fixed

- **Composer 下拉菜单**：CodeZ / WorkZ 输入框下方模式、模型、技能等选择列表改用视口 `fixed` 定位，避免被聊天侧栏 `overflow` 裁切右侧。

## [0.5.8] - 2026-06-13

### Added

- **WorkZ 协作看板**：进行中任务卡片内嵌 2 行 CLI 窗，实时显示 Koi 流式输出。
- **技能卸载保护**：已被「从技能生成」的匿名代理引用的技能，须先删除代理方可卸载。

### Changed

- **资源库 UI**：内容区内边距、书页式分类标签；修复发现页 `MarketGridView` 误用 pill 样式导致巨大圆角框；空列表改为轻量提示。
- **Composer 技能列表**：列表仅显示名称，悬停以 toast 展示说明。
- **匿名代理**：内置标签与名称同行；内置项操作改为「重定义」；钩子保存按钮文案缩短为「保存钩子」。
- **工作流设计器**：打开与自动布局后自动适应视野并居中（等待宽面板动画完成后再 `fitView`）。
- **WorkZ 文案**：副标题与团队模式空状态 i18n 更新（变更浮窗在右上角）。

## [0.5.7] - 2026-06-13

### Fixed

- **Windows icons**: regenerate `icon.ico` with 16–256px frames (was single 16×16, blurry on HiDPI).
- **Release CI**: `cargo fetch` retry + `CARGO_NET_RETRY` for flaky crates.io; job `timeout-minutes` to avoid zombie runners.

## [0.5.6] - 2026-06-13

### Changed

- **Kernel `piscis-engine v0.8.60`**: cross-platform plan path validation fix from engine CI.

## [0.5.5] - 2026-06-12

### Added

- **统一资源库**：标题栏 Store 图标打开「资源库」面板，合并技能/工具/智能体/团队/连接器/匿名代理的已安装、发现与编排视图；设置仅保留模型、消息渠道、扩展、规则、钩子。
- **WorkZ 通用智能体**：输入框下方技能/连接器多选；空列表可深链到资源库发现页。
- **协作增强**：pool 按 `team_id` / `workflow_run_id` 隔离（`piscis-engine v0.8.59`）、depends_on 等待 UI、patrol/heartbeat、连接器白名单。

### Changed

- 设置「助理」改名为「消息渠道」；匿名代理文案与资源库 i18n 统一（中英文）。
- 移除 MarketplacePanel、ClawHubPanel 及设置内技能/工作室/子代理/连接器 Tab。

## [0.5.4] - 2026-06-12

### Added

- **WorkZ 协作机制全面修复**：协调者 `pool_session_id` 绑定、`swarm_coordinator` 收敛协议、成员 Koi **Stop Gate** 六层 prompt、pool 按 `team_id` 隔离、workflow run 独立 pool、depends_on 等待 UI、patrol/heartbeat、workflow driver 互斥与 LLM 分支 fail-fast。
- **CodeZ Browser E2E**: `browser-changed` event sync, auto-open Browser tab on agent actions, close guard when agent is active.
- **Browser automation**: RobotZ `snapshot`/`ref`/`lock`/`fill`, assert DSL (`assert_url`, `assert_visible`, `assert_text`), `wait_for_text`.
- **Kernel alignment**: `piscis-engine v0.8.58` (`web_fetch`, `piscis-ide-tools`), shared `robotz-browser v0.1.2` with IDE panel.
- **Skills / debug**: `codez-e2e-testing` skill; `debug_scenarios_list` with four browser regression prompts.

### Changed

- System prompt and Plan mode docs for `web_fetch` vs `browser` E2E workflow.
- `toolDisplay` summaries for `browser` and `web_fetch`.

## [0.5.3] - 2026-06-10

### Fixed

- **Workflow designer toolbar**: add-node and auto-layout buttons now share a fixed 28px height with aligned glyphs and labels.

## [0.5.2] - 2026-06-10

Complete i18n coverage for IDE explorer controls, git panel, unsaved prompts, and interactive chat forms.

### Fixed

- **Explorer**: expand-all / collapse-all tooltips showed raw keys (`ide.expandAll`) instead of translated text.
- **Git panel**: `noGitRepos` message; **editor**: unsaved close confirmations.
- **Interactive cards**: all validation and wizard strings now localized (EN + ZH).

## [0.5.1] - 2026-06-10

Workflow designer UX — structured branch/loop conditions, auto fit view, and edge flow animation.

### Added

- **Condition builder** for workflow branch (expr) and loop `exit_when`: pick blackboard key, operator, and value with live expression preview; advanced raw-text mode still available.
- **Expr branch paths**: true/false target dropdowns auto-set edge labels; graph validation for missing paths and unknown keys.

### Changed

- **Workflow canvas** auto fit view on open and after auto-layout; edge dash animation flows source → target (arrow direction).

## [0.5.0] - 2026-06-10

Trusted skill evolution — quadrant storage, background review, Curator, and Settings UI.
Requires **piscis-engine v0.8.57**.

### Added

- **Quadrant skill storage**: `skills/installed/`, `.draft/`, `learned/`, `.archive/` under the global config dir.
- **`skill_manage` tool**: agent-mode controlled writes to draft/learned skills (`safe_join` path guard).
- **Background review**: post-turn memory extract + skill review fork (configurable via `skill_evolution` in settings).
- **Curator**: stale marking, archive, LLM merge; idle scheduler every 30 minutes.
- **Memory hardening**: L2 compaction (3×) triggers session memory consolidation.
- **Settings → Skills**: evolution panel (promote/discard/lock), Curator controls, review toggle.
- **Global skill DB**: `{config_dir}/piscis.db` for skill metadata/revisions (separate from project session DB).

### Changed

- **ClawHub install** targets `skills/installed/` and registers skills in the global DB.
- **piscis-engine** dependency bumped to **v0.8.57**.

## [0.4.6] - 2026-06-09

Edge drawer polish, explorer expand/collapse, and faster git refresh after agent edits.

### Changed

- **Global edge drawer**: compact number badge (no label overlap with title bar);
  changes and artifacts merged into one panel with explicit close; flat artifact list.
- **CodeZ chat**: code blocks show copy only — removed Apply-to-editor button.
- **Context ring**: progress ring and percentage both use `totalInputBudget`.

### Added

- **Explorer**: expand-all / collapse-all toolbar buttons.
- **Agent turns**: emit `ide-file-changed` after file-modifying tools so git status
  and the file tree refresh even when inotify is delayed.

## [0.4.5] - 2026-06-08

WorkZ-first startup, per-mode model memory, and quieter composer placeholders.

### Changed

- **First launch** opens **WorkZ** by default; later launches restore the mode you
  exited in (saved in workspace layout).
- **CodeZ / WorkZ** title-bar toggle: WorkZ is listed before CodeZ.
- **Model picker** selections are saved independently for CodeZ and WorkZ and
  restored on the next app start.
- **Composer placeholder** text is smaller and closer to the background so it is
  less distracting.

### Added

- **WorkZ**: full artifact preview (images, PDF, Markdown, HTML, code), tree-style
  artifacts drawer, worktree path resolution, three-tab task view (main / Koi
  chatroom / coordination log), task-bound team mode, and sidebar status fixes.
- **CodeZ**: global edge drawer for git changes and artifacts; inline file diff
  cards after agent turns; task-card composer layout; Flash LLM session titles for
  CodeZ and WorkZ.
- **Extension host**: dev build sync, ready-handshake race fix, and more reliable
  `$initialize` startup.

### Fixed

- CodeZ black screen from `UserMessage` / `chatFileRefs` parse bugs and duplicate
  React keys in tool steps.
- Composer image chips persisting after send (async paste + attach replay).
- Extension host RPC timeouts and stale `host.js` in dev builds.
- Browser viewport CDP API for chromiumoxide 0.9.

## [0.4.4] - 2026-06-08

Replace native `<select>` dropdowns with theme-aware `DropdownSelect` so popup
lists match the dark UI (fixes unreadable white/gray OS menus in Tauri).

### Changed

- **WorkZ**: team and agent pickers above the goal input now use the same styled
  menu as CodeZ composer (assistant / skills / model).
- **CodeZ composer**: menu implementation consolidated into shared `DropdownSelect`.
- **Settings, Studio, Workflow designer, Connectors, Hooks, IM assistants**:
  all form selects migrated to `DropdownSelect`.
- **`chat_ui` forms**, project picker, and bottom-panel output channel selector
  also use the shared component.

## [0.4.3] - 2026-06-07

Rich assistant message rendering aligned with OpenPiscis, plus CI clippy fix
from v0.4.2.

### Added

- Agent chat markdown: **Mermaid** diagrams (` ```mermaid `), **HTML** (inline or
  ` ```html ` blocks, sanitized), **KaTeX** math (`$…$` / `$$…$$`), GFM tables
  with horizontal scroll, and clickable images.
- `chat_ui` card `text` / `section.description` blocks now render markdown
  (mermaid, HTML, formulas) instead of plain text.
- Render error boundary: malformed markdown falls back to a code block instead of
  breaking the chat pane.

### Fixed

- `WorkspaceCloseGate`: implement `Default` so `cargo clippy -D warnings` passes
  in CI (Desktop host job).

## [0.3.1] - 2026-06-07

Production-readiness release: CI quality gates, test coverage, logging, and
release documentation. Still **unsigned / manual download** — see
[`RELEASE.md`](RELEASE.md).

### Added

- CI gates: `cargo test`, `cargo clippy -D warnings`, `npm test`, ESLint.
- Frontend `vitest` suite for `validateGraph`; Rust unit tests for workflow
  routing (`branch_target`, `loop_step`, `fault_decision`) and run persistence.
- Daily-rolling file logs at `{config}/logs/agentz.log` (plus stdout).
- `CHANGELOG.md`, `LICENSE`, `.env.example`, `RELEASE.md`.

### Changed

- Workflow runner and persistence refactored for testability (no behavior change).
- Swarm patrol hardened against poisoned locks and single-round panics.
- Brand cleanup: AgentZ naming in LSP bridge, DingTalk UA, i18n hints.
- `extension-host` version aligned to `0.3.1`.

## [0.3.0] - 2026-06

First publicly released build. Distributed **unsigned / manual download** across
Linux (x86_64 + aarch64), Windows (x86_64 + aarch64) and macOS (Apple Silicon +
Universal); see [`RELEASE.md`](RELEASE.md) for install notes.

### Added

- **Teams: dual execution modes.** A team can now run in one of two modes:
  - **Swarm** — emergent, coordinator-driven collaboration. Member agents share
    an organizational spec (`org_spec`) which is now injected into every member
    turn, and a background patrol recovers stale tasks and activates pending
    todos so swarm runs do not stall.
  - **Workflow** — deterministic, graph-driven execution with `start` / `end` /
    `agent` / `branch` / `loop` / `human` nodes, a shared blackboard, branch
    conditions (LLM classification or simple expression), bounded loops, and a
    total-step circuit breaker.
- **Workflow designer** — a React Flow canvas with node inspector, semantic edge
  coloring/labels, auto-layout, and live graph validation.
- **Workflow run experience** — live run panel with streaming agent output,
  human-in-the-loop steps, cancel, and re-run; plus a run-history browser with
  live updates, per-run delete, and clear-finished.
- **Node-level fault tolerance** — agent nodes support `max_retries` and an
  `on_error` policy (`fail` or `skip`).
- Built-in "Code Review Loop" sample workflow team.

### Changed

- Coordinator preamble (role + `org_spec`) is now injected only on the first
  turn of a swarm session instead of repeating every turn.

### Quality / tooling

- Extracted pure decision functions (`branch_target`, `loop_step`,
  `fault_decision`) and `&Path` persistence cores for unit testing.
- Added Rust unit tests (routing, loop bounds, retry/skip, run persistence
  guards) and a frontend `vitest` suite covering `validateGraph`.
- CI now runs `cargo test`, `cargo clippy -D warnings`, `npm test`, and ESLint;
  added Prettier config and `lint`/`format` scripts.
- Hardened the swarm patrol against poisoned locks and single-round panics.

[0.4.5]: https://github.com/njbinbin-piscis/AgentZ/releases/tag/v0.4.5
[0.4.4]: https://github.com/njbinbin-piscis/AgentZ/releases/tag/v0.4.4
[0.4.3]: https://github.com/njbinbin-piscis/AgentZ/releases/tag/v0.4.3
[0.3.1]: https://github.com/njbinbin-piscis/AgentZ/releases/tag/v0.3.1
[0.3.0]: https://github.com/njbinbin-piscis/AgentZ/releases/tag/v0.3.0
