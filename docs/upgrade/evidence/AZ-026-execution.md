# AZ026 complete vendor freeze and compatibility evidence

2026-10-09. Local candidate preparation only. No tag, public package, commit/push, release or deployment was created. AZ026 remains preparing; original upstream acceptance/M1 requirements remain pending.

The previous eight-patch utility is a partial chain. The new separate scripts/upgrade-kernel-freeze.mjs archives only immutable upstream commit 527e55496caf4437e4265e02869f0a8c764549f6 from the sibling repository; its uncommitted working files are never read. It snapshots the actual product vendor, creates a complete binary-capable overlay in a fresh independent temporary repository, checks/apply/reverses it against the fixed archive, compares complete file sets and SHA256, applies again, and verifies a relocated portable archive. Global Git settings are unchanged; temporary repositories explicitly disable checkout conversion and hooks.

Initial runs failed closed because the complete overlay exceeded the default child-process output buffer. The utility now has a bounded 64MiB buffer and phase-only error output, never raw patch content. Failed snapshots were retained, not used as releases. Complete byte comparisons then passed. A subsequent dependency audit found the Linux-only ../../secret-service local dependency outside the engine tree. The final bundle preserves piscis-engine and adjacent secret-service directories, checks all 29 companion files and stability, and does not claim Windows execution validates Linux service behavior.

Final freeze session 96478 exited 0. Engine source: 132 files; companion: 29 files. Forward application, reverse restoration, second application, relocated bytes and live source/integration stability all passed. The overlay includes existing vendor changes and line-ending differences; these counts are not counts of newly implemented features. Every overlay has not thereby received a new independent review. Tar bytes identify this particular artifact; arbitrary future regenerations are not promised byte-identical tar headers.

Artifacts retained under target/release/kernel-contract-0.9.7-20261009-06e56229; copied artifact hashes verified:

- candidate.tar: 2703360 bytes; aee1264293cac757c7ce778a00bcc486b9136cd9ec0171c1b0ee037ede85f23d.
- full-vendor.patch: 3597717 bytes; df1bed1772b62d0993a75d27d9151df69b7c392876d2dab68a6fd48e3a5df69b.
- baseline.tar: 2222080 bytes; 2ccfc6dd1bbb62c5f01455824499f2a1517bd24413c10f51f1ecd173ffd8ca96.
- manifest.json: 68803 bytes; 1a26117c22a92ea62554cabcbda38ea576dcf544fb2c9e381feac82a4ded4406.

AZ-026-full-source-manifest.json preserves complete baseline/current/companion/source/integration fingerprints; AZ-026-artifacts.json records locations/hashes and release_ready=false. Source and artifact review remain pending, not silently inferred from hash equality.

Actual fixed-product compatibility:

- Session 5365: cargo metadata --offline --locked --format-version 1 exited 0. All seven local packages resolve inside AgentZ, no sibling override. Three engine manifest versions are core/kernel 0.8.63 and ide-tools 0.1.0, distinct from upstream tag v0.8.68; secret-service is 5.1.0. Sanitized local resolution is in AZ-026-cargo-resolution.json. Cargo.lock and dependency declarations were not rewritten merely to force matching version labels.
- Session 94054: cargo test -p agentz-host --offline --locked --test kernel_contract -- --test-threads=1 exited 0; 3 passed, 0 failed, 0 ignored, 0.01s; compilation 4m41s. Tests import the public recovery and retry APIs through the product dependency declarations. They verify preservation of tool content/image with passive unknown recovery, cancellation distinct from omission/no replay, and permanent authentication/quota failures without retry/fallback.
- Session 17811: cargo clippy -p agentz-host --offline --locked --test kernel_contract -- -D warnings exited 0, 2m05s.
- node --check for the new freeze utility and limited git diff --check exited 0. Earlier current-kernel full389/1ignored and Windows runner6 results are referenced by AZ022, not rerun or counted again here.

Independent final script/source-bundle review is still required. Codex subagents reached their usage limit after delivering AZ025; this is an execution resource limitation, not a product decision or authorization request. Parent completed these local implementation/byte/integration checks. GUI, real provider side effects, full relocated recompilation, Linux/macOS runtime, fresh-machine installation, signature/license distribution clearance and rollback remain unverified. The older unsigned 0.9.7 installer does not contain this later source snapshot.

Subsequent review update: after quota reset, remote_tests independently reviewed the final utility/public contract tests and actually matched all132 engine,29 companion,6 integration source hashes plus4 retained artifact sizes/hashes. AZ-026-independent-review.md records no blocking defect within this reconstruction/compatibility scope and preserves the semantic/security/license/platform limitations. Limited implementation_review is now recorded; formal task/Gates/release-ready remain unchanged. A new local installer build is separately tracked under AZ064, never inferred from this source archive.
