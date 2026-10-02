# AgentZ Extension Host

A clean-room, VS Code–compatible **extension host** for AgentZ. It runs real
`.vsix` extension JavaScript against a `vscode` API implementation and talks to
the AgentZ renderer (Monaco + React) over a structured RPC protocol — the
"Theia-style compatible host" approach.

## Architecture

```
Tauri renderer (React + Monaco)          Tauri host (Rust)         Node sidecar (this package)
┌───────────────────────────┐  ext_host  ┌──────────────────┐  stdio  ┌──────────────────────────┐
│ MainThread* bridges        │◀──events──▶│ ext_host.rs      │◀──────▶ │ extensionHostProcess      │
│ rpcProtocol (renderer copy)│  invoke    │ broker (stdin)   │  NDJSON │ rpcProtocol + ExtHost*    │
│ Monaco language registry   │            │ vsix.rs (unpack) │         │ vscode API factory        │
└───────────────────────────┘            └──────────────────┘         │ 3rd-party extension JS    │
                                                                       └──────────────────────────┘
```

- **Transport**: line-delimited JSON (NDJSON). The renderer cannot talk to the
  Node process directly, so the Rust host brokers it: the sidecar's stdout lines
  are emitted on the `agentz:ext-host` Tauri event channel, and the renderer
  sends frames back through the `ext_host_send` command (→ sidecar stdin).
- **Protocol**: `src/common/{proxyIdentifier,rpcProtocol,protocol,dto}.ts`.
  `MainThread*` shapes run on the renderer; `ExtHost*` shapes run here. Methods
  are `$`-prefixed and routed by numeric proxy id. The renderer keeps a
  byte-compatible copy of the `common/` modules under `src/extensions/common/`.
- **API surface**: `src/host/apiFactory.ts` assembles the `vscode` namespace
  from the `ExtHost*` services and the concrete types in `types-impl.ts`.
  `require('vscode')` is intercepted per-extension in `extensionService.ts`.

## Building

```bash
npm install
npm run build      # -> dist/host.js (+ dist/smoke.js)
npm run smoke      # in-process loopback test (no Tauri required)
npm run typecheck
```

`dist/host.js` is what the Rust host launches:
`node <resources>/extension-host/host.js`. Resolution order in
`ext_host.rs`: explicit arg → `$CODEZ_EXT_HOST_JS` → bundled resource → dev
paths (`extension-host/dist/host.js`). Override the Node binary with
`$CODEZ_NODE`.

## Packaging (Node runtime)

Tauri does not embed Node. For shipped builds:

1. `tauri.conf.json` bundles `dist/host.js` into app resources
   (`bundle.resources`), and `beforeBuildCommand` runs `npm run build:exthost`.
2. The app currently launches the host with the system `node` (or `$CODEZ_NODE`).
   To make installs fully self-contained, add a per-platform Node binary as a
   Tauri **externalBin** sidecar and point `node_bin()` at it. (Tracked as the
   remaining hardening item — see milestone M17.)

## Licensing strategy

- This host is an **MIT clean-room implementation**. It does *not* vendor source
  files from `microsoft/vscode` or `eclipse-theia/theia`; the public `vscode`
  extension API contract and the RPC pattern are reimplemented here.
- Eclipse Theia (EPL-2.0) was used only as a *reference* for the MainThread-side
  design; no Theia source is copied, so the project stays MIT by default.
- Extensions are installed from **Open VSX** (and user-supplied `.vsix`) to avoid
  the VS Code Marketplace Terms of Service.

## Compatibility tier (professional)

Implemented capability bridges: commands, documents/editors, workspace + fs +
configuration, messages, status bar, quick input, output channels, language
features (completion / hover / definition / references / highlights / symbols /
formatting / code actions / signature help / diagnostics), tree views, webviews,
terminals, SCM providers, tasks (via PTY), debug (DAP), testing, and notebook
serializers.

Known limits: native (`.node`) modules require ABI-matching prebuilds.

## Remote targets (agentz-server)

