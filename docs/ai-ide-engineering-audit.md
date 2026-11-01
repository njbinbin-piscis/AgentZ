# AgentZ AI IDE Engineering Audit and Remediation Guide

**English** | [简体中文](ai-ide-engineering-audit.zh-CN.md)

> Audit date: 2026-09-28  
> Scope: editor, Git version control, Hint/AI Completion, LSP, performance, Extension Host, Agent Harness, security boundaries, testing and engineering maintainability  
> Project version at audit time: `0.6.3`  
> Nature of the audit: read-only code audit plus local build verification; no product code was modified during the audit

> **Status (2026-11-02, v1.0.40, piscis-engine v0.8.71):** The findings below are preserved as written for `0.6.3`. File/line references in the body point at the 0.6.3 code and no longer match current line numbers. Per-finding status against the current code:
>
> | ID | Status | Where it stands now |
> | --- | --- | --- |
> | LSP-01 | Fixed | `CodeEditor.tsx` passes a file-URI `path` to Monaco (comment near line 729); shared URI helper in `src/services/tauri/editorUri.ts` (matches `monaco.Uri.file`). |
> | LSP-02 | Fixed | Project/language session reuse in `src/services/tauri/lspSession.ts` (+ `lspSession.test.ts`). |
> | LSP-03 | Fixed | `LspClient` in `src/services/tauri/lsp.ts` uses a single `ws.onmessage` dispatcher, pending map with timeouts, and `rejectAllPending()` on close/disconnect; `waitForResponse()` is gone. |
> | LSP-04 | Fixed (by design change) | The frontend sends bare JSON; the Rust bridge owns Content-Length framing (`lsp.ts` header comment, test "sends bare JSON, never Content-Length framing" in `lsp.test.ts`). |
> | LSP misc. | Fixed | Document versions are per-URI integer counters (`docVersions` in `lsp.ts`); completion fallback range uses `model.getWordUntilPosition` (`lsp.ts` ~line 660). |
> | HINT-01 | Fixed | `ai_inline_completion_cancel` + per-request cancel flags in `state.completion_cancel` (`src-tauri/src/commands/edit.rs`); frontend `cancelAiInlineCompletion` in `src/services/tauri/edit.ts`. |
> | EDIT-01 | Fixed | Reject applies a scoped inverse edit (`rejectInline` in `CodeEditor.tsx`) and refuses if the document changed after the preview; no global undo. |
> | GIT-01 | Fixed | `run_git_cmd_bytes(&repo, &["status", "--porcelain=v1", "-z", "-uall"])` in `src-tauri/src/commands/ide.rs`; NUL/rename parsing in `git_workspace.rs`. |
> | GIT-02 / GIT-03 | Fixed | Git commands call `git_workspace::resolve_git_context_for_root(..., git_root)` / `resolve_git_dir(..., git_root, ...)`; no more `let _ = git_root;`. |
> | AGENT-01 | Fixed | All `ToolContext`s now use `bypass_permissions: false` (`commands/chat_turn/mod.rs`, `chat_turn/tool_approval.rs`, `runtime/approval.rs`, `browser/mod.rs`, `tools/devenv.rs`, `tools/durable_plan_todo.rs`). |
> | 7.2 capability metadata | Partially | Kernel `Tool` trait exposes `is_read_only()` (`piscis-kernel/src/agent/tool.rs`); the full structured `ToolCapabilities` metadata proposed here does not exist. |
> | SEC-01 | Fixed | Mermaid runs with `securityLevel: "strict"` and SVG is sanitized via `src/workspaces/codez/mermaidSafe.ts` (`loadMermaid`, `sanitizeMermaidSvg`); `src-tauri/tauri.conf.json` now defines an explicit CSP. |
> | 9.3 Mermaid chunking | Fixed | Both `Markdown.tsx` and `MarkdownPreview.tsx` load Mermaid via `loadMermaid()`. |
> | CSS-01 | Fixed | `src/extensions/ui/extensions.css` comment rewritten without `*/`. |
> | TEST-01 | Partially | Frontend now has 69 test files (LSP, LSP session, Git panel, editor recovery/conflict, permission card, etc.); full Agent policy matrix and Extension Host contract suite remain open. |
> | MAINT-01 | Partially | `chat_turn.rs` split into `commands/chat_turn/{mod,prompt_assembly,tool_approval,post_turn}.rs`; `workz/index.tsx` (~300 lines), `AssistantPanel.tsx` (~320) and `codez/index.tsx` (~560) shrank. `commands/ide.rs` grew to ~2,900 lines, `CodeEditor.tsx` ~900, `lsp.ts` ~880, `gateway/wechat.rs` ~2,050 — still open. |
> | PERF-01 / 9.4 | Not re-measured | Bundle sizes were not re-measured for this update. |

## 1. Executive Summary

AgentZ has built a fairly complete desktop AI IDE stack: a Tauri/Rust backend, a React/Monaco editor, a standalone Extension Host, LSP, Git, multi-agent, workflows, context compaction, a file journal, a code index and a knowledge graph are all in place.

Measured against long-term maintenance and shipping to real users, the project is currently closer to a broad-coverage Beta:

- Rust foundation modules, workflows, indexing and the gateway have good test quality.
- The core paths of the editor, LSP and Git contain defects that directly affect functional correctness.
- The Agent Harness already has a PolicyGate and capability tiers, but the actual execution context bypasses permission confirmation by default, leaving a gap in the security model.
- Frontend tests, LSP integration tests and Extension Host contract tests lag well behind feature growth.
- Monaco, Mermaid and the main app carry a high load cost; cold start and time-to-first-usable-editor have substantial room for improvement.
- Several core files exceed 1,000 lines; regression risk will rise quickly as features keep growing.

