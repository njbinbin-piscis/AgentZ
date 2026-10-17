# AZ-021 cancellation candidate independent review

Reviewed 2026-10-09; source review only. This reviewer did not run Cargo, GUI, models, SSH, or operating-system process acceptance.

## Limited conclusion

No blocking defect found in the reviewed local single-agent invocation/owned-worker cancellation decision. This does not attest cleanup of arbitrary detached, remote, team, or pool resources.

- `chat_turn.rs` captures `cancellation_requested` once before drain. That same snapshot controls Run cancellation and lifecycle classification. A later Stop cannot become a falsely confirmed cancellation without drain. The later flag read only decides whether to save an interrupted reply tail; it does not authorize terminal Cancelled.
- Capture retains the registry before the agent starts and binds the exact trusted Attempt plus fresh cancellation Arc. `wait_requested` rejects another binding and an unset flag. Only Complete permits Cancelled; Pending/Failed contribute `turn_failed` and timeout/interrupted finalization. Storage failures retain their separate failure cause and suppress cancellation/timeout success classification.
- The actual main-loop tool call runs through `call_with_completion`. Registration checks cancellation/sealing before invoking the tool. A normally returned Result, including Err, acknowledges invocation termination; dropped or timed-out futures drop an incomplete receipt and permanently record failure. Consequently an uncovered pending generic tool cannot leave an empty registry that incorrectly returns Complete.
- Vendor shell/code_run register a second receipt before spawn_blocking and move it into the worker. Dropping the generic future does not drop the worker receipt. Drain remains Pending/Failed with active count until actual worker cleanup returns; timeout does not kill or falsely acknowledge that worker.
- Registry drain seals new starts, retains failures after active reaches zero, and handles poisoned state conservatively. Cancellation does not rerun tools.

## Actual implementation and reconstruction evidence

Source/vendor `agent/loop_.rs` bytes agree: SHA-256 `32f68e24181f1f6b63f43af27485c56ff7a5be1d0c5bd6b12a3c3c68cdd17280`.

Incremental patch `docs/upgrade/patches/AZ-021-invocation-completion-v0.8.68.patch`: SHA-256 `b5e054e5e15999239824571e39f802b7641c0ba44a4faaa5591e80f405211592`. Baseline is frozen AZ-023 loop SHA-256 `846ae9d3198c2c6bac80611a530998ee8872fabb80154e08a2921066ea795781`; this is not a complete vendor reconstruction.

Actually ran rustfmt on the owned loop file and isolated git apply --check/apply/reverse --check/reverse with exact target/baseline byte comparisons. All passed in `.cache/az021-invocation-i58w7gb1`; local test-repository core.autocrlf=false, no global Git configuration change.

Added two controlled fixtures: boxed deferred generic tool future Drop yields Failed { active: 0, failures: 1 }; normal returned tool Err completes invocation while a pre-cancelled context refuses invocation. These fixtures and existing detached blocking-worker fixture require parent-run Cargo verification; root reported test process 54937 underway, no outcome claimed here.

## Remaining acceptance limits

A normal tool return proves only invocation termination. A tool that detaches resources without registering an owned worker is not resource-confirmed by this mechanism. Standalone source shell/code_run differ from the vendor sandbox implementation and were not overwritten; the vendor sandbox overlay is separately versioned. Fresh cancellation identity per Attempt and capture-before-launch are host preconditions, not renderer authority. GUI Stop interaction, actual AppContainer descendants/pipes/ACL teardown, team/pool/remote resources and provider billing remain unverified. A late Stop after the finalization snapshot may finish as the already-completed turn; this avoids claiming a cancellation that was never drained.
