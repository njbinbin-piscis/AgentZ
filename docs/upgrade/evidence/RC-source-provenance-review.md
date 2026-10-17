# Candidate source provenance / submission scope review

2026-10-07, read-only local metadata review. No stage, commit, push, source mutation or build. The141 untracked allowlisted inputs identified in the closure follow-up belong to task-state, three engine crates and secret-service; this count excludes separate licenses/provenance support files. Inventory presence is not approval of copyright, origin or every change's ownership.

## Concrete directories for source review

| Directory | Present provenance / license metadata | Submission review disposition |
| --- | --- | --- |
| crates/agentz-task-state | AgentZ-local manifest v0.1.0 declares Apache-2.0; root LICENSE is Apache2.0; AZ-017 and task/environment/approval evidence describes local implementation | Review its11 source/manifest inputs and ownership as AgentZ-local changes; retain root license. These records describe engineering history, not proof of author ownership. |
| vendor/piscis-engine/piscis-core | Baseline note states upstream commit527e55496caf4437e4265e02869f0a8c764549f6/tagv0.8.68; local manifest version0.8.63, no package license field | Necessary local path override; include source review, but upstream license/permission and baseline correspondence need explicit evidence before public source approval. |
| vendor/piscis-engine/piscis-kernel | Same baseline; documented AZ029/079/034/081/082/030/083 changes plus platform adapters; local manifest0.8.63, no license field | Review91 inputs including koi_execute_todo.tmpl, required workspace metadata and complete final delta. Do not assume tagged baseline package version must equal tag, or assume reconstruction script equals current bytes. |
| vendor/piscis-engine/piscis-ide-tools | Same baseline; manifest0.1.0 declares Apache-2.0 | Review10 inputs with engine workspace metadata; license declaration exists but preserve actual upstream license/notice evidence. |
| vendor/secret-service | Manifest5.1.0 declares MIT OR Apache-2.0, upstream repository; both LICENSE-MIT and LICENSE-APACHE present. AGENTZ-BASELINE records archiveSHA and upstream commit7e8f960c767d34d961d82913a2eb0ce3bce09701, matching local.cargo_vcs_info.json | Review18 captured inputs plus licenses/baseline/patch descriptions. Needed for Linux-conditioned local dependency resolution; Windows candidate build does not prove Linux runtime acceptance. |

Engine Cargo manifests explicitly use workspace=".."; therefore vendor/piscis-engine/Cargo.toml and its member/layout requirements belong in the source review too. The workspace lists piscis-cli in addition to the three product overrides; ensure a reviewed checkout retains valid workspace members, rather than submitting only the three directories blindly.

## Evidence that exists, and remaining gaps

- secret-service patch file `docs/upgrade/patches/AZ-078-secret-service-5.1.0.patch` was independently hashed in this review: `C179551741B95825AB778338DC8E1AE135CAAA435B8E64FE6BDF9BA27BF15F5E`, matching recorded evidence. Existing notes describe no-prompt, entropy-failure handling and workspace isolation. This review did not retrieve/reverify the original registry archive or apply/reverse the patch.
- Engine baseline note explicitly states candidate reconstruction omits AppContainer, platform adapters and subsequent changes. It is useful provenance context, but does not establish a complete byte-for-byte final patch chain. A final reviewed diff against the immutable baseline remains needed.
- No LICENSE/NOTICE file was found under vendor/piscis-engine using a file-name inventory. core/kernel manifests lack license declarations. Root AgentZ Apache license cannot by itself establish upstream engine licensing. Confirm upstream rights/notice and preserve them before public source distribution; do not infer permission solely from shared repository ownership.
- THIRD_PARTY_NOTICES.md currently describes CodeBuddy preinstall and has no piscis-engine/secret-service entry. Consolidated public notices for these vendored inputs remain a concrete release documentation gap; licenses in secret-service exist locally.
- Source141 count is not all support files: review baseline notes, licenses, engine workspace manifest, required workspace members and patch evidence alongside it. Do not submit `.cargo-ok`, local caches, unrelated changes or private runtime data merely because directory status is untracked.

The local0.9.7 unsigned candidate has its own pre/post source records and installer evidence. Any events/plan work added after that frozen build is **post-candidate** and is not claimed to be present in the0.9.7 package. This document neither approves all dirty changes nor changes the candidate's uninstalled/unsigned/unpublished status.

## Immutable local engine baseline follow-up

Read-only Git object inspection in `C:/Projects/piscis-engine` confirmed commit `527e55496caf4437e4265e02869f0a8c764549f6` exists. Its recursive tree has no LICENSE/NOTICE file. A bounded baseline Git grep over README, root/package manifests and Rust source paths found only the ide-tools Apache-2.0 manifest declaration; core/kernel/CLI manifests have no license declaration. Their baseline versions are respectively0.8.63/0.8.63/0.8.47, matching the local package versions: tagv0.8.68 and package-version differences are therefore already present in the immutable baseline, not evidence by themselves of an incorrect vendor copy.

This establishes the metadata gap exists upstream at the pinned commit, rather than merely being a missing copied license. **Upstream license/permission for core, kernel and CLI still needs the user's ownership/permission decision or authoritative upstream evidence.** No Apache declaration was invented for these packages. The existence of the local upstream repository does not supply that decision.

The immutable workspace lists all four engine members and workspace path dependencies, including piscis-cli. The current vendor workspace manifest and all11 CLI manifest/source/test paths are untracked; CLI's explicit binary source `src/bin/openpiscis-headless.rs` exists (6874bytes). A coherent submission scope must review the engine workspace manifest plus CLI member paths alongside the three product overrides. This adds workspace support inputs beyond the earlier141 count; current inventory-required_roots is still not a complete nested-workspace-members resolver. No external repository files were modified, downloaded or built.