Overall assessment:

| Dimension | Rating | Notes |
| --- | ---: | --- |
| Architectural potential | 8/10 | Layering is sound; boundaries between Rust kernel, Tauri host, React UI and Extension Host are mostly clear |
| Correctness/reliability | 6/10 | Critical defects in LSP lifecycle, model URIs, protocol races and Git path parsing |
| Security boundaries | 5/10 | ToolContext permission bypass, Mermaid loose mode and disabled CSP form a combined risk |
| Performance | 5/10 | Monaco, the TS worker, the main bundle and Mermaid graph dependencies are large |
| Test maturity | 6/10 | Rust unit tests are good, but IDE main paths and end-to-end tests are insufficient |

Recommendation: address the P0/P1 issues in this document before expanding with new features.

## 2. Audit Method and Verification Results

### 2.1 Verification performed

```bash
npm run typecheck
npm run lint
npm test -- --reporter=verbose
cargo check --workspace
cargo test --workspace
npm --prefix extension-host run build
npm run build
```

Results:

- TypeScript typecheck: passed.
- ESLint: 0 errors, 61 warnings, including several React Hook dependency issues.
- Vitest: 5 test files, all 21 tests passed.
- Cargo check: passed.
- Rust tests: 87 passed, 1 test requiring local Chromium ignored.
- Extension Host: build passed, `dist/host.js` about 80 KB.
- Vite production build: passed, but with a CSS syntax warning, an ineffective dynamic-split warning and several large-chunk warnings.
- No existing business files were modified between the start and end of the audit; this document was the only new file.

### 2.2 Size of key source files

The following files have become maintenance hotspots:

| File | ~LOC | Risk |
| --- | ---: | --- |
| `src-tauri/src/commands/chat_turn.rs` | 2,288 | Agent turn assembly, tool registration, context, execution and persistence mixed together |
| `src-tauri/src/gateway/wechat.rs` | 2,064 | Protocol, networking, state and parsing concentrated in one place |
| `src-tauri/src/commands/ide.rs` | 1,560 | Aggregates file, Git, terminal, watcher and LSP commands |
| `src/workspaces/workz/index.tsx` | 1,521 | Workspace UI with a lot of concentrated state |
| `src/workspaces/codez/AssistantPanel.tsx` | 1,368 | Session, events, input, plan and tool state mixed |
| `src/workspaces/codez/index.tsx` | 1,323 | File tree, tabs, watcher, Git refresh and layout concentrated |
| `src/workspaces/codez/CodeEditor.tsx` | 690 | Monaco, LSP, AI completion, inline edit and DAP mixed |
| `src/services/tauri/lsp.ts` | 737 | Protocol, client, conversion and Monaco providers mixed |

## 3. Defect Priority Overview

| ID | Priority | Module | Issue | Main impact |
| --- | --- | --- | --- | --- |
| LSP-01 | P0 | Editor/LSP | Monaco model URI does not match provider target URI | Hover, completion, go-to-definition and references may all fail; markers may land on the wrong file |
| LSP-02 | P0 | Editor/LSP | LSP disconnects on tab switch but is not reliably reconnected | Tabs other than the first file lose LSP |
| LSP-03 | P0 | LSP protocol | Race that can lose the initialize response | Connection occasionally waits 10 seconds and fails |
| LSP-04 | P0 | LSP protocol | Content-Length uses UTF-16 character length | Wrong length for messages containing CJK/emoji |
| AGENT-01 | P0/P1 | Agent Harness | ToolContext defaults to `bypass_permissions: true` | Permission model cannot be guaranteed; file write / command execution tools may bypass confirmation |
| SEC-01 | P1 | Markdown/Tauri | Mermaid `securityLevel: loose` with CSP disabled | Larger attack surface when rendering untrusted content |
| GIT-01 | P1 | Git | porcelain v1 parsed by line/string slicing | Wrong paths for spaces, quotes, CJK, newlines, renames |
| GIT-02 | P1 | Git | Nested-repo diff mixes workspace path and repo-relative path | Empty diff or file not found |
| GIT-03 | P1 | Git API | Several APIs accept `git_root` and then ignore it | Repo chosen in the UI may differ from the one actually operated on |
| EDIT-01 | P1 | Inline Edit | Reject relies on the global undo stack | May undo user edits after concurrent edits |
| HINT-01 | P1/P2 | AI Completion | Only frontend logical cancellation, no backend request cancellation | Wasted inference, higher cost, resource usage |
| PERF-01 | P1/P2 | Build/startup | Monaco, main bundle and TS worker too large | High cold start, editor-ready time and memory usage |
| CSS-01 | P1 | CSS | Comment contains `*/`, causing a syntax warning | Following CSS may be mis-parsed |
| TEST-01 | P1/P2 | Testing | No regression tests for editor/Git/LSP/Agent permissions | Core defects are hard to catch in time |
| MAINT-01 | P2 | Architecture | Core module responsibilities and file sizes too large | Rising change cost and regression risk |

## 4. Editor and LSP

### 4.1 LSP-01: Monaco model URI does not match provider URI

#### Location

- `src/workspaces/codez/CodeEditor.tsx:593-640`
- `src/services/tauri/lsp.ts:626-660`

