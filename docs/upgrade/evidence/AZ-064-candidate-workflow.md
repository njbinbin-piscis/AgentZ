# AZ-064 artifact-only candidate workflow

2026-10-07. Isolated candidate preparation; existing `.github/workflows/release.yml` was read and left unchanged. No CI was dispatched and no download, desktop build, signing, upload, publication, or deployment was performed.

Added `candidate.yml`, a manual workflow_dispatch-only Windows x64 engineering candidate job with contents:read and checkout credentials disabled. It invokes the Tauri CLI directly, not the release-producing tauri-action. Installer output is explicitly unsigned/unverified, retained only as a workflow artifact for seven days. The pre-artifact check requires a nonempty NSIS installer; upload uses if-no-files-found:error. A generated candidate-status.json declares signature and runtime acceptance not_verified. No cross-platform support claim is made.

The existing Node fetch script downloads runtime archives without trusted checksum verification. This workflow does not call it. Preflight requires an independently reviewed `docs/upgrade/resource-integrity.json` (schemaVersion 1, windows-x64, signatureStatus not_verified, sha256 map), matching hashes for every staged runtime/preinstall file, a Windows node.exe, both codebuddy manifests, and repository lockfiles. That integrity manifest is currently absent, so the candidate pipeline intentionally fails closed before dependency installation/build. This package does not invent approved digests, claim downloaded runtime provenance, or remove the blocker. The existing Tauri resource mappings/build logic are unchanged; this job overrides the extension-host before-build step to use already-installed locked dependencies rather than npm install.

Verification actually run:

- `node scripts/upgrade-candidate-workflow.verify.mjs`: exit 0; static policy assertions and five mutations (push trigger, contents write, missing-file warning, release field, secrets field) correctly rejected.
- `node scripts/upgrade-candidate-workflow.verify.mjs --preflight`: exit 1 as expected, explicitly reporting missing approved resource-integrity.json. This is a blocked prerequisite, not a successful supply-chain or build check.

The verifier is a conservative source-policy guard, not a general YAML parser or execution sandbox. Native Windows packaging, installation/uninstallation, signed release, rollback, platform resource overlays, dependency provenance, and CI execution remain unverified. Upstream actions remain version-tag references, matching current repository convention; immutable action SHA pinning is a further supply-chain prerequisite before release acceptance. AZ-064 is preparing and this implementation alone does not satisfy release acceptance.

## Offline inspection manifest integration

Post-build workflow now invokes the existing `upgrade-release-preflight.mjs` with `--platform windows --arch x64 --channel candidate` and an absolute NSIS artifact directory derived from GITHUB_WORKSPACE. PowerShell captures `$LASTEXITCODE` immediately, writes stdout as UTF-8 `candidate-manifest.json`, and exits with the original nonzero code before upload. The manifest joins the installer and status file in the artifact upload. Its signature remains the offline inspector's unverified result; no status is patched to verified. Earlier multi-command preflight/dependency steps also explicitly exit after a failing native command.

Updated static verification actually ran with exit 0. Four additional negative mutations reject a hardcoded zero preflight status, removed failure exit, missing manifest upload path, and a non-candidate channel, in addition to the original five policy mutations. This only verifies workflow source integration; the offline inspector itself was not executed by this agent in this follow-up, and CI/build remain undispatched. The initial trusted-resource-manifest blocker remains in place.
