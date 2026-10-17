# RC source snapshot inventory

Captured 2026-10-07 09:22:19 UTC / 17:22:19 Asia/Shanghai. This is a read-only source inventory, not release approval or proof of the source that produced the existing 0.9.6 NSIS installer.

## Executed evidence

- `node --check scripts/upgrade-source-snapshot.mjs`: exit 0.
- Summary and full JSON inventory: exit 0; full JSON contained zero invalid SHA256 values for present source entries.
- HEAD: `a787b6dafba84489670d944e8339611ff0fcd563`.
- Observed package version: `0.9.7`; capture was stable while reading HEAD, status and source bytes.
- Git status with `--untracked-files=all`: 417 entries; source policy included 268 dirty tracked / nonignored untracked source entries. This expanded count is not comparable directly with a directory-collapsed 117-entry status. Status is data and does not identify ownership or approval.
- Excluded counts: 115 generated/runtime directory entries, 34 outside the build-source allowlist. Excluded paths and contents were not disclosed.
- Script SHA256: `0797b032dcf7ae567af806ba23ae44c1f64c1b3edb84a8bb9844a0c592e90ce0`.
- SHA256 of the captured entries serialized by PowerShell `ConvertTo-Json -Depth 8 -Compress`: `f0918b4006edbf204fd1dd0b2eb6e72f74f65670349b70c8f92b51fbb2d9be2f`. This identifies that serialization, not an archive of the source bytes.

The CLI emits JSON to stdout by default and a compact report with `--format summary`. It does not write an inventory, read excluded secrets, stage files, commit, archive, build or invoke install/signing tools. Git optional locks are disabled. Known credential/private-key paths, runtime data, artifacts, binaries, oversized sources and symlinks are excluded before reading source contents. The allowlist is not a general secret scanner; do not add confidential values to source files.

## Required local Cargo source coverage

| Cargo member / local path | Nonignored source paths | Dirty paths | Untracked paths |
| --- | ---: | ---: | ---: |
| crates/agentz-host | 2 | 0 | 0 |
| crates/agentz-task-state | 11 | 11 | 11 |
| src-tauri | 125 | 63 | 7 |
| vendor/piscis-engine/piscis-core | 11 | 11 | 11 |
| vendor/piscis-engine/piscis-kernel | 90 | 90 | 90 |
| vendor/piscis-engine/piscis-ide-tools | 10 | 10 | 10 |

All six roots have a manifest and Rust source in the nonignored inventory. **122 source paths in task-state and the three vendored crates are untracked.** A checkout of HEAD alone cannot reproduce this current source tree. The script's `covered` field proves only manifest/Rust presence under root Cargo declarations; it does not prove full dependency/resource closure. The check is a narrow declaration parser, not Cargo resolution.

## 0.9.7 candidate review procedure

1. Coordinate a short source freeze before the candidate build, then regenerate the inventory. Record exact HEAD, current source hashes, version agreement across package/lock/Cargo/Tauri manifests and build start/end times.
2. Review ownership of each dirty source change. Preserve user changes; a status marker does not authorize staging or committing them.
3. Review the 122 untracked required local sources explicitly. Check manifest dependencies, build scripts, `include_str!` / `include_bytes!` inputs and necessary non-Rust resources. Inspect excluded/ignored build inputs separately through their approved resource-integrity workflow; never widen this inventory to runtime credentials or private keys.
4. After scope approval, establish a reviewed source baseline that retains the required local crates and lockfiles. This script does not create that baseline or automatically submit user changes. Hashes alone cannot reconstruct dirty file bytes.
5. Build the 0.9.7 candidate from that exact reviewed baseline and correlate its source inventory with build logs and installer SHA256. A later inventory must not be presented retroactively as the source provenance of 0.9.6.

No build, install, GUI acceptance, source commit, signing verification or release was executed in this package. Existing installer reproducibility remains unverified.