`CodeEditor` creates the Monaco `<Editor>` without passing `path`:

```tsx
<Editor
  height="100%"
  theme={editorTheme}
  language={editorLanguage}
  value={tab.content}
  ...
/>
```

Without `path`, `@monaco-editor/react` usually uses an `inmemory://model/...` URI. The LSP provider, however, uses a file URI:

```ts
const modelUri = monaco.Uri.parse(`file://${filePath}`);
const matchesFile = (model) => model.uri.toString() === modelUri.toString();
```

#### Impact

- The completion provider returns `null` because `matchesFile` is false.
- Hover, Definition and References fail the same way.
- When diagnostics cannot find the target model they fall back to `monaco.editor.getModels()[0]`, which can write markers to the wrong tab.
- Different tab open orders make the problem appear random.

#### Fix

1. Create a single URI helper:

```ts
export function editorUri(monaco: typeof Monaco, fullPath: string) {
  return monaco.Uri.file(fullPath);
}
```

2. Pass a stable path to Editor:

```tsx
<Editor
  path={fullPath}
  keepCurrentModel
  ...
/>
```

3. `registerLspProviders`, markers, didOpen/didChange/didClose and definition/reference locations must all use the same URI logic.
4. Remove this fallback:

```ts
monaco.editor.getModel(modelUri) ?? monaco.editor.getModels()[0]
```

If the target model does not exist, skip the marker update and emit an observable log.

#### Acceptance tests

- After opening TS, Rust and Python files, each model URI must be the corresponding file URI.
- With two same-language files open, diagnostics appear only on the corresponding file.
- Hover, Completion, Definition and References providers match the current model.
- Closing a file does not update markers on the disposed model.

### 4.2 LSP-02: LSP lifecycle breaks after tab switch

#### Location

- Initialization: `src/workspaces/codez/CodeEditor.tsx:483-535`
- Cleanup: `src/workspaces/codez/CodeEditor.tsx:554-566`
- Editor mount: `src/workspaces/codez/CodeEditor.tsx:595-624`

LSP initialization lives entirely inside the `onMount` callback. On tab switch React may reuse the same Editor component, so `onMount` does not run again. Meanwhile the cleanup of the effect keyed on `tab.path` disconnects the current client.

#### Reproduction

1. Open the first LSP-capable file.
2. Wait for the LSP to connect.
3. Open and switch to a second file.
4. The effect cleanup for the first path calls `disconnect()`.
5. Monaco does not remount; the new file never runs LSP start/connect/provider registration.

#### Fix

Separate responsibilities:

- `onMount`: only store the `editor` and `monaco` instances and register global commands.
- `useEffect([projectDir, language])`: obtain or reuse a project/language-level LSP client.
- `useEffect([client, tab.path])`: perform didOpen/didClose and register file-level provider/marker bindings.
- One long-lived LSP server/session per project + language; do not stop the server on every file switch.
- Maintain a per-document incrementing version number.
- Reclaim servers according to policy when the project closes or the last document of that language closes.

Suggested data structures:

```ts
type DocumentState = {
  uri: string;
  version: number;
  opened: boolean;
};

