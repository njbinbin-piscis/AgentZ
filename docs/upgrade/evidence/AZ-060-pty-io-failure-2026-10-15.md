# AZ-060 PTY I/O acquisition cleanup fixture (2026-10-15)

Reader cloning and writer acquisition now use one post-spawn cleanup path. A focused native test injects each acquisition failure with a real contained PTY shell and verifies that the shell has exited and the Windows job has no active processes. This covers both branches that previously could leak a shell when setup failed. It does not emulate an operating-system API failure itself.

`cargo test --manifest-path src-tauri/Cargo.toml terminal_tree_tests::reader_and_writer_acquisition_failures_reap_the_shell -- --nocapture` passed (1 test). The concurrent command fixture, resource census, and two-hour soak against a frozen AZ-057 budget remain outstanding. AZ-060 stays pending.

## Duplicate registration follow-up

The late duplicate check now uses a registry insertion helper that returns the rejected PTY to its caller; the caller tears down its process tree after releasing the registry lock. A real two-shell fixture registers one session, rejects a second with the same ID, verifies the first still runs and the rejected shell/job are empty, then closes the first. The complete `terminal_tree_tests` suite passed 6/6 on Windows. The test exercises late registration; it does not drive two concurrent Tauri command invocations. Resource census and two-hour soak remain outstanding.
