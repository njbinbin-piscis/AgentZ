# Third-Party Notices

## CodeBuddy Marketplace Preinstall (AgentZ v0.6.0)

Source: https://cnb.cool/codebuddy/marketplace.git

After compatibility audit, bundled preinstall contains:

| Category | Bundled | Excluded |
|----------|--------:|---------:|
| Skills | 254 | 3 |
| Agents | 233 | 104 |
| Slash commands | 336 | 36 |
| Project templates | 9 | — |

Counts reflect `bundled/preinstall/` and `bundled/codebuddy/exclude.json` as of 1.0.40.

Excluded ids: `bundled/codebuddy/exclude.json`  
Audit report: `bundled/codebuddy/compatibility-report.json`

### Tool name mapping (CodeBuddy → AgentZ)

| CodeBuddy | AgentZ |
|-----------|--------|
| Read | file_read |
| Write | file_write |
| Edit | file_edit |
| Bash | shell |
| Grep | codebase_search |
| Glob | file_list |

### Regenerate

```bash
npm run import:codebuddy
npm run remediate:preinstall
npm run lint:preinstall -- --strict
npm run verify:preinstall -- --strict
```

### Security

- Preinstall copies are write-if-absent.
- Imported hooks default to `enabled: false`.
- Hub-locked skills (`source: codebuddy`) resist silent overwrite.

## Vendored Rust crates

### secret-service 5.1.0 (modified)

Source: https://github.com/hwchen/secret-service-rs (crates.io `secret-service` 5.1.0)  
License: MIT OR Apache-2.0 (`vendor/secret-service/LICENSE-MIT`, `vendor/secret-service/LICENSE-APACHE`)  
Location: `vendor/secret-service/`, used by `piscis-kernel` for Linux credential storage.

AgentZ changes (opt-in `agentz-no-prompts` feature, no panic on entropy failure) are listed in `vendor/secret-service/AGENTZ-BASELINE.md`.

## Bundled runtimes

### Node.js 22.11.0

Source: https://nodejs.org/dist/v22.11.0/  
License: MIT, plus the licenses of the third-party components shipped with Node.js (see the `LICENSE` file inside each downloaded Node.js distribution).

Release builds run `npm --prefix extension-host run fetch-node` (`extension-host/scripts/fetch-node.mjs`, `NODE_VERSION`), which places the host-platform runtime and linux-x64 / linux-arm64 archives under `src-tauri/resources/node/`. That directory is bundled as an app resource; it runs the extension host locally and is uploaded to SSH / container / WSL targets that lack Node >= 18.
