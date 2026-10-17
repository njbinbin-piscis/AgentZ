# AZ-053 local RobotZ dependency check (2026-10-16)

AgentZ remains pinned to `robotz-browser` `v0.1.13` at `cdff2c947f812648050a550b8e9d8a887bf01e01` in `src-tauri/Cargo.toml` and `Cargo.lock`. The separate `C:/Projects/RobotZ` working tree is based on the same commit but has uncommitted browser snapshot and tool changes. AgentZ does not consume those working-tree bytes.

Read-only validation of that **unshipped working tree**: `cargo test --manifest-path C:/Projects/RobotZ/Cargo.toml -p robotz-browser --offline --locked generation_tests -- --nocapture` passed 4/4 on Windows. The tests cover stale references after navigation, capture interrupted by navigation, fresh references, and one-time reporting of a navigation-released lock. This result does not validate AgentZ's pinned dependency or prove user takeover stops delayed browser actions. No RobotZ source, dependency pin, or external release was changed in this check.

Next dependency step: integrate a reviewed immutable RobotZ revision that contains these fixes, then run stale-reference and takeover cases through AgentZ's actual pinned browser path. AZ-053 remains pending.