type LanguageSession = {
  projectDir: string;
  languageId: string;
  client: LspClient;
  documents: Map<string, DocumentState>;
  refCount: number;
};
```

#### Acceptance tests

- After switching A.ts → B.ts → A.ts, LSP works in both.
- Switching TS → Rust → TS does not wrongly reuse a different language server.
- Ten rapid switches leave no orphan WebSocket/provider.
- Closing one tab does not disconnect the server used by other tabs of the same language.

### 4.3 LSP-03: initialize response race

#### Location

- `src/services/tauri/lsp.ts:155-245`
- `src/services/tauri/lsp.ts:435-462`

Current flow:

```ts
ws.send(initReq);
const initResp = await this.waitForResponse();
```

`waitForResponse()` temporarily replaces `ws.onmessage`. If the initialize response arrives before the handler is replaced, the original `handleMessage()` receives it; because the initialize ID was never added to `pending`, the response is dropped and the wait eventually times out after 10 seconds.

#### Fix

- Allow only one `onmessage` dispatcher for the WebSocket's lifetime.
- initialize must go through the common `sendRequest("initialize", params)`.
- `sendRequest` must register the pending entry before sending.
- A pending entry holds `resolve`, `reject` and a timeout handle.
- Reject when the server returns a JSON-RPC error instead of treating `null` as a normal result.
- Reject all pending requests on disconnect/close/error.
- Remove the `waitForResponse()` handler-swapping implementation.

Reference structure:

```ts
type PendingRequest = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timeoutId: ReturnType<typeof setTimeout>;
};
```

#### Acceptance tests

- Connection reliably succeeds when a fake LSP server replies to initialize synchronously/immediately.
- When initialize returns an error, fail immediately without waiting for the timeout.
- All pending promises are rejected on WebSocket close.
- No messages are lost when notifications and responses interleave.

### 4.4 LSP-04: wrong Content-Length calculation

#### Location

- `src/services/tauri/lsp.ts:401-404`

Current code:

```ts
return `Content-Length: ${body.length}\r\n\r\n${body}`;
```

JavaScript `string.length` counts UTF-16 code units, not the UTF-8 byte length that LSP Content-Length requires. CJK, emoji and some combining characters all produce a wrong length.

#### Fix

```ts
const byteLength = new TextEncoder().encode(body).byteLength;
return `Content-Length: ${byteLength}\r\n\r\n${body}`;
```

Also confirm whether the WebSocket bridge really needs LSP stdio framing. If each WebSocket frame on the bridge is already one JSON-RPC message, prefer sending plain JSON to avoid double framing.

#### Tests

- Body containing CJK, emoji, CRLF and surrogate pairs.
- One WebSocket frame containing one or more LSP frames.
- Fragmented frames are reassembled correctly by the bridge; if the protocol does not support fragmentation, state the constraint on both sides.

### 4.5 Other LSP correctness issues

#### Diagnostics are not isolated per URI

`LspClient` has only one `diagnostics: LspDiagnostic[]` and one callback. Once multiple documents share a client, it must become:

```ts
Map<DocumentUri, LspDiagnostic[]>
Map<DocumentUri, Set<DiagnosticsListener>>
```

`publishDiagnostics.params.uri` must take part in routing.

#### Document version uses `Date.now()`

`src/services/tauri/lsp.ts:248-264` uses a timestamp as the version. Although usually increasing, semantically it should be a per-document integer counter. Rapid calls, system clock adjustments or snapshot restores can all break the ordering assumption.

#### Wrong completion fallback range

`src/services/tauri/lsp.ts:543-550` gives a fixed `(1,1)-(1,1)` range when an LSP item has no `textEdit`. This makes completion text insert at the top of the file, or Monaco may reject it as invalid.

Correct approach:

- fill in the range at the provider layer from the current `position` and current word range; or
- have the converter accept `model` and `position` and compute the replacement range via `model.getWordUntilPosition(position)`.

#### Hand-forged Location URI

`src/services/tauri/lsp.ts:556-575` hand-constructs a fake `Monaco.Uri` object. Use `monaco.Uri.parse(loc.uri)` where the provider has the Monaco namespace, and handle percent-encoding, Windows drives and UNC paths correctly.

## 5. Hint, AI Completion and Inline Edit

### 5.1 HINT-01: AI completion cannot actually cancel backend requests

#### Location

- Frontend provider: `src/workspaces/codez/CodeEditor.tsx:432-480`
- Backend request: `src-tauri/src/commands/edit.rs:117-206`

The frontend waits 350 ms and checks the Monaco token, but once `aiInlineCompletion()` is sent via Tauri invoke, the token cannot cancel the LLM HTTP request inside Rust. Continuous typing produces stale requests that keep running.

#### Impact

- More model-call cost and network requests.
- Stale requests occupy connections, memory and provider rate limits.
- Requests may continue after closing a tab or switching projects.
- High-latency models clog the completion queue.

#### Fix

- The frontend keeps a request generation per editor/model.
- When a new request starts, explicitly cancel the previous request ID.
- Rust keeps `request_id -> CancellationToken`.
- The LLM client accepts a cancel future/AbortHandle.
- Allow at most one in-flight completion globally or per project; newest request wins.
- Add a small LRU cache for repeated context within a short window.
- Record requested, displayed, accepted, cancelled, latency and tokens metrics.

#### Acceptance tests

- After typing 20 characters in a row, the backend keeps at most one live request.
- Closing a tab cancels the request.
- If an old request returns after a newer one, the old result is not shown.
- Provider timeouts, cancellations and API errors do not pollute the UI.

### 5.2 Completion context is too mechanical

#### Location

- `src-tauri/src/commands/edit.rs:180-187`

Currently it only takes the last 2,000 characters of the prefix and the first 600 characters of the suffix. Suggested incremental upgrades:

1. AST range of the current function/class.
2. File imports and exported symbols.
3. LSP expected type/signature.
4. Recently edited regions and context near the cursor.
5. Lightweight symbol summaries of related files instead of concatenating whole files.
6. Truncate by token budget rather than Unicode character count.

The settings or first-enable flow must clearly state that code context may be sent to the remote model/provider configured by the user.

### 5.3 EDIT-01: Inline edit reject relies on global undo

#### Location

- `src/workspaces/codez/CodeEditor.tsx:169-220`
- `src/workspaces/codez/CodeEditor.tsx:284-299`

The AI proposal is written directly into the model via `executeEdits()`; Reject uses:

```ts
editor.trigger("agentz-inline-edit", "undo", null);
```

If other edits, extension formatting, LSP code actions or external syncs happen after the preview, the top of the undo stack is not necessarily the AI edit anymore.

#### Fix

Proposal state should store:

- model URI;
- `alternativeVersionId` before apply;
- original range;
- inverse edit;
- proposal generation ID;
- version after apply.

On Reject:

- if model/version still match, apply the inverse edit;
- if a concurrent change has happened, show a conflict diff and let the user choose keep/restore;
- never use global undo as a business rollback mechanism.

### 5.4 Async consistency of inline edit

When switching tabs during generation, the code only checks whether `inlineStateRef.current` exists and does not verify the proposal still belongs to the original model/path. On response, verify:

```ts
request.path === activePath
request.modelUri === editor.getModel()?.uri.toString()
request.generation === currentGeneration
```

Discard the response on mismatch.

## 6. Git Version Control

### 6.1 GIT-01: unsafe porcelain output parsing

#### Location

- Command: `src-tauri/src/commands/ide.rs:739-745`
- Parsing: `src-tauri/src/commands/git_workspace.rs:173-208`

Current command:

```bash
git status --porcelain=v1 -uall
```

and `line[3..]` is treated as the path. This cannot correctly handle:

- spaces, quotes, backslashes and Git quotePath escaping;
- CJK/non-ASCII file names;
- rename/copy `old -> new`;
- newlines in file names;
- non-UTF-8 paths;
- the difference between Rust char boundaries and byte slicing.

#### Fix

Use NUL-separated output and parse as bytes:

```bash
git -c core.quotepath=false status --porcelain=v1 -z -uall
```

Give `run_git_cmd` a byte-oriented variant for Git status:

```rust
async fn run_git_cmd_bytes(...) -> Result<Vec<u8>, String>
```

In `-z` mode rename/copy entries need an extra path field to be read. Use `OsString`/`PathBuf` internally where possible and only do lossy conversion at the UI serialization boundary, flagged as such.

#### Required fixtures

- `normal.ts`
- `a b.ts`
- `中文.ts`
- `quote"name.ts`
- rename old → new
- copied file
- staged + worktree modified at the same time
- untracked directory
- nested repository

