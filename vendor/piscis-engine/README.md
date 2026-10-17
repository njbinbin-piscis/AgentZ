# piscis-engine

OS / UI-neutral **agent runtime kernel**, extracted from
[`openpiscis`](../openpiscis) so it can be shared by multiple host products
(the `openpiscis` desktop app and the `codez` AI IDE).

This repo contains only the host-agnostic layers — it **never** depends on
Tauri, any GUI framework, or platform-specific tooling. Hosts inject all of
that through the trait contracts in `piscis-core`.

## Crates

| Crate | Role |
|---|---|
| `piscis-core` | Pure contracts: `HostRuntime` / `EventSink` / `Notifier` / `HostTools` / `SecretsStore` + shared schema types. Depends only on serde / chrono / anyhow / async-trait. |
| `piscis-kernel` | The agent runtime: agent loop + harnesses, LLM adapters (claude / openai / qwen / deepseek / kimi / minimax / zhipu), SQLite store + encrypted settings, memory + Dream consolidation, policy / approval, scheduler, security, and platform-neutral tools (file_*, shell, code_run, web_search, ssh, mcp, …). |
| `piscis-cli` | Headless host adapter + the `openpiscis-headless` binary (NDJSON streaming, env-var secrets). Cross-platform agent runner for CI / evals / IDE integrations. |

## Build & test

```bash
cargo build                                   # whole workspace
cargo test  -p piscis-core -p piscis-kernel -p piscis-cli --lib --bins
cargo clippy -p piscis-core -p piscis-kernel -p piscis-cli --all-targets -- -D warnings
cargo build -p piscis-cli --release --bin openpiscis-headless
```

## Consuming this engine from a host

Hosts depend on the crates as path (local dev) or git (CI / release)
dependencies. With both repos checked out as siblings:

```
Projects/
├── piscis-engine/      ← this repo
├── openpiscis/         ← desktop host
└── codez/             ← AI IDE host
```

a host's `Cargo.toml` references:

```toml
[dependencies]
piscis-core   = { path = "../../piscis-engine/piscis-core" }
piscis-kernel = { path = "../../piscis-engine/piscis-kernel" }
# Git dependency (pin rev across hosts — keep in sync with openpiscis / AgentZ / agent-workbench):
# piscis-kernel = { git = "https://github.com/njbinbin-piscis/piscis-engine.git", package = "piscis-kernel", rev = "v0.8.38" }
```

## Provenance

Extracted from `openpiscis` with `git filter-repo`, preserving the full commit
history of `piscis-core` / `piscis-kernel` / `piscis-cli`. The desktop-coupled
`piscis_compact_one` benchmark binary (which links `piscis-desktop`) was
intentionally left behind in `openpiscis`.
