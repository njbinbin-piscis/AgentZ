# AZ036 observation-only task-view client review

2026-10-09. Independent read-only source/test review; no additional tests/typecheck/GUI run. Root sessions29454/29325 underway; no outcome claimed here.

No blocking defect found in this limited DTO/client slice. invoke sends only opaque scopeHandle and sessionId to task_view_resolve, never authority/workspace paths. Local authorityProject is solely an expected comparison value, not host permission. Exact record keys, own properties, plain-object prototype, version1 and observation_only true are enforced. Task/session/original authority identity must match both top level and scope; workspace remains a separate bounded string and is not substituted as project authority. Environment string and binding hash checks reject malformed identity. Request prevalidation occurs before invoke.

Tests meaningfully cover differing authority/workspace, exact IPC arguments, substituted identities, operational claims/extra field, malformed binding and oversize path, and invalid request without invocation. They are source fixtures, not evidence of host lookup/lease/revocation or IDE action authorization. Exported validateTaskView assumes the expected arguments are supplied by a trusted client path; observeTaskView validates them. Returned DTO is an observation, not a capability or a native task-view handle. Consumers must not drive terminal/editor/send authorization directly from workspace_root. Concurrent UI response isolation remains caller responsibility and is not delivered by this service alone.

The module does not mutate project/session, resume model, mint Run or register a tool. Schema strictness will reject future fields until deliberately versioned. No native GUI or full same-task CodeZ/WorkZ routing acceptance inferred.

Source fingerprints:
- src/services/tauri/taskView.ts: 1ec02d0698eb4c4cb8ec82a31453007cbd35fe28f4502310663aebfdc2cafd7b
- src/services/tauri/taskView.test.ts: b6a238ddde57abaf2a0644a738684e45a3cfe716fb961b2c5bb3ef9de7502986