### 6.2 GIT-02: mixed paths in nested-repo diff

#### Location

- `src-tauri/src/commands/ide.rs:796-835`
- `src-tauri/src/commands/git_workspace.rs:102-147`

`resolve_git_context()` already returns `root` and `path_in_repo`, but parts of the diff command still use the workspace-relative `path`. When the current directory is already the nested repo root, passing a workspace path with the repo prefix duplicates the path.

Example:

```text
workspace: /repo
nested repo: /repo/packages/app
workspace path: packages/app/src/main.ts
repo-relative path: src/main.ts
```

Git commands run in `/repo/packages/app` must use `src/main.ts`.

#### Fix

Define once:

```rust
struct GitContext {
    repo_root: PathBuf,
    repo_root_rel: PathBuf,
    workspace_path: PathBuf,
    repo_relative_path: PathBuf,
}
```

All Git subcommands accept only a `GitContext`; passing an ambiguous `path: String` on its own is forbidden.

### 6.3 GIT-03: `git_root` parameter ignored

#### Location

Typical locations:

- `src-tauri/src/commands/ide.rs:906-917`
- `src-tauri/src/commands/ide.rs:935-945`
- `src-tauri/src/commands/ide.rs:953-980`
- `src-tauri/src/commands/ide.rs:986-996`

Several APIs accept `git_root` and then do:

```rust
let _ = git_root;
```

This makes the repo explicitly selected in the UI inconsistent with what the backend actually resolves, and makes the API contract misleading.

#### Fix

- If `git_root` is provided, first validate via `resolve_git_dir()` that it is inside the workspace and is a discovered repo.
- Then verify that path actually belongs to that repo.
- Infer from path only when it is not provided.
- If path conflicts with git_root, return an explicit error; do not silently choose another repo.

### 6.4 Git discard and filesystem safety

`GitPanel` already prompts for confirmation on discard at `src/workspaces/codez/GitPanel.tsx:79-109`, which is the right direction. Still recommended:

- use the existing native Tauri `confirmDialog()` to avoid unreliable WebView `window.confirm`;
- send deleted untracked files to the system recycle bin or a project recovery area first;
- create a recoverable journal/checkpoint before discard;
- discard all must not swallow per-file errors; aggregate succeeded, failed and unprocessed paths;
- all destructive operations return structured results, not concatenated strings.

### 6.5 Git product capability gaps

To reach a mature IDE experience, consider adding:

- clear separation of staged diff and worktree diff;
- hunk/line stage and unstage;
- three-way merge conflict view;
- detached HEAD, remote branches, ahead/behind;
- fetch/pull/push;
- stash;
- amend, sign-off, signing;
- askpass/credential helper integration;
- a Git operation queue to avoid concurrent index modification;
- operation progress, cancel and audit log.

## 7. Agent Harness

### 7.1 AGENT-01: execution context bypasses permissions

#### Location

- Delegated/auxiliary path: `src-tauri/src/commands/chat_turn.rs:1243`
- Main Agent turn: `src-tauri/src/commands/chat_turn.rs:2084`

Both set:

```rust
bypass_permissions: true
```

Although the outer layer creates a `PolicyGate`, the ToolContext then explicitly bypasses permissions, making the final permission semantics opaque. The main registry includes file writes, shell, process, browser, MCP, App self-management, user-defined executable tools and more; it cannot rely solely on the soft "confirm first" constraint in the system prompt.

#### Fix

1. Default to `false` for the main Agent.
2. Allow bypass only for internal, read-only tasks proven safe.
3. Permission decisions must be enforced by the harness, not left to the model.
4. Build separate policies for:
   - workspace read;
   - workspace write;
   - outside-workspace read/write;
   - process execution;
   - network;
   - credentials/secrets;
   - destructive operations;
   - external side effects, e.g. sending messages, submitting forms.
5. Log every policy decision: tool, argument summary, capability, matched rule, decision, user confirmation result.
6. The read-only registry for sub-agents is the right direction, but add a capability token on top for a double constraint.

### 7.2 Tool allowlist relies on strings and drifts easily

`chat_turn.rs:930-944` and the tool registry enable/disable capabilities by tool-name strings. Newly added, renamed or aliased tools can slip past the security classification.

Each tool should expose structured metadata:

```rust
struct ToolCapabilities {
    reads_workspace: bool,
    writes_workspace: bool,
    executes_process: bool,
    uses_network: bool,
    accesses_credentials: bool,
    external_side_effect: bool,
    destructive: bool,
    reversible: bool,
    workspace_scope: Scope,
}
```

PolicyGate decides based on metadata and arguments. Add a CI test: the build fails if any registered tool lacks capability metadata.

### 7.3 Insufficient tool transactions and recovery