The same `host.js` doubles as **agentz-server**: stdin lines carrying a
`$agentz` key are a control plane (`src/server/agentzServer.ts`: `info`,
`fs.*`, `exec`, `proc.*`) answered on stdout as `{"$agentz":"res"|"event"}`,
everything else is extension RPC. The Rust broker (`commands/ext_host.rs`,
`remote/`) launches it over one of:

| Target | Transport |
|---|---|
| local | `node host.js` (bundled pinned Node if present, else system `node`) |
| SSH | system `ssh -T host sh -c …` (honours `~/.ssh/config`, agent, ProxyJump) |
| Docker / Dev Container | `docker exec -i <id> sh -c …` (`devcontainer up` via `@devcontainers/cli`) |
| WSL | `wsl.exe -d <distro> -- sh -c …` |

On connect the bundle is uploaded content-addressed to
`~/.agentz-server/<hash>/server.js`; if the target lacks Node >= 18, a pinned
Linux tarball from `src-tauri/resources/node` (`npm run fetch-node`) is
uploaded too. Extensions whose `extensionKind` allows the workspace side are
tar-synced to `~/.agentz-server/extensions/`. Remote files are addressed as
`agentz-remote://<authority>/<path>` in the renderer and rewritten to `file:`
URIs at the RPC boundary (`typeConverters.ts`).

### Remote workspaces in the IDE

A remote folder is opened as a project whose dir is the URI itself, e.g.
`agentz-remote://ssh-remote+box/home/me/proj`. Because the IDE builds file
paths as `${projectDir}/${rel}`, every `ide_*` command receives a remote URI
and routes it (`crate::remote::resolve`):

| Feature | Remote implementation |
|---|---|
| File tree / read / write / create / rename / delete | `fs.*` on the server |
| Find in files | `search` (ripgrep on the remote, else grep) |
| External change reload | `fs.watch` → `ide-file-changed`; re-armed after reconnect |
| Git panel | `git` run on the remote via `exec`; nested repos found by `git.discover` (depth 4, cached) |
| Terminal | local PTY running `ssh -tt` / `docker exec -it` / `wsl --cd` — a real remote TTY |
| Language features | extensions on the remote host, plus built-in LSP (`remote/lsp.rs`): the server is spawned on the remote via the target transport, URIs rewritten `agentz-remote://` ↔ `file://` at the bridge |
| Agent tools | `tools/remote_fs.rs` replaces `file_*` / `shell` with remote versions |
| Debugging | `dap_start` with `remoteDir` spawns the adapter on the remote (target transport); `dapClient` maps `agentz-remote://` ↔ POSIX paths for breakpoints / stack frames |
| Code index | `remote/mirror.rs` keeps an incremental local source mirror (`~/.agentz/remote-mirror/<hash>`, files <512 KB, vendored dirs pruned); codebase / graph / symbol indexes, their agent tools and `@codebase` / `@graph` run on it |
| Agent context | `.agentz/rules` / `.cursor/rules` and `@file` mentions are read from the remote |
| Port forwarding | `ports.list` (Linux `/proc/net/tcp`); SSH uses `ssh -N -L`, Docker/WSL tunnel over the server (`tcp.*`). Ports ≥1024 that open after connect are auto-forwarded with a notification |
| Password hosts | "Set up key login" installs your public key once (`remote/ssh_setup.rs`) |

Commands issued while the connection is still being deployed wait for it
(`ExtHostManager::wait_for_authority`). The status bar shows a remote badge
that turns into a reconnect button when the link drops; the renderer also
retries with backoff.

Remote limits: the host connection is non-interactive, so password-only hosts
need the one-time key setup first. New host keys are accepted automatically
(`accept-new`); changed keys are rejected. Port auto-detection is Linux-only
(manual ports work anywhere). The index mirror re-syncs at most every 20 s
(on index rebuild / tool use), so results can lag the remote briefly. musl-only images (Alpine) need a
system Node because the bundled tarballs are glibc builds.
