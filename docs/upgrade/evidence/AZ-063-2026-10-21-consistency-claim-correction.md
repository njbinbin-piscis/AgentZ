# AZ-063 backup consistency claim correction

The offline inventory and encrypted backup previously emitted `snapshot_consistent: true` when no SQLite sidecar was seen and the file inventories matched. Those checks cannot prove that the AgentZ host stopped writers or that the copied SQLite files represent one coherent point in time. The inventory and backup now emit `snapshot_consistent: false`; backup creation also labels its mode `operator_asserted_offline`. The encrypted bundle retains its authenticated file inventory and restore-preview behavior.

This is a claim correction, not an implementation of host-coordinated quiescence, SQLite online backup, or native restore acceptance. Historical 1.0 engineering decisions and Gate records remain unchanged. AZ-063's broader next action remains in force.

Verification on 2026-10-21: `node scripts/upgrade-data-inventory.verify.mjs`, `node scripts/upgrade-data-backup.verify.mjs`, `node scripts/upgrade-plan.mjs validate`, and `git diff --check` all passed. The `reeval` command updated AZ-063's evidence and next action without changing task status or Gate records.

Both script fixtures also passed under AgentZ's bundled Node 22.11.0. That runtime does not provide `node:sqlite`, so a Node built-in online backup cannot be assumed for the shipped host. A coherent SQLite capture needs an explicit host/Rust integration or a verified runtime dependency.