FileJournal already exists and is a good foundation. But multiple modifications within one Agent turn do not naturally form a transaction: when the third file write fails, the first two may already be on disk.

Recommendations:

- create a journal transaction ID at turn start;
- file tools record before image, after image and hash;
- on turn failure, the UI offers "roll back this turn";
- automatic rollback only when the files have not been modified externally since;
- detect conflicts with content hashes;
- mark process/network side effects as irreversible; do not pretend they can be rolled back.

### 7.4 Cancellation and timeout

`chat_turn.rs:2171-2191` sets a cancel flag after timeout and then waits for the collector. Verify that every tool and the LLM client observe cancellation promptly; otherwise the UI may show a timeout while the background task keeps consuming resources.

Recommendations:

- all Tool trait implementations accept a common `CancellationToken`;
- external processes run in their own process group, and the whole group is terminated on cancel;
- HTTP requests are bound to cancellation;
- the collector has its own deadline if the producer exits abnormally;
- after timeout, distinguish `cancel_requested`, `cancelled` and `force_terminated`;
- add a fake tool test that deliberately ignores cancellation.

### 7.5 Recommended Agent Harness split

`chat_turn.rs` handles request normalization, attachments, slash commands, skills, MCP, tool registration, context, model config, execution, event collection and persistence all at once. Suggested split:

```text
TurnRequestNormalizer
AttachmentResolver
ContextAssembler
CapabilityResolver
ToolRegistryFactory
HarnessFactory
TurnRunner
TurnEventCollector
TurnPersistence
TurnRecovery
```

Principles:

- Tool registry construction does not depend on UI event details.
- Context assembly is a pure function testable with fixtures.
- Persistence is separate from the streaming collector.
- Policy decisions can be unit-tested independently.
- Main Agent, delegate, Fish and Koi use the same capability model with different configuration.

## 8. Security

### 8.1 SEC-01: Mermaid loose mode and disabled CSP

#### Location

- `src/workspaces/codez/Markdown.tsx:49-60`
- `src/workspaces/codez/Markdown.tsx:92-124`
- `src-tauri/tauri.conf.json:25-27`

Current Mermaid setup:

```ts
mermaid.initialize({
  startOnLoad: false,
  theme: "dark",
  securityLevel: "loose",
});
```

The generated SVG is inserted into the DOM via `innerHTML`, and the Tauri CSP is `null`. The normal Markdown HTML path already uses rehype-sanitize, which is good, but Mermaid-generated SVG goes through a separate path.

Content sources include model output, tool results, repository docs and extensions; not all can be treated as trusted.

#### Fix

- Switch Mermaid to `securityLevel: "strict"`.
- Use an SVG-aware sanitizer before inserting SVG into the DOM.
- Forbid event handler attributes, `javascript:`, `foreignObject`, external resources and unnecessary style/url.
- Configure an explicit CSP for Tauri, restricting at least script, connect, img, style and frame sources.
- Enumerate the connect-src entries needed for the LSP WebSocket, model APIs and bundled resources; do not open everything.
- Tauri commands must still validate paths and permissions; do not treat the WebView as a trusted boundary.

#### Tests

- Malicious Mermaid labels/links.
- SVG event attributes.
- `javascript:`/data URLs.
- Raw HTML, iframe, foreignObject.
- Malicious Markdown links in model output.

### 8.2 Markdown sanitizer caveats

`sanitizeSchema` allows span style and global className/id. Confirm that:

- style is only used for attributes KaTeX needs;
- `url()`, position overlays and other suspicious CSS are disallowed;
- ids cannot clobber application DOM;
- link clicks go through a protocol allowlist and uniformly get `rel="noopener noreferrer"`;
- local absolute-path links are checked against workspace scope.

## 9. Performance and Resource Management

### 9.1 PERF-01: build output size

Key results of this production build:

| Artifact | Size | gzip |
| --- | ---: | ---: |
| Main app `index-*.js` | ~2,011.65 KB | ~561.29 KB |
| Monaco `monaco-*.js` | ~3,830.98 KB | ~992.59 KB |
| TypeScript worker | ~7,020.26 KB | gzip not shown for workers |
| CSS worker | ~1,031.26 KB | gzip not shown for workers |
| HTML worker | ~690.01 KB | gzip not shown for workers |
| Markdown chunk | ~335.50 KB | ~101.86 KB |
| Cytoscape | ~442.57 KB | ~141.98 KB |
| Wardley | ~615.35 KB | ~148.57 KB |

The build transformed 4,106 modules and took about 2 min 29 s on the local machine.

### 9.2 Monaco loading strategy

#### Location

- `src/monaco-setup.ts:5-46`
- `vite.config.ts:20-29`

`monaco-setup.ts` imports full Monaco and the JSON/CSS/HTML/TS workers at top level. Recommendations:

- do not load Monaco before the IDE workspace is first opened;
- load contributions and workers per language on demand;
- do not start the TS worker until a JS/TS file is opened;
- evaluate whether read-only diff/preview needs the full language service;
- Extension UI, WorkZ and Settings should not load Monaco because of a shared entry;
- use route/workspace-level lazy boundaries.

### 9.3 Mermaid dynamic splitting ineffective

Vite reports that Mermaid is dynamically imported in `Markdown.tsx` but statically imported in `MarkdownPreview.tsx`, so it never actually becomes a separate dynamic chunk.

Fix: load it dynamically through the same `loadMermaid()` module from both entries and remove all top-level Mermaid imports.

