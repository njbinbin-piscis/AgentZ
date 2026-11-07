# AgentZ

**English** | [简体中文](README.zh-CN.md)

AgentZ is a desktop AI IDE (current source version **1.0.59**; local engineering and automated acceptance gates pass, see the [1.0.0 release decision](docs/upgrade/release-decision-1.0.0.md)). It puts an editor and autonomous agents in one app on a shared agent kernel. You can edit code by hand in the editor, or hand a goal to an agent that works in an isolated git worktree and comes back for review.

The UI has three first-class modes, switched from the title bar:

| Mode | What it does |
| --- | --- |
| **CodeZ** | Editor-centric. Monaco, file tree, search, Git, terminal, LSP and debugging, with a toggleable AI chat on the right. Close to Cursor. |
| **WorkZ** | Task-centric. Submit a goal and the agent plans, edits and runs tools on its own. Each task runs on its own branch; afterwards you review the diff and merge, open a PR or discard. Agents can also be grouped into teams. Close to Codex. |
| **BotZ** | Agent-centric. Each bot keeps a long-running chat, memory and scheduled routines. A team works in its own office directory: the coordinator splits the goal, and finished work is reviewed, sent back or merged. |

The kernel comes from [`piscis-engine`](https://github.com/njbinbin-piscis/piscis-engine) (`piscis-core`, `piscis-kernel` and `piscis-ide-tools`, currently pinned at `v0.8.71`). AgentZ itself implements the desktop host, the IDE, the extension host and remote development. The repository is licensed under [Apache-2.0](LICENSE).

Installers are published on GitHub Releases. They are currently **unsigned** and must be installed manually; see [RELEASE.md](RELEASE.md) for steps and the system prompts on each platform.

## CodeZ

Open a local folder, or connect to a remote host from the status bar and open a remote folder.

- **Editing.** Multi-tab Monaco editor, Markdown and image preview, light and dark themes, English and Chinese UI.
- **Project tools.** File tree, global search, Git (staging, commits, branches, nested repositories) and an integrated terminal (xterm + PTY). Closing a tab releases its editor model.
- **Language and debugging.** LSP is bridged to language servers over WebSocket. Debugging uses DAP with breakpoints and call stacks; the debug adapter can run locally or remotely.
- **Chat.** Streaming Markdown, collapsible tool calls, stop, queueing the next message, and a session list (new, switch, delete, fork). Forking from a checkpoint can copy only the recent context or the full history. Reference files with `@path`, and the code index and code graph with `@codebase` / `@graph`. Select code and press ⌘K / Ctrl-K for inline edits shown as a green/red inline diff; Enter accepts, Esc reverts. Tab completion shows ghost text.
- **Code map.** Dependency graph and size heat map, backed by an incremental code-graph index ([docs/graph-schema.md](docs/graph-schema.md)).
- **Built-in browser.** Page screenshots and elements can be sent to the chat. Browser tools use [RobotZ](https://github.com/njbinbin-piscis/RobotZ) (`robotz-browser` v0.1.14).
- **Extensions.** Actually runs the JavaScript inside `.vsix` packages instead of only reading `package.json` contributions. See [Extension host](#extension-host) below.

## WorkZ

- **Tasks.** Each task is one kernel session. After you submit a goal, the plan, edits and tool calls stream into the step list.
- **Isolation.** A task creates branch `workz/task-<id>` in `../.agentz-worktrees/task-<id>` next to the project, so the main working tree is never rewritten directly. Several tasks can run in parallel. The review panel diffs `base...branch`, then merges (no-ff), opens a PR with `gh`, or deletes the worktree and branch.
- **Teams.** The studio lets you author reusable agents (role, prompt, skills and tools) and teams. A team runs either as a collaborating swarm or as a graph built in the workflow designer (branches, retries, resume after human approval). The collaboration board shows pool activity.
- **Built-in seeds.** The first run writes the Architect, Coder, Reviewer, Researcher and Writer agents, plus the `fullstack-squad` and `research-duo` teams.
- Plain sessions do not have to be bound to a project; the working directory is set in Settings.

## BotZ

- **Bots.** Agents created in the resource library show up on the roster. Opening one starts a long-running chat: it remembers notes, and you can edit that memory directly. When the daily token budget is spent, wait until the next day or raise the cap in the side panel.
- **Routines.** Schedule a prompt for a bot, either at a fixed time each day or every few minutes, and run it immediately when you want.
- **Offices.** Each office is one team bound to its own local directory, isolated from the others. Members and the collaboration contract can be filled from a team template. Offices cannot be created on a remote directory.
- **Collaboration.** Give the coordinator a goal and it splits the work onto members' desks. The board columns are to do, in progress, blocked or in review, and done. A review can quote a diff line, request changes, approve, or approve and merge.

## Agent capabilities

All three modes run the same agent loop. They differ in the tool surface, the context and where artifacts land.

- **Models.** Anthropic, OpenAI, DeepSeek, Qwen, MiniMax, Zhipu, Kimi and custom OpenAI-compatible endpoints. Multiple named models, temperature, top_p, thinking mode, streaming and vision input are configurable. API keys stay on the local machine.
- **Policy.** Strict, balanced and developer presets. Model and tool iterations per task are capped (default 200). Whether the agent auto-continues while plan todos are still open is a setting; it never auto-continues past a question to you.
- **Skills, rules, hooks, MCP.** The skill marketplace uses ClawHub, and local folders or manifests can also be installed. Project rules are read from `.agentz/rules/` (`.cursor/rules/` is also recognised). `hooks.json` inserts hooks before each turn. MCP servers configured in Settings (stdio or SSE) inject tools into the chat.
- **Sub-agents.** The main agent can delegate read-only research with a budget and timeout; sub-agents cannot delegate further. An optional small Flash model can be dedicated to these light tasks.
- **Repository wiki and code index.** The index splits source into windows for keyword search (no embedding key needed). The wiki generates module and architecture overviews from the index.
- **Dev environment hints.** At the start of a turn AgentZ scans for Rust, Node, Python, Go, Java/Kotlin, .NET, C/C++, Tauri, PHP, Ruby, Dart/Flutter and other toolchains. Missing ones become todos for the agent, which checks, fixes or dismisses them with the `devenv` tool.
- **Reversible settings changes.** `app_control` grades each field: secrets and permission switches are locked, and security or routing fields always need confirmation. Settings keeps a change log where individual changes can be rolled back.
- **Messaging channels.** The assistant can connect to Feishu / Lark, WeChat and DingTalk. Connectors turn external services into MCP via OAuth or API keys.

## Remote development

**⇄ Remote** at the bottom left of the status bar (also shown when no folder is open) opens the connection dialog:

1. Choose SSH, Dev Container, Docker or WSL. For SSH, enter `user@host` or a Host from `~/.ssh/config`. A non-22 port appends an alias to `~/.ssh/config`.
2. If key-based login fails, enter the password once. AgentZ installs the local public key into the remote `authorized_keys`, so no password is needed afterwards. The password itself is never stored.
3. Browse from the remote home directory or type a path. On open, AgentZ deploys **agentz-server** (the same `host.js` as the extension host) and enters that folder.

Remote project paths look like `agentz-remote://ssh-remote+host/home/me/proj`. Files, search, Git, file watching, terminals, language services, debugging and agent tools all go over this connection. If the remote has no Node ≥ 18, a Linux Node bundled with the app is uploaded. Workspace-side extensions are synced to `~/.agentz-server/extensions/` on the remote. The code index keeps an incremental local mirror.

Limitations:

- The connection probe times out after 15 seconds. New host keys are accepted with `accept-new`; **changed keys are rejected**.
- Automatic port discovery only works on Linux remotes (it reads `/proc/net/tcp`); manual forwarding works on every target. Ports ≥ 1024 opened after connecting are forwarded automatically.
- The bundled Node is a glibc build. musl-only images (such as Alpine) need a system Node.
- The index mirror syncs at most about every 20 seconds, so search results can briefly lag behind the remote.

Transport details are in the Remote targets section of [extension-host/README.md](extension-host/README.md).

## Extension host

`extension-host/` is a clean-room VS Code-compatible extension host: a Node sidecar runs extension JavaScript, and Rust relays NDJSON RPC to the renderer. It does not copy VS Code or Theia source. Extensions are installed from Open VSX or from user-supplied `.vsix` files.

Wired capabilities include commands, documents and editors, workspace and configuration, messages, status bar, quick input, output channels, completion / hover / definition / references / formatting / code actions / diagnostics, tree views, webviews, terminals, source control, tasks, debugging and testing, and notebook serialization.

Native `.node` modules need prebuilt binaries matching the host Node ABI. On remotes the same `host.js` acts as the agentz-server control plane (`fs.*`, `exec`, `proc.*`).

## Repository layout

```
AgentZ/
├── src/                        # Vite + React + TypeScript UI
│   ├── App.tsx                 # CodeZ / WorkZ / BotZ switching, projects and settings
│   ├── i18n/                   # English and Chinese
│   ├── extensions/             # Renderer-side extension RPC, remote URIs
│   ├── services/tauri/         # Frontend wrappers for host commands
│   └── workspaces/
│       ├── codez/              # Editor, chat, Git, terminal, browser, remote dialog
│       ├── workz/              # Tasks, teams, collaboration board, workflows
│       └── botz/               # Bot roster, long-term memory, routines, team offices
├── src-tauri/                  # Tauri 2 desktop host (agentz-desktop)
│   └── src/
│       ├── commands/           # IDE, chat, Git, remote, extensions, marketplace, workflows…
│       ├── remote/             # SSH / Docker / WSL, deployment, port forwarding, code mirror
│       ├── gateway/            # Feishu, WeChat, DingTalk
│       └── lsp/                # LSP ↔ WebSocket
├── extension-host/             # Node extension host, also the remote agentz-server
├── crates/
│   ├── agentz-host/            # Kernel link smoke-test binary
│   └── agentz-task-state/      # Durable task and environment identity
├── vendor/                     # Vendored piscis-engine and secret-service sources
├── bundled/                    # Preinstalled skills and other resources
└── docs/                       # Design notes, reviews and release records
```

## Development

You need Node 20+, Rust (see `rust-version` in `src-tauri/Cargo.toml`; the workspace treats warnings as errors) and the Tauri dependencies for your OS. On Linux:

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev librsvg2-dev \
  libayatana-appindicator3-dev patchelf build-essential xdg-utils
```

```bash
npm install
npm run typecheck          # tsc --noEmit
npm test                   # vitest
npm run tauri dev          # builds the extension host, starts Vite, then the desktop shell
cargo check -p agentz-desktop
cargo run -p agentz-host   # verifies the kernel links, without starting the UI
```

`npm run tauri dev` runs `beforeDevCommand`: first `npm run build:exthost`, then Vite (port 5273). To package:

```bash
npm run tauri build
# artifacts in target/release/bundle/
```

Releases are tag-driven. On a `v*` push, `.github/workflows/release.yml` builds Linux (x86_64 / aarch64 `.deb` and AppImage), Windows (x86_64 / aarch64 NSIS `.exe`) and macOS (Apple Silicon and Universal `.dmg`) and attaches them to the GitHub Release. On Windows, `src-tauri/tauri.windows.conf.json` limits bundling to NSIS, so the `.exe` is the only Windows installer. See the [AZ-005 support matrix](docs/upgrade/evidence/AZ-005-support-matrix.md) for Windows installer support and [docs/known-issues.md](docs/known-issues.md) for known issues. A version bump also covers the lockfiles, README, RELEASE and the matching release notes and checklist; run `node scripts/upgrade-version-check.mjs` to check them and the local doc links, and update [CHANGELOG.md](CHANGELOG.md).

The build compiles the kernel from `vendor/piscis-engine` through the `[patch]` section in `Cargo.toml`. When changing the kernel, keep the vendored copy and the upstream `piscis-engine` repository in sync, then bump the pinned tag.

Optional environment variables are listed in [.env.example](.env.example). Common ones:

| Variable | Purpose |
| --- | --- |
| `AGENTZ_CONFIG_DIR` | Override the global config directory |
| `RUST_LOG` | Log level, e.g. `info` |
| `CODEZ_AUTO_MODEL_ROUTING` | When `1`, planning uses a faster model and execution a stronger one |
| `CODEZ_EXT_HOST_JS` / `CODEZ_NODE` | Extension host script and Node executable |
| `CODEZ_CHROME` | Chrome / Chromium used by the built-in browser |

On VMs without a GPU (for example VMware `vmwgfx`) frosted-glass effects and infinite animations are turned off automatically. To force them, set `agentz-graphics` in localStorage to `low` or `full`.

## Configuration and data

Global configuration (`config.json`, agents, teams, logs) lives in the Tauri app data directory:

- Linux: `~/.local/share/com.agentz.desktop/` (or `$XDG_DATA_HOME/com.agentz.desktop`)
- macOS: `~/Library/Application Support/com.agentz.desktop/`
- Windows: `%APPDATA%\com.agentz.desktop\`

`AGENTZ_CONFIG_DIR` can point elsewhere. Logs are in `logs/` under that directory.

Project data lives in `{project}/.agentz/`: the session database `piscis.db`, rules, hooks, code index and wiki. When migrating from the old CodeZ app, rename the `com.codez.desktop` data directory to `com.agentz.desktop`, and the project's `.codez/` to `.agentz/`.

Without an API key, chat returns an error right away and Settings prompts you to enter a key.

## Documentation

- [CHANGELOG.md](CHANGELOG.md): version history
- [RELEASE.md](RELEASE.md): installing unsigned builds, and plans for signing and auto-update
- [CONTRIBUTING.md](CONTRIBUTING.md): how to build, test and submit changes
- [SECURITY.md](SECURITY.md): reporting vulnerabilities
- [extension-host/README.md](extension-host/README.md): extension host and remote control plane
- [docs/graph-schema.md](docs/graph-schema.md): code-graph nodes, edges and incremental updates
- [docs/known-issues.md](docs/known-issues.md): known issues
- [docs/agentz-design.md](docs/agentz-design.md): early design notes; remote development, the extension host and workflows came later, so this README and the code take precedence

English documents are canonical. Chinese translations sit next to them as `*.zh-CN.md`; when the two disagree, the English version wins. Release records under `docs/upgrade/` are historical and kept in their original language.

## Contributing

Issues and pull requests are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) and follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

[Apache-2.0](LICENSE). Third-party notices are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
