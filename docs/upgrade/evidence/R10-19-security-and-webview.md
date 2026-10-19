# R10-19 Evidence: All-Outlet Secret Scrubbing & Webview Isolation Gate

- **Date**: 2026-10-19
- **Task**: `R10-19` — Enforce zero plaintext secret leakage across all 4 egress boundaries (Diagnostic Export, Settings IPC, `app_control`, and Passive Event Projection) and verify extension Webview CSP/nonce/resource isolation.

## 1. Four-Outlet Secret Scrubbing Gate

1. **Diagnostic Export (`src-tauri/src/diagnostics.rs`)**:
   - Collects secret keys via `is_secret_key`, sensitive URLs (`url` / `*_url`), MCP server credentials (`command`, `args`, `url`, `env`, `headers`), and unsealed OAuth connector tokens (`unseal_credentials`).
   - Redacts literal, URL-encoded (`%XX` / `%xx`), and Base64 (`STANDARD`, `STANDARD_NO_PAD`, `URL_SAFE`, `URL_SAFE_NO_PAD`) variants longest-first.
   - Fails closed on symlinks/junctions, malformed connector files, or log file size/mutation races.
   - Verified by 15 unit tests in `diagnostics::tests` (`15 passed`).
2. **Settings IPC (`src-tauri/src/commands/settings.rs` & `src/services/tauri/settingsSecrets.ts`)**:
   - `to_dto` never echoes model API keys or MCP credentials; returns boolean `SecretReferenceDto` descriptors instead.
   - `splitSettingsSecretUpdates` strips all secrets from the general settings payload and routes them through `SecretReplacement` references.
   - Verified by `commands::settings::secret_response_tests` (`7 passed`) and `src/services/tauri/settingsSecrets.test.ts` (`1 passed`).
3. **Agent `app_control` Tool (`src-tauri/src/tools/app_control.rs` & `src-tauri/src/tools/settings_catalog.rs`)**:
   - `Tier::Locked` blocks any write to secret keys; `redact_secrets` masks `is_secret_key`, `mcp_servers`, `env`, `headers`, `credentials`, and `auth` subtrees before returning settings or diffs.
   - Verified by `tools::app_control::secret_projection_tests` (`1 passed`) and `tools::settings_catalog::tests` (`6 passed`).
4. **Passive Run Event Projection (`src/services/tauri/runEventProjection.ts`)**:
   - Strips raw tool inputs/outputs, error detail payloads, and redacts `Bearer ...`, `sk-...`, and `(api_key|access_token|password|secret)=...` patterns in text deltas.
   - Verified by `src/services/tauri/runEventProjection.test.ts` (`6 passed`).

## 2. Extension Webview Security & Isolation (`src/extensions/ui/webviewSecurity.ts`)

- Enforces `default-src 'none'`, `connect-src 'none'`, `frame-src 'none'`, `object-src 'none'`, per-panel cryptographic nonce on `<script>`/`<style>`, and UUID-scoped `agentz-resource://localhost/<token>/` external scripts.
- Enforces `MAX_WEBVIEW_MESSAGE_CHARS` (`1_048_576`) and single-acquisition `acquireVsCodeApi`.
- Verified by `src/extensions/ui/webviewSecurity.test.ts` (`13 passed`).