### 9.4 Main bundle splitting

`vite.config.ts` currently has only three coarse manual chunks: app-vendor, monaco and markdown. Suggested lazy boundaries:

- CodeZ workspace;
- WorkZ workspace;
- Settings;
- Extension Manager;
- Browser Panel;
- Workflow Designer;
- Graph/Cytoscape;
- Markdown/Mermaid/KaTeX;
- Image Preview;
- advanced Git panel features.

Do not rely only on manualChunks to force-split third-party packages; prefer dynamic imports from React page/feature entries so unvisited features are not downloaded, parsed or executed.

### 9.5 File watcher and indexing

Existing strengths:

- the watcher filters ignored paths;
- the index worker has event coalescing and bounded-queue tests;
- UI refresh is reduced while the agent is busy;
- graph rebuild/patch is scheduled asynchronously.

Suggested additional metrics:

- watcher raw events, ignored, coalesced;
- paths per batch and number of rebuilds;
- index latency and failure rate;
- Git refresh count and duration;
- initial scan time when opening a large repo;
- watcher overflow and fallback rebuilds.

### 9.6 Performance budgets

Suggested CI additions:

- first-screen main bundle gzip budget;
- CodeZ first-load resource budget;
- Monaco ready time;
- LSP ready time;
- first keystroke response;
- AI completion TTFT/P95;
- large-file open and edit latency;
- 10k/100k-file workspace watcher/index benchmark;
- steady-state memory and leak checks after tab switching.

## 10. Extension Host

Current strengths:

- The Extension Host is built separately, so extensions are not mixed into the React UI main thread.
- Abstractions exist for RPC, workspace, SCM, language features, tasks, testing, debug, tree view, webview and more.
- Output retention already has boundary tests.

Risks and recommendations:

- Verification is mainly build/smoke; there are no VS Code API compatibility contract tests.
- Build a capability/compatibility matrix for every implemented API.
- Extension crashes, unbounded output, infinite loops, timed-out RPCs and repeated activate/deactivate need isolation tests.
- The Extension Host should have memory/CPU/message-size/call-rate limits.
- Webview, shell, filesystem and network permissions should be granted per extension declaration.
- RPC requests must support timeout, cancel, structured errors and host restart.
- After an Extension Host restart, restore installed extension state without re-registering commands/providers.
- When switching workspaces quickly, verify the old host no longer operates on the new workspace.

## 11. React and UI State Issues

ESLint reports 61 warnings; the following Hook warnings carry behavioral risk and should not be treated as mere formatting:

- `src/extensions/ui/QuickInput.tsx:17-20`: effect missing `inputBox?.value`; UI does not sync when the default value updates for the same input ID.
- `src/workspaces/codez/AssistantPanel.tsx:530`: callback missing `flushStreamDelta`; may capture stale stream-flush logic.
- `src/workspaces/codez/AssistantPanel.tsx:784-802`: callbacks missing `onModeChange`; may use stale translations/state.
- `src/workspaces/codez/CodeEditor.tsx:538`: handleMount missing `tab.content`, `tab.isReadOnly`, amplifying the stale-closure problems for LSP and breakpoints.
- `src/workspaces/codez/FileTree.tsx:255`: missing `node.is_dir`.
- `src/workspaces/codez/Terminal.tsx:159`: effect missing `active`.
- `src/workspaces/codez/index.tsx:595`: cleanup reads a ref that may have changed; capture the relevant collection/state inside the effect.
- `src/workspaces/codez/settings/WfBranchExprPaths.tsx:29`: memo missing `nodeLabel`.
- `src/workspaces/workz/index.tsx:355,821`: missing `setPreviewPath`.

How to handle:

- Do not just run a bulk `eslint --fix` and stop.
- For each warning decide whether the callback needs a ref, a useEvent-style wrapper or a real dependency.
- Add race/cancellation tests for effects involving subscriptions/async requests.
- CI can introduce a warning budget gradually and eventually promote `react-hooks/exhaustive-deps` to an error.

## 12. CSS-01: CSS syntax warning in production build

#### Location

- `src/extensions/ui/extensions.css:1-2`

The comment text contains:

```css
(--bg-*/--text-*/--border/--accent)
```

The `*/` closes the CSS comment early, and Vite/esbuild reports:

```text
Unexpected "*" [css-syntax-error]
```

#### Fix

Rewrite the text without a comment terminator, for example:

```css
/* Uses the --bg-, --text-, --border-, and --accent token families. */
```

Make production build warnings a CI failure so real syntax errors are not ignored long-term.

## 13. Testing Strategy

### 13.1 Current coverage strengths

Rust already covers:

- workflow validation/runtime;
- gateway message parsing;
- graph/index;
- path filtering;
- codebase index worker;
- terminal log bounds;
- skills provenance;
- part of the interactive UI schema;
- basic nested repo discovery.

### 13.2 Current key gaps

The frontend has only 5 test files and 21 tests, without adequate coverage of:

- Monaco multi-tab lifecycle;
- LSP client and WebSocket framing;
- provider URI matching;
- diagnostics routing;
- Git porcelain special paths;
- nested repo Git operations;
- GitPanel destructive flows;
- AI completion cancellation;
- inline edit conflicts and rollback;
- AssistantPanel streaming event ordering;
- Agent permission matrix;
- prompt injection;
- Markdown/Mermaid hostile input;
- Extension Host API contracts.

### 13.3 Test suites to add

#### LSP fake server integration tests

