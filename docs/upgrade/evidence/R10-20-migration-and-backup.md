# R10-20 Evidence: Historical Data Migration Fixtures, Writer Quiescence & SQLite Coherent Backup/Restore

- **Date**: 2026-10-19
- **Task**: `R10-20` — Verify 2 historical version migration fixtures (`v0.9.0` & `v0.9.12`), enforce writer quiescence and SQLite sidecar-free consistency in `scripts/upgrade-data-inventory.mjs` & `scripts/upgrade-data-backup.mjs`, and pass encrypted backup/restore drills.

## 1. Historical Version Migration Fixtures (`v0.9.0` & `v0.9.12` -> `1.0.0`)

Added `historical_v090_and_v0912_fixtures_migrate_cleanly_to_1_0` in `src-tauri/src/commands/settings.rs`:
1. **`v0.9.0` Fixture**:
   - `config.json` with legacy `max_iterations: 50` and flat `skills/pdf-helper/SKILL.md` layout.
   - `load_settings_from_dir` migrates `max_iterations` from `50` to `200`.
   - `migrate_flat_skills_to_installed` moves `skills/pdf-helper/SKILL.md` into `skills/installed/pdf-helper/SKILL.md`, writes `.migrated_to_quadrants`, and is idempotent on repeat runs (`moved == 0`).
2. **`v0.9.12` Fixture**:
   - `config.json` omitting `max_iterations` with multi-provider profile array (`llm_providers`).
   - `load_settings_from_dir` populates `max_iterations = 200` while preserving all named provider profiles (`llm_providers[0].id == "p1"`).

## 2. Writer Quiescence & SQLite Coherent Encrypted Backup/Restore

- `scripts/upgrade-data-inventory.mjs`:
  - Detects all SQLite sidecars (`*.db-wal`, `*.db-shm`, `*.db-journal`) across `global-config` and `.agentz` project roots.
  - Reports `snapshot_consistent: sqliteSidecars.length === 0`.
- `scripts/upgrade-data-backup.mjs`:
  - Requires `--quiesced true`, matching `config.json` + `.secret_key`, and **rejects backup creation if any active SQLite WAL/SHM/journal sidecar exists** either before or after encryption (`Active SQLite sidecar files detected`).
  - Encrypts all objects with `scrypt-N32768-r8-p1` + `aes-256-gcm` authenticated encryption, verifies before/after SHA-256 inventory parity, and marks `snapshot_consistent: true`.
  - Restores only into a new, unoccupied directory outside source and bundle roots (`restore-preview`).

## 3. Verification Results

- `node scripts/upgrade-data-inventory.verify.mjs` -> `upgrade-data-inventory: synthetic config/key/project inventory passed`
- `node scripts/upgrade-data-backup.verify.mjs` -> `upgrade-data-backup: synthetic encrypted create/verify/restore-preview and failure cases passed`
- `cargo test -p agentz-desktop --lib commands::settings::secret_response_tests` -> `8 passed; 0 failed`.
