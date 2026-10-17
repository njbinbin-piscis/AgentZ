# AZ040 bounded input/budget independent review

# AZ040 input-context independent integration review

2026-10-10. Read-only design and current state-source review; no product changes, tests, Cargo, models or GUI run. Presentation is descriptive configuration, not a new permission/Run authority.

## Exact input sources and safe wiring

CodeZ AssistantPanel chatSend sends modelId || null; WorkZ sends showModelSelector ? modelId || null : null. useAppSettings derives defaultModelLabel/defaultModelHint from getSettings provider/model/providers; a loaded label is a configured global default, not proof of the model chosen for an existing turn. Failed settings loading clears these values. Selected scoped model IDs must be resolved in the current provider list; absent/stale IDs render Unknown rather than guessed model names. In hidden-selector WorkZ agent/team/pool mode, show Unknown/agent-controlled instead of the remembered modelId or claiming the global default is effective. Keep send request fields unchanged.

Permission policy is exactly appSettings.policy_mode:string (settings.ts). Label it configured policy. Missing/unrecognized values render Unknown; do not convert arbitrary string to an approval grant. PermissionRequestCard contains requestId/toolName/input/description and optional approval taskId/environmentId/expiry, but no compulsory owning session/project. Current setPermissionRequest handlers do not retain envelope.sessionId, and clearing is tied to card onResolved rather than all task/project/final transitions. WorkZ filters incoming permission events to foregroundSessionRef, which does not alone invalidate a previously retained card on navigation. Consequently !!permissionRequest is not a trustworthy current “waiting for approval” predicate. Minimal safe summary may omit that state; alternatively retain project/session ownership at receipt, match current identity before rendering, clear on task/final boundaries, and respect optional approval task/expiry without pretending it proves a live host response channel. Do not show raw tool inputs in summary.

CodeZ queueRef/queuedView is a real local unsent-message queue, populated while busy and on timeout continuation and shifted after completion. queuedView.length can describe “local messages queued”; it is not authoritative persisted Run execution=Queued. WorkZ has pendingResumeRef and multi-task running bookkeeping but no equivalent queuedView message queue: do not invent queued count from busy or number of running tasks. Busy is local UI activity, distinct from verified current Run/Attempt state. Latest tool/action should remain Unknown unless bound to the current task and attempt; never reuse old stream content for a summary.

WorkZ authority project and configured execution directory should be separate labels: existing registered worktree path when available, otherwise configured free cwd/direct directory. They are send-time configuration, not a fresh environment attestation. Team/pool modes remain explicitly separate. Advanced grouping must preserve current control handlers, task-bound/busy locks, selected values, free/isolate defaults and send semantics; collapsed controls must not unmount/clear configuration through new lifecycle logic.

## Minimum verification and outstanding acceptance

Meaningful fixtures: missing settings/provider, stale selected model, hidden selector with remembered model, configured policy versus grant, stale permission card after task switch, local queue count versus Run queued, collapsed controls preserving values and existing disabled rules. Existing observed budget unknowns remain unknown and enforcement unconnected. No model/account balance zero substitution. Native keyboard/focus/resize, real permission lifecycle, five-user first-use feedback and GUI acceptance remain unexecuted. This review supports a bounded descriptive summary plus WorkZ Advanced presentation candidate, not AZ040 final acceptance.


# AZ040 summary/Advanced independent implementation review

2026-10-10. Read-only review of InputContextSummary and the added CodeZ/WorkZ JSX integration. Existing unrelated repository diff was not attributed to this slice. No independent tests/build/models/GUI run.

The shared summary is presentational: props, translated strings and native details only; no IPC, tool execution, permission response, state mutation or scope minting. Unknown/empty strings remain Unknown, optional counts reject negative/unsafe values. Policy explicitly says configured policy does not grant permission. Workspace is explicitly configured context rather than host-verified environment. Queued text explicitly says “Queued locally”/“本地待发”; it is not persisted Run state. The approval display is a typed possibility but neither current caller supplies it, avoiding inference from stale permissionRequest.

CodeZ adds the summary immediately above its existing composer. Authority and workspace are the configured projectDir. Model uses current selected ID or loaded configured default label; the ID is a selection identifier, not a provider-confirmed effective model. Settings-loading failure falls back to Unknown. Busy displays Running, then existing error, then local queuedView presence, otherwise Idle. Busy takes precedence over pending local messages, so this slice is not an exhaustive queue dashboard. Existing Send/Stop and composer fields remain unchanged by the summary. CodeZ optional skills/connectors count is not yet supplied; their existing controls remain visible and functional, so no hidden-option behavior is introduced by this slice.

