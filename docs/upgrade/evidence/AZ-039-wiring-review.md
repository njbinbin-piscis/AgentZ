# AZ039 two-workspace wiring independent review

2026-10-10. Read-only inspection of root integration; no tests/typecheck/GUI/model or Git operations executed. The four new component files are covered separately by AZ-039-implementation-review.md, left unchanged.

CodeZ mounts DeliveryReviewPanel only with a current project and session, using those inputs for the scope-aware observer. Open artifacts sets a project/session identity key; TaskArtifactViewer mounts only when that key matches the current pair and is React-keyed by it. Root added key clearing on project reset, newSession and switchSession start, preventing an existing artifact preview surviving asynchronous selection of another session. No review/merge/PR callback is attached to CodeZ; its existing composer/stream handling is otherwise unchanged by this addition.

WorkZ reuses the existing non-team/non-pool, selected-project/canonical-project/current-session gate. It passes canonical authority to DeliveryReviewPanel. Review changes is present only when not busy and worktree.id matches sessionRef.current, and only opens the existing review UI; no merge/discard/PR runs automatically. Open artifacts uses the same canonical-project/session key as the existing viewer. Old preview clearing at openTask async start remains present. Changing the project/session prevents mismatched-key viewer mounting; child scope generation validation remains the actual asynchronous-response isolation. Neither button issues native permission or changes selected project/task.

The WorkZ review button is UI eligibility, not a host authorization or proof the task remains idle until a later review action. Existing AgentTaskReview/openPr limitations recorded under AZ013 remain unchanged. Delivery observations refresh manually and do not constitute an automatically live completion notification. Artifact viewer has explicit version capture/delete controls from AZ038; merely opening it does not mutate. Native GUI layout/focus and combined tests/typecheck are root-owned outstanding checks, not asserted here.

No blocking defect found in these added mounting/key/reset lines within the limited contract.

Reviewed SHA256:
- src/workspaces/codez/AssistantPanel.tsx: ab469eb32e479f537b7819a2e95311821398f091c6243a8393a2dfc755874673
- src/workspaces/workz/index.tsx: 472d401fd2bf8b543162f2ffe30a00b4e7aab145fd94a81a224d9b68a20cb866
