# AZ-067 local documentation/version candidate

README now names the current 0.9.7 source version and explicitly leaves installation/publication unverified. The 0.9.7 candidate notes separate implemented, verified, candidate-stage skipped, and external/planned scopes. The older release-candidate scope is marked as a historical snapshot because its task counts and some implementation claims no longer describe the current ledger.

`scripts/upgrade-version-check.mjs` compares package.json, Tauri config, desktop Cargo package and README versions, and requires the matching candidate release-notes and checklist files. CI runs the check in the frontend job. The local check printed `AgentZ 0.9.7: package, Cargo, Tauri, README and candidate docs agree` and exited zero. No CI job was dispatched; no installation, release, or cross-platform support was verified. AZ-067 remains pending until extension/platform coverage and unified documentation review.

Follow-up: the same CI check now compares the root npm lockfile's top-level and root-package versions with AgentZ 0.9.7. It separately checks that extension-host's own 0.3.1 package and lockfile agree, without conflating the sidecar component version with the desktop product version. The local check and `git diff --check` passed. CI was not dispatched; compatibility and platform acceptance remain open.
