# AgentZ release procedure

**English** | [简体中文](RELEASE.zh-CN.md)

Current source version: **1.0.58**. See the [1.0.0 release decision](docs/upgrade/release-decision-1.0.0.md), [historical support baseline](docs/upgrade/evidence/AZ-005-support-matrix.md), [current upgrade progress](docs/upgrade/progress.md), [1.0.58 release notes](docs/upgrade/release-notes-1.0.58.md), and [1.0.58 checklist](docs/upgrade/release-checklist-1.0.58.md) for the recorded scope and checks.

## Build and distribution boundary

`.github/workflows/release.yml` runs on a pushed `v*` tag or manual dispatch. Its `tauri-action` jobs create or update a **published** GitHub Release (`releaseDraft: false`) and upload bundles. Every bundle job depends on `release-readiness`, which runs `node scripts/upgrade-version-check.mjs`, `node scripts/upgrade-plan.mjs validate`, and `node scripts/upgrade-release-gate.mjs` before bundling. The gate checks the source version, the M0–M8 and MB gate records, the accepted AZ-069 decision, and that the ref is the matching `refs/tags/v<version>` tag. This wiring passed local workflow inspection and script tests; no publishing workflow rehearsal or signed installer acceptance has been recorded. The owner authorized version-specific gate waivers for 0.9.12 through 0.9.19; see their release checklists. Triggering the workflow is a distribution action, not a local verification step.

The workflow builds Linux x86_64 and aarch64 `.deb` and AppImage bundles, Windows x86_64 and aarch64 NSIS `.exe` installers, and macOS aarch64 and Universal (`--target universal-apple-darwin`) `.dmg` bundles. Each bundle job first runs `npm --prefix extension-host run fetch-node`, which downloads the pinned Node.js runtime into `src-tauri/resources/node/` (bundled as an app resource for the extension host and remote targets). `tauri.conf.json` has `targets: "all"`, but `src-tauri/tauri.windows.conf.json` overrides Windows to `targets: ["nsis"]`, so no `.msi` is built; Windows artifacts are `bundle/nsis/*.exe`. A bundle target or CI build is not evidence of install, launch, upgrade, uninstall, credential access, or rollback on that platform.

The current candidate has no recorded signed Windows installer, notarized macOS bundle, clean-machine install/uninstall, or native macOS/Linux core journey. The in-app trusted updater and its recovery path are not accepted. See the [upgrade progress](docs/upgrade/progress.md) and AZ-064/AZ-065/AZ-077/AZ-078 evidence before changing support claims.

## Prepare a candidate

1. Update `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, the README source version (`README.md` and `README.zh-CN.md`), this file, the changelog, and matching release notes/checklist. Keep the extension-host package and lockfile aligned with each other.
2. Run `node scripts/upgrade-version-check.mjs` and the relevant build/tests. This checks version labels, required documents, and local links in the README, RELEASE and CONTRIBUTING files; it does not certify the installer.
3. Freeze source and artifact hashes. Complete the release preflight, signing/trust checks, native installation and recovery journeys, and explicit gate decision in `docs/upgrade/`.
4. Only after an authorized release decision, push a version tag or manually dispatch the publishing workflow. Record the resulting assets and platform receipts.

## Signing and updater prerequisites

macOS notarization requires an Apple Developer signing identity and notarization credentials. Windows signing requires an appropriate trusted certificate or signing service. The workflow currently has no accepted signing identity for this candidate. Keep secrets in CI's protected store; do not put private keys or passwords in this repository.

An updater needs a pinned trusted public key, authenticated update manifest and channel policy, compatibility checks, and interrupted-download/install recovery. Implement and verify those with AZ-065 before enabling automatic updates. Do not infer updater readiness from the presence of Tauri bundle files.

## Diagnostics and security scope

Diagnostic export and secret storage have their own evidence in AZ-031/AZ-061. The renderer CSP (`app.security.csp`, plus `devCsp` for development) is configured in `src-tauri/tauri.conf.json`; any change to it needs desktop coverage for Monaco, local bridges, resources, and configured endpoints. The config and source are the authority for current behavior, not historical release notes.
