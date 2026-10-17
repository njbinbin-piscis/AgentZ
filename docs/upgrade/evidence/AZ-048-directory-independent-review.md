# AZ048 projectless directory independent review

2026-10-10/11. Reviewer: Codex current_source_review. Read-only module/export inspection; no source, ledger or existing pinned evidence edits. Root reports offline locked domain session24352 one actual filesystem fixture passed. Reviewer did not run Cargo, native IPC or GUI. Host integration is absent.

Observed SHA256:
- crates/agentz-task-state/src/projectless.rs: 3df63fb04bdbf8cb797160440815ac0c6bd7ddab05e00cc8cd191353c16f33f2
- crates/agentz-task-state/src/lib.rs: ba73a74324b07960b853780e797af76d34929047c03f9008505385e0fb2acc73

checked_id requires 36 bytes with fixed hyphens and lowercase hexadecimal, preventing path separators/traversal, though it accepts any UUID-shaped bits rather than enforcing a UUID version. create requires an existing ordinary dedicated root, canonicalizes it, uses nonrecursive create_dir so existing task paths are never adopted, and writes a create_new ownership marker with exact task ID plus sync_all. It does not initialize Git or delete anything. Marker creation/write/sync/open validation failures can leave a partial directory/marker; retry create then fails rather than silently adopting it. Errors are not rollback claims. No root initialization, ledger registration or transactional durable task identity is provided.

open verifies current root/task directory metadata, rejects directory symlink and Windows reparse attributes, canonicalizes the child and requires its parent equal root. Marker must be a regular nonsymlink file with length36 and exact ID bytes. A removed/corrupt marker fails instead of reconstructing state. The actual filesystem fixture proves artifact text survives reopen, Git absence, duplicate-create refusal, traversal refusal and corrupt-marker refusal with artifacts retained. It does not exercise reparse/hardlink/races or interrupted creation.

## Required marker check

At this fingerprint marker validation lacks the Windows FILE_ATTRIBUTE_REPARSE_POINT check used by ordinary_directory. Windows non-symlink reparse files may appear regular and pass is_symlink=false. Add matching marker reparse rejection before reading; report sent to root. The current module should not be frozen as enforcing ordinary marker identity until this is addressed and reviewed. No source modification by reviewer.

## Trust and race limits

Caller must supply a dedicated application-owned root and authoritative task ID; returned path/marker is not permission. This public API cannot distinguish a user project root from a trusted root. Ancestor components are not independently checked. Metadata→canonicalization→create/open→read are separate path operations with TOCTOU; marker read is not same-handle/no-follow, length check does not bound a replaced file's allocation, and marker hardlinks are not rejected. Equal marker contents establish current naming/content consistency, not durable filesystem object identity, unforgeable ownership or hostile concurrent-root safety. Cooperative exclusive root access is required until stronger native containment/lease handling is implemented. Returned paths can be replaced after validation, so callers need fresh authorization for later IO. Directory/parent sync and crash durability are not proven by marker sync alone.

lib.rs currently contains a trailing pub mod projectless export with no added global initializer or call at that export. Exporting this module alone does not create directories or change startup permission. Exact historical reviewed dirty bytes are missing; this inspection cannot prove the entire lib diff is only that export or all previous functions unchanged. Seven upstream consumers may receive a limited export/current-fingerprint refresh preserving prior histories, conditioned on root independently validating current source and retaining its earlier scopes; this review is not whole-lib historical equivalence. New module itself remains subject to the marker finding above. Full AZ048 task acceptance, projectless host persistence/routing, native authorization and GUI remain open.
