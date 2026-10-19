# AZ-064: 1.0.0 release workflow guard correction

The 1.0.0 sign-off recorded a passing local `upgrade-release-gate.mjs` check, but the publishing workflow did not invoke it. All six bundle jobs could therefore reach `tauri-action` on a tag without that check.

Added a `release-readiness` job to `.github/workflows/release.yml`. It checks version synchronization, ledger structure and the release gate. Every Linux, Windows and macOS bundle job now depends on it. A manual dispatch from a branch fails the gate because the script requires the matching version tag.

Verification: `node --test scripts/upgrade-release-gate.test.mjs` passed 3/3, including a workflow integration test covering all six jobs. Invoking the gate with a local `refs/tags/v1.0.0` environment passed. `node scripts/upgrade-plan.mjs validate` and `node scripts/upgrade-version-check.mjs` passed. The guard has not been observed running in GitHub Actions.

AZ-064 remains deferred under ADR-006: there are no signing/notarization identities, signed artifact receipts, clean-machine installation results, cross-platform desktop sessions, or rollback evidence. This change does not authorize publishing.
