# AZ-060 local resource cleanup candidate (2026-10-14)

Resource inventory found a PTY create failure path that spawned a shell before cloning the reader/taking the writer; an error returned without explicit process-tree teardown. Concurrent creates with the same terminal ID could also replace a registered session, bypassing its normal destroy path. `src-tauri/src/commands/ide.rs` now rejects duplicate IDs before spawn and again under the registry lock before insertion, and calls the existing tree termination path on setup failures or a late duplicate. An existing session is preserved on the duplicate path.

The CodeZ workspace keeps inactive Monaco models across tab switches. Its tab-close effect already disposes models removed from the tab set. `src/workspaces/codez/index.tsx` now also disposes workspace-owned models on full pane unmount.

Local checks: `cargo check -p agentz-host --offline --locked` passed (dependency only); `cargo check -p agentz-desktop --offline --locked` passed; `npm run typecheck` passed; targeted ESLint completed with 0 errors and 8 existing warnings in the edited CodeZ file; `git diff --check` passed. No PTY failure injection or two-hour soak was run. AZ-057's resource baseline and the close/restart census remain outstanding; AZ-060 stays pending for unified acceptance.

## Follow-up, 2026-10-14

Post-spawn setup errors now preserve any process-tree teardown error in the returned message. If job containment fails, the uncontained shell is killed and reaped where possible, and kill failure is surfaced. A real Windows PTY fixture injects a setup error after spawning a contained 30-second shell, then checks that teardown reaps the shell and empties the job. `cargo test --manifest-path src-tauri/Cargo.toml terminal_tree_tests -- --nocapture` passed all 4 tests. This does not exercise a forced PTY reader/writer API failure or the two-hour resource soak; AZ-060 remains pending.

Current-source regression, 2026-10-16: `cargo test --manifest-path src-tauri/Cargo.toml terminal_tree_tests --offline --locked -- --nocapture` passed 7/7 Windows tests. The current fixture now injects reader and writer acquisition failure, rejects duplicate registration while preserving the first live shell, races two registrations to keep one owned shell, and exercises descendant termination. This is the registry/PTY-domain fixture, not two concurrent Tauri command invocations or a measured two-hour close/restart soak. AZ-057 budget and unified acceptance remain outstanding.

The matching `graph_index::resource_tests` filter passed 6/6 current-source tests with `--offline --locked`, covering queued/in-progress build close, cached status release and late watcher/incremental callbacks. These are controlled worker fixtures; a real repository graph-build close and resource slope remain unmeasured. An initial `graph_index::tests` filter selected zero tests and is not counted as verification.