- immediate initialize response;
- delayed response;
- interleaved response/error/notification;
- non-ASCII payload;
- socket close;
- multiple documents;
- diagnostics per URI;
- reconnect and server crash.

#### Monaco integration tests

- unique URIs for two tabs;
- providers still valid after A → B → A;
- markers do not leak across files;
- close/reopen does not leak providers;
- external disk update conflicting with a dirty buffer.

#### Git fixture tests

- spaces, CJK, quotes, rename, copy, delete, untracked;
- staged + unstaged dual state;
- nested repo status/diff/add/reset/discard;
- explicit git_root conflicting with path;
- discard failure aggregation;
- unborn branch/first commit;
- detached HEAD.

#### Agent policy tests

- every registered tool must have capability metadata;
- decision matrix for profiles such as balanced/strict/auto;
- outside-workspace;
- shell/network/credentials/destructive;
- user declines confirmation;
- sub-agents cannot obtain write/exec capability;
- default deny when an MCP tool's capability mapping is missing.

#### Cancellation tests

- LLM request cancel;
- shell process tree cancel;
- hung tool timeout;
- collector close;
- no further token/tool events after UI cancel;
- session state recovers correctly after timeout.

#### Security tests

- malicious Markdown/HTML/SVG/Mermaid;
- link protocols;
- WebView → Tauri invoke scope;
- extension webview messages;
- user tool manifest path traversal;
- prompt injection attempting to enable disabled tools.

## 14. Recommended Refactoring Boundaries

### 14.1 `CodeEditor.tsx`

Split into:

```text
CodeEditorView
useMonacoModel
useLspDocument
useAiInlineCompletion
useInlineEditPreview
useBreakpointGutter
useEditorCommands
```

### 14.2 `src/services/tauri/lsp.ts`

Split into:

```text
lsp/protocol.ts       JSON-RPC types and framing
lsp/client.ts         WebSocket, pending, cancel, reconnect
lsp/documents.ts      document lifecycle/version
lsp/converters.ts     LSP ↔ Monaco conversion
lsp/providers.ts      Monaco provider registration
lsp/sessionPool.ts    project/language reuse
```

### 14.3 `src-tauri/src/commands/ide.rs`

Split into:

```text
commands/ide/files.rs
commands/ide/git.rs
commands/ide/terminal.rs
commands/ide/watcher.rs
commands/ide/lsp.rs
```

### 14.4 Frontend workspaces

For `codez/index.tsx` and `AssistantPanel.tsx`, separate the watcher, tab model, session stream and Git refresh scheduler into dedicated stores/hooks, so one component does not handle persistence, subscriptions, scheduling and rendering at once.

## 15. Phased Implementation Plan

### Phase 1: correctness and security, address immediately

1. Fix the Monaco model URI and remove the first-model fallback for diagnostics.
2. Move the LSP lifecycle out of onMount and support multiple documents.
3. Fix the initialize race, UTF-8 Content-Length and pending rejection.
4. Disable `bypass_permissions` for the main Agent and add minimal permission regression tests.
5. Use Mermaid strict mode, enable CSP and sanitize SVG.
6. Switch Git status to `-z` byte parsing.
7. Fix nested repo diff/path and the ignored git_root.
8. Fix the comment syntax in `extensions.css`.

Phase 1 acceptance gates:

- all new regression tests pass;
- production build has no warnings;
- manual smoke tests with two files, multiple languages and multiple repos pass;
- Agent file writes / command execution trigger confirmation or denial according to policy;
- malicious Mermaid/Markdown tests cannot execute scripts or invoke beyond scope.

### Phase 2: core experience stabilization

1. Project/language LSP session pool.
2. Real AI completion cancellation, concurrency limit, generation guard.
3. Inline edit inverse-edit rollback and conflict handling.
4. Git operation queue, staged/worktree diff, structured errors.
5. Split `chat_turn.rs`, `CodeEditor.tsx` and the CodeZ workspace.
6. Add core integration tests to CI.
7. Clear all React Hook warnings.

### Phase 3: performance and productization

1. Route-level lazy loading for WorkZ/CodeZ/Settings/Extensions/Graph/Browser.
2. Per-language loading of Monaco contributions and workers.
3. Single-path dynamic loading of Mermaid.
4. Bundle, startup, LSP and completion performance budgets in CI.
5. Extension Host compatibility suite and resource limits.
6. Tool capability metadata, transactional rollback and full audit logging.

## 16. Suggested Definition of Done

Every core fix PR should at least:

- include matching automated regression tests, not only manual verification;
- describe the impact on Windows, macOS and Linux path and process behavior;
- introduce no new lint/type/build warnings;
- have explicit permission decisions for destructive or external-side-effect behavior;
- include cancellation, timeout and stale-response handling for async logic;
- provide before/after metrics for performance-sensitive changes;
- update related architecture docs or code comments;
- verify multi-tab, multi-repo, large-file and project-switch scenarios.

## 17. Final Conclusion

AgentZ's core strength is that it already has most of the building blocks a real AI IDE needs, rather than simply adding a chat box next to an editor. The main problem right now is not missing features but that the lifecycle, protocol correctness and permission model of some critical paths are not yet fully closed.

The highest-return order of work: first fix LSP URI/lifecycle/protocol, then Agent permissions and Git path parsing while tightening Mermaid/CSP; then fill in integration tests and cancellation; finally do module splitting and load-performance optimization. Once Phase 1 is done, editor intelligence, multi-repo Git reliability and Agent safety will all improve significantly, establishing a stable baseline for further feature work.
