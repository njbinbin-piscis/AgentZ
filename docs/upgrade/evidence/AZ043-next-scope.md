# AZ043 next candidate scope: owned debug-session lifecycle

Read-only source review, 2026-10-10. No product edits, Cargo, GUI, adapter launch, installation or release was performed. The findings below are reproducible interleavings derived from actual code; the proposed fixtures have not been run. No recording/native-debug acceptance is claimed.

## First implementation boundary

The smallest useful candidate spans `src-tauri/src/commands/dap.rs`, `src/extensions/debug/dapClient.ts`, `src/extensions/debug/debugController.ts` plus targeted tests. Keep existing DebugView and CodeZ navigation unchanged in the first package. Root can own the Rust broker; separate agents can own the client and controller with a frozen session-envelope contract. One debug lifecycle candidate, not three unrelated roadmap tasks.

## Concrete defects

1. **P1 — Stop can hang permanently.** `dapClient.ts:101-108` inserts pending requests without a deadline. `debugController.ts:194-202` waits for disconnect before calling dispose, the only operation that rejects pending requests. If the adapter exits or never answers disconnect, Stop never reaches disposal and next Start hangs at `await this.stop()` (line 60). The exit notification does not itself reject pending requests. Fix client request deadlines, clear pending on send failure/exit, and detach current client synchronously before best-effort bounded disconnect/disposal. Stop must not need an adapter reply to revoke its UI ownership.

2. **P1 — Late old transport messages control a newer session.** Broker output/log/EOF tasks emit global `agentz:dap` without identity (Rust lines 90-123). Every client subscribes to the same stream; request sequences restart at 1. Old EOF can be consumed as new-client exited, causing controller to stop the new adapter. Old responses can settle new requests with the same sequence. `dap_send` and `dap_stop` also operate whichever manager child is current without caller identity. Fix a caller-captured opaque session ID, validated by host, returned in status and included in every event. Compare it on send/stop; old stop must not clear a replacement. Host generation also guards publication/install so overlapping starts cannot replace a newer child. Filtering only in the controller is insufficient.

3. **P1 — Mutable current-client access after awaits.** Start creates `this.client` but continues reading the field across connect/initialize/launch waits. Two concurrent Start events, Stop during Start, and a late old-start rejection can operate on or stop a replacement client. `onStopped` and `loadVariables` likewise request via mutable current client and publish after awaits without session/paused-generation checks. Capture a local owned client and lifecycle epoch; require ownership after every await. A second pause/continue needs a stopped-observation revision so old stack/scopes/variables cannot repaint a running or newer stopped session.

4. **P2 — Failed step leaves a false running UI.** Controller line 190 marks running before requesting continue/next/stepIn/stepOut and only logs rejection. The toolbar remains disabled even though the adapter rejected the command. Do not publish running until the request succeeds or authoritative continued arrives; retain/report the prior paused state on failed request under the same session/stop revision. Running currently also retains prior frames/variables; clear paused projections on actual continue and clear variables at each new stop, including empty scopes, rather than show stale values.

5. **P2 — Broker frame accumulation has no bound.** `drain_frames` (line 201) waits forever for a header separator or declared body, permits absent/invalid Content-Length as zero, and uses unchecked `body_start + content_len`. A malformed adapter can cause unbounded allocation or arithmetic panic. Extract a bounded incremental parser with checked arithmetic, header/body maximums and explicit protocol failure. On failure terminate only the owned adapter and issue a same-session diagnostic/exit. This is independent of remote/real adapter installation and can be verified with byte fixtures.

## Project and exception/source limitations

`extensionService.start` changes its projectDir before draining the old extension host; `stopInternal` does not stop a debug session. Controller reads that mutable global projectDir only after creating a client and uses arbitrary config.cwd. Session identity must capture the project/remote target at start; project switching needs a deliberate owned-session stop or visibly retained original context, not rebinding a running session. This is a follow-up integration point in `extensionService.ts`, not proof that current debugging is safely task-scoped.

Stopped reason/description/exception fields are discarded; all stops look like paused. No `setExceptionBreakpoints`/`exceptionInfo` requests or capability negotiation exist. `DebugView.tsx:46-48` renders stack frame name and line as plain non-clickable divs. Frame source is stored by controller but has no editor routing, so there is no existing source-navigation permission check to certify. Do not add file navigation merely by concatenating adapter-provided source with current project: later navigation must retain captured original project/authority, reject other roots/recovery namespace and use explicit CodeZ reveal contract. BreakpointStore is process-global and sendBreakpoints sends all paths; any filtering must use the captured session root/target, not the latest UI project.

The node adapter default is explicitly a placeholder (`node --stdio`); successful generic lifecycle fixtures will not establish functioning Node debugging. ConfigurationDone and breakpoint errors are currently swallowed. A timeout race at line 96 proceeds with configuration even without initialized; minimum lifecycle patch should make timeout a failure, and negotiate supportsConfigurationDoneRequest rather than infer support.

## Minimal deterministic tests

- Deferred fake adapter never answers disconnect: Stop settles within configured bound, clears active state, rejects pending exactly once; subsequent Start works.
- Session A emits late response with same seq as B, old EOF and stderr: B ignores A; old stop/send cannot mutate B in broker fixture.
- Two Start calls resolve connect out of order; old initialization/rejection cannot launch/configure/stop B.
- Pause A stackTrace delayed, then Continue or Pause B; late A cannot restore frames/variables. Empty scopes clears previous variables. Failed continue leaves paused status and exposes error.
- Client send failure removes pending entry; deadline/dispose/exit clear timers and cannot settle twice. Disposal during listen/connect must not leave subscription/process ownership.
- Pure parser split frames/header/body limits/invalid lengths/overflow: bounded error, no silent truncated continuation. Rust owned fake stdio fixture can verify stopping stale session does not stop current child without external hosts.

After these tests, one installed real adapter/native launch/step/exception/stop/source-navigation walkthrough remains a distinct acceptance gate. Current review cannot claim it passed.