WorkZ only displays worktree_path when worktree.id matches sessionRef.current; otherwise the workspace fallback is configured free cwd/direct project, or Unknown for unverified isolated/agent/team contexts. Hidden model selector forces model/source Unknown and does not display the remembered scoped ID or global default. Mode labels separate agent/team/plain; policy remains configuration. Summary counts existing selected optional skills/connectors only when generic controls apply. No stale permission card is converted to waiting state, and no local queue is fabricated for WorkZ.

Native Advanced details keeps its children mounted while collapsed. Team/persona selectors retain original values, callbacks and taskBound/busy disabling. Free cwd and isolation remain outside the collapsed controls. The new start-source/base controls are scoped to isolate && !isFree && !taskBound and busy-disabled, feeding already existing startMode/startBase state consumed by task creation. The default remains working; no automatic switching or selection clearing was added. This adds access to existing configurable inputs rather than changing the send algorithm. Keyboard/focus, overlay placement within collapsed details, layout clipping and native desktop interaction still require GUI acceptance.

No blocking source defect was found in this bounded descriptive-summary/Advanced slice. It does not close budget presentation, permission receipt ownership, verified Run/Attempt live state, five-user first-use evaluation or full AZ040 acceptance. Selected model IDs are intentionally descriptive configured identifiers; future friendly-label resolution must preserve Unknown for missing provider definitions.

Reviewed SHA256:
- src/components/chat/InputContextSummary.tsx: 10ab6a5753f3874655cb2c9636c4b146afc0d8b63f68b3e024ab0f282e1c5847
- src/workspaces/codez/AssistantPanel.tsx: 7c82b46a98d816b34f4ab0ffedbff22f48fa6eaee0010cd9bfcaae25d82a7598
- src/workspaces/workz/index.tsx: e11099d392fe3184eb539db9b2c8b612b1b52af18ac12262c68a9c90ba51f468


# AZ040 observed-budget presentation independent review

2026-10-10. Read-only source/fixture review; no tests, Cargo, models or GUI executed by reviewer. Root reports author BudgetObservation seven and TaskDetailsPanel five targeted tests passed; final combined validation remains root-owned.

BudgetObservation renders totals only for an available typed source. Missing/unknown/stale/unavailable/unsupported sources have explicit status text and no numeric totals. Available known zero is preserved as an observation, not substituted for absent values; null policy maxima use Not configured via nullish checks. Known input/output, known active duration and usage count are distinct from unknown token/duration/pricing record counts. The fixtures cover each unavailable source, mixed known/unknown records, real known zero and absent cost observations.

Costs remain separate currency buckets in micro-units; no conversion, invoice claim, subtraction from maxima, remaining balance or token-to-price estimate is introduced. Empty cost map says No known cost observations. Configured maxima and observed exceedances are labelled observational; the component explicitly says enforcement is not connected, known totals may be incomplete and maxima are not an enforced allowance. It does not call the reservation API or authorize execution. Data validity is supplied by the existing strict taskDetails DTO validator; this component is not a replacement trust boundary.

TaskDetailsPanel change is an import plus replacement of its old budget paragraph with BudgetObservation using data.sources.budget. Existing scope/project/session request key masks old data during render before effect; generation/disposed checks suppress late responses, focus coalescing and error clearing remain intact. Therefore no old budget is exposed through the new child on a task/scope switch. Sources-read-separately/non-atomic disclaimer remains displayed. No polling or model continuation was added.

No blocking defect found within this observational presentation slice. Host accounting completeness, prices/invoices, reservation enforcement, team/member coverage and GUI usability remain unverified and are not inferred from these fixtures.

Reviewed SHA256:
- src/workspaces/codez/BudgetObservation.tsx: 567d6defee4616872f05a7995420e52d0cafdddc0b0a8ec24db8ad001878fa7c
- src/workspaces/codez/BudgetObservation.test.tsx: db01fad4a7d937dd0009db77a80a4499c501030b5112616e70c78ea20abcf22f
- src/workspaces/codez/TaskDetailsPanel.tsx: 8b925d75fb02697050ad3bc910999dfe5348cdf88a209bf3ee7b0f1bd9ad1a6d
