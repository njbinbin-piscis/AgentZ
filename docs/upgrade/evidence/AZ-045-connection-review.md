# AZ-045 connection review

## Whole-repository follow-up: three actionable remaining boundaries

Read-only on 2026-10-07 after remote file list/action migration and Git discovery binding. No builds or runtime checks were performed by this review agent.

1. **P1 — Agent prompt preparation remains unbound.** `commands/chat_turn.rs::expand_file_refs_remote` resolves the workspace, then calls `remote_read_text`, which uses unbound `remote::call`. `project_rules_context_remote` similarly resolves the root and issues multiple unbound readDir/readFile calls. These helpers are actively called by chat_turn; neither that caller nor these helpers is wrapped in `with_session`. Reproduce A workspace resolution, replacement with B, then a matching-path read: B file text or rules can enter the model context for A. Bind once for the complete preparation operation and reject replacement requests. This concerns read provenance, not only mutation replay.
2. **P1 — extension deployment captures target/server separately.** `commands/remote.rs::remote_sync_extensions` awaits `mgr.target()` then separately awaits `mgr.server()`. A lifecycle switch between them can associate A transport with B's home. `remote::sync_extensions` subsequently runs mkdir/tar on the captured A target using that mixed directory. Capture target and deployed server as one generation-checked snapshot, then use only that snapshot for the deployment. Test a lifecycle replacement between the two former captures; reject rather than combining the metadata. These functions have active call sites; this excludes unused path-only read/write helpers.
3. **P2 — Git diff is not bound as one composite operation.** `commands/ide.rs::ide_git_diff` runs show, binds for reading current text, then runs diff; each Git helper can independently bind the same authority's latest generation. A disconnect/reconnect can combine results from different sessions, and intermediate errors are swallowed into empty values. Wrap the entire remote diff in one retained session, as discard already does. Test replacement after show and require a failed operation rather than a mixed successful DiffResult. Single Git mutations already use a generation-bound request and are not reported again.

`refresh_git_repos` now uses a bound session; its earlier gap is resolved. File list/action now bind and validate rename destination authority; their earlier gap is resolved. LSP startup passes a captured RemoteTarget rather than using a later current-manager request, so it is not classified as the same resolve/request defect; actual session lifetime and replacement remain covered by the separate lifecycle candidate and pending native acceptance.

## Follow-up review after RemoteSession implementation

Read-only follow-up on 2026-10-07 while the primary agent's combined build was running; no additional build was started.

**Resolved in the reviewed implementation:** `bind_authority` captures generation before waiting and rejects a replacement after waiting. `target_for_generation` captures the target and checks the same generation again; `remote::bind` verifies the captured target authority. Bound requests retain that generation and the stdin mutex prevents pipe replacement during their write. The terminal constructs argv from the captured `session.target`, so a later broker switch cannot silently choose another host for that terminal. These are code-review conclusions, not process-level test results.

**Resolved in the reviewed implementation:** watch registration, cancellation, and rearming use `watch_operations` serialization; rearming verifies that the token/path still exists while holding that operation lock. This closes the cancellation/old-snapshot resurrection ordering described below. Watch requests also retain their authority-bound generation.

**Resolved in the reviewed implementation:** start now returns an atomic `connected` snapshot instead of constant `running: true`. Connected is published before the stdout reader starts; EOF subsequently sets it false, with no later start-path store overwriting it. The returned snapshot can still become stale immediately after reading, which is an ordinary asynchronous status boundary requiring UI exit-event handling. A real immediate-exit process test remains necessary.

**Remaining P1 caller gap:** `commands/remote.rs::remote_fs_action` still resolves the URI into a String and then invokes `state.ext_host.request` without a session. Its create/delete/rename requests can run on a replacement authority. Rename also resolves its destination separately, permitting cross-authority source/destination association. `remote_fs_list` has the same issue for reads. `remote/mod.rs::refresh_git_repos` also retains the old resolve/call split outside a task-local bound session. Migration of IDE and Agent tool entry points does not cover these remaining callers. Reproduce source-A resolution followed by switching to B before final request; require rejection and no B frame. Rename must additionally reject a destination URI from another authority.

**Coverage still missing:** blocked/partial stdin writes, early child EOF before start publication, response-versus-restart through actual pipes, and cancel-versus-rearm through the server protocol. A 30-second write timeout can occur after partial or complete delivery, so callers must not infer that retrying a mutation is safe. No replay was introduced by the reviewed manager.

The original findings below remain as history; resolved labels above describe their current status.

Read-only review of `src-tauri/src/commands/ext_host.rs`, `remote/mod.rs`, and `tools/remote_fs.rs` on 2026-10-07. No cargo build, GUI, or real SSH was started by the reviewing agent. Findings refer to the working tree at review time; they require verification after the primary agent's fixes.

## P1: resolved authority is not bound to execution

`remote::resolve` waits for an authority and returns only a remote path string. `remote::call` subsequently uses the manager's current connection through `request`. Reproduction order: resolve an A workspace path; switch the manager to B; issue the file write or shell request with the resolved path. The request captures B's generation and can execute on B. Rejecting writes captured before a generation change does not cover this ordering. `wait_for_authority` also reads target and pipe readiness separately, allowing a target snapshot from A to be combined with readiness from B.

Bind authority and generation to resolution and retain that binding through execution. Add a controlled A-resolution / B-switch / write test proving that B receives no frame. Broaden coverage to shell, terminal, Git mutations, and LSP callers that resolve then call separately. Do not classify this as an automatic replay defect: it is a missing origin connection binding.

## P2: rearmed watch can outlive cancellation

`rearm_watches` clones registrations, then awaits requests. A cancellation can remove a token and finish `fs.unwatch` before the recovery loop sends `fs.watch` from its old snapshot. The server accepts that later watch, resurrecting an unwanted registration. Checking the map immediately before sending reduces but does not eliminate the check/send race. Serialize watch registration, cancellation, and rearming, or use a versioned protocol that rejects obsolete registration operations. Test the snapshot / unwatch / rearm ordering explicitly.

## P2: start response can claim running after immediate exit

The stdout reader can observe EOF and set `connected` false before `ext_host_start` returns its constant `running: true`. Status calls subsequently report false. Use a controlled child that exits immediately to verify that initial responses and emitted exit events cannot leave the UI showing a healthy connection.

## Coverage assessment

The four current Rust generation/readiness tests cover stale-write rejection before touching a pipe, default disconnected readiness, stale EOF preserving a pending sender, and generation mutex exclusion. They are helper tests without a real child stdin/stdout exchange. They do not yet establish blocked write behavior, partial-frame timeout recovery, response-versus-restart ordering, authority switching between resolve and execution, watch cancellation during rearming, or immediate child exit during initialization.

The stdin mutex prevents replacement of the actual child pipe during a write, and no automatic retry of a rejected mutating request was found in the reviewed manager. A timed-out write can have emitted a partial frame; its error must not imply that remote execution was impossible. Real SSH network loss, changed known-host key rejection, and reconnection remain unverified by this review.
