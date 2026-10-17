# AZ-020 frontend project scope service candidate

2026-10-08. Exclusive new `src/services/tauri/projectScope.ts` and its test file. Command contracts coordinated directly with the host scope command agent:

- `project_scope_pick` has no renderer arguments and returns `{handle, project_dir}` or null.
- `project_scope_restore` accepts `{source}` for host workspace/free roots, or `{source: 'workz_project', path}` for a host-listed saved project identity. The host validates membership; the service cannot authorize arbitrary paths.
- `project_scope_revoke` accepts only `{handle}`. No renderer window/generation parameter is exposed.

Exported API: `pickProjectScope`, `restoreProjectScope`, `revokeProjectScope`, `getProjectScope` and readonly `ProjectScopeDTO`. Handles are retained only in a module-local map keyed by the exact host-returned canonical project directory. Returned DTOs are frozen. The service does not persist, log or insert scopes into prompts. Lookup is a convenience, not authority; host commands must still validate every handle. Invalid restore sources/renderer path combinations and malformed host replies reject without mint fallback.

Revocation removes the local cached handle before awaiting the host, so a failed revoke does not leave a cached token in use. Revoking an older handle does not erase a newer handle for the same project. Native picker cancellation preserves existing scopes. Callers own open/close lifecycle and must revoke superseded scopes; this service does not coordinate concurrent UI navigation or confer authority on other frontend APIs.

Actually run: `npx vitest run src/services/tauri/projectScope.test.ts`, exit 0, four tests passed. Mock invoke fixtures verify exact command payloads, no picker path/window/generation, authorized restore-source routing, missing/invalid arguments, malformed replies, host authorization error propagation, cancellation, immutable storage and old/new revocation handling. Scoped Prettier and diff check also ran successfully. No native picker GUI, Rust command execution, App/CodeZ integration, host build or real permission acceptance was performed by this agent.

## Observable scope lifecycle follow-up

Added `subscribeProjectScopes(listener)` and `useProjectScope(projectDir)` using React `useSyncExternalStore`. Acquisition/replacement and local revocation notify subscribers even if the active directory prop remains unchanged. The snapshot accessor returns the same frozen DTO object until a actual handle change, or stable null. Repeated host replies for the same project/handle reuse the original DTO and do not notify; picker cancellation and revoking an older, superseded handle likewise produce no misleading change notification. Successful unsubscription stops notification. The server snapshot is null and no persisted token is hydrated.

Actual follow-up Vitest run: five tests passed, exit 0 (session 31083; 92 ms test time). New subscription fixture records the snapshot inside the listener after acquire/replacement/revoke, verifies duplicate no-op stability, cancellation and older-handle no-op behavior, observes immediate local revoke even when native acknowledgment fails, and checks unsubscribe. Scoped Prettier and diff check again exited 0. React consumers must adopt useProjectScope rather than a non-observed lookup; component integration and GUI acceptance remain outside this exclusive package.
