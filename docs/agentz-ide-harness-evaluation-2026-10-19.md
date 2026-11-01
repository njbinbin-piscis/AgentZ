# AgentZ System Evaluation: Professional IDE Perspective × Agent Harness Completeness

**English** | [简体中文](agentz-ide-harness-evaluation-2026-10-19.zh-CN.md)

> **Status (2026-11-02, AgentZ 1.0.40, kernel piscis-engine v0.8.71):** This is a dated evaluation of 0.9.19; its conclusions are kept as written. Since then the ledger (`docs/upgrade/plan.json`) has moved from 37 to **65 of 83 items `done`**, with the remaining 18 `deferred` (no item is `preparing/pending/blocked` any more). Marked `done` since this report: AZ-026, 031, 032, 036, 037, 041, 043, 044, 046, 053, 056, 057, 059, 060, 062, 063. Still open (`deferred`): AZ-045 (remote reconnect), 051/052 (connector OAuth, external sends), 054 (desktop automation), 064/065 (signing, trusted updates), 070/071/075 (team isolation, team budget, multi-agent evaluation), 077/078 (macOS/Linux credential stores). A ledger `done` was not re-verified natively for this note. Code-level changes: `chat_turn.rs` is now a `src-tauri/src/commands/chat_turn/` module (4 files, ~3.7k lines); the CodeZ/WorkZ/AssistantPanel components were split (see §6 status notes); `release.yml` now gates every package job on `release-readiness`. Size figures in the body are as of 2026-10-19 unless marked.

| Item | Details |
|---|---|
| Evaluation date | 2026-10-19 |
| Subject | AgentZ desktop `0.9.19` (`main`, `1b17637 release: 0.9.19 symbol graph, DeepWiki, and anchored repo memory`) + pinned kernel `piscis-engine v0.8.68` (audited overrides/patches under `vendor/piscis-engine`) |
| Stance | Professional IDE developer (editor / language services / debugging / extensions / distribution) + Agent Harness engineering (execution loop / tools / context / permissions / recovery / evaluation) |
| Evidence sources | Source inventory (`src/`, `src-tauri/`, `crates/agentz-task-state`, `crates/agentz-host`, `extension-host/`, `vendor/piscis-engine/`), the 83-item ledger in `docs/upgrade/plan.json|tasks.md|progress.md`, `docs/upgrade/reassessment-2026-10-13.md`, `docs/agentz-product-maturity-review-2026-10-04.md`, `docs/ai-ide-engineering-audit.md`, `docs/upgrade/acceptance-0.9.12.md` (46 unaccepted items), 0.9.13–0.9.19 release notes, CI configuration |
| Evidence boundary | **This report is a static review plus re-examination of ledger evidence, not a new hands-on acceptance run.** This round did not re-run the full `cargo test --workspace`, a production build, long real-model tasks, native GUI journeys, real macOS/Linux machines, or signed installers. Anything the ledger marks `current_source_read_only_no_tests_rerun`, `native_unverified`, or `unreviewed candidate` is never promoted to "verified" here. Conclusions distinguish three levels: **implemented (source exists) ≠ self-tested (unit/integration tests pass) ≠ accepted (native/real-machine/real-model evidence)**. |

---

## 0. Executive summary (one-page conclusion)

### 0.1 Overall judgement

> **AgentZ has moved past the "chat box + tool calls" prototype stage and has become an unusually broad prototype of an AI-native desktop workbench; but by the delivery standards of a professional IDE it is still a "developer preview / controlled beta" product, not a 1.0-class IDE you can trust with long tasks or advertise as stably supported.**

The dividing line is not the number of features but **how closed the contracts are**: task ↔ environment binding, completeness of change review, actual enforcement of permissions, human–agent handoff, crash recovery, cross-platform support and distribution. These "last mile" contracts are exactly where the ledger's many `preparing / review / blocked / deferred` items cluster (only 37 of 83 items `done`, and several `done` items were limited to `bounded_source_review` in the 2026-10-13 reassessment and need re-running on current source).

> *Status (2026-11-02):* the ledger now shows 65 `done` / 18 `deferred`; the open items are concentrated in distribution, remote, connectors and multi-agent (see the status note at the top).

### 0.2 Two-axis scores (out of 10; IDE engineering convention: 7 = usable for your own daily work, 8.5 = can be rolled out to a team, 9+ = mature commercial IDE)

| Dimension | Score | One-line rationale |
|---|---:|---|
| **IDE shell and editor** (Monaco/files/tabs/layout) | 6.5 | Shell is complete and the dual workbench (CodeZ/WorkZ) works; single-WebView rendering, giant components and the unresolved dirty-buffer conflict (AZ-041) drag down daily reliability |
| **Language intelligence** (LSP/completion/navigation/symbol graph) | 6.0 | LSP bridge + crash recovery (AZ-042 done) + tree-sitter symbol graph/CodeMap are highlights; real language-server journeys, incremental sync and the extension language ecosystem are not accepted |
| **Debugging and terminal** (DAP/PTY) | 5.5 | DAP broker and PTY lifecycle code are in place; AZ-043/AZ-044 both `preparing`, no native recordings or multi-terminal crosstalk tests |
| **Git and change review** | 7.0 | M1 fixed the F02/F03-class defects: complete ChangeSet (committed/staged/unstaged/untracked), journal routed by environment, archive snapshots and merge pre-checks (AZ-011~016 done); but native review-UI checks still rely on waivers |
| **Extension ecosystem** (VSIX/Extension Host/Webview) | 5.0 | Node extension host + VSIX contribution-point ingestion are architecturally sound and rare; Webview isolation AZ-032 still `review`, compatibility profiles for 6 real extensions AZ-046 `deferred` — "installs" ≠ "compatible" |
| **Remote and browser/desktop automation** | 4.5 | SSH/VFS/port forwarding/remote LSP and a CDP browser panel are implemented; AZ-045 remote disconnect `blocked` (no real SSH), AZ-053 browser preemption `pending`, AZ-054 desktop operation `not_started` |
| **Agent Harness execution loop** | 7.5 | The system's strongest area: scenario-based harnesses (main/koi/fish/scheduler/debug/headless), layered budgets and three-level compaction, retry classification, streaming, pluggable plan/memory/hooks; design completeness close to mature agent frameworks |
| **Tool system** | 7.0 | 20+ kernel-neutral tools, IDE-coupled tools, MCP, user tools and connectors are all there; but the tool surface is very large, and alignment of some high-risk tools (email/ssh/elevate, etc.) with permission enforcement relies on item-by-item backstops in AZ-028/034 |
| **Context engineering** | 7.0 | LayeredPrompt, StateFrame, rolling summaries, micro/auto/full three-level compaction, graph/RepoWiki/anchored memory injection — modern approach; no published long-session degradation curves or evaluation (AZ-062 pending) |
| **Permissions and security** | 6.0 | Four-level permission contract, persistent approval intents, Windows AppContainer reference policy, DPAPI, fail-closed encryption (AZ-027~030, 034 done) are real investment; full-egress secret redaction AZ-031 `blocked`, macOS/Linux credentials AZ-077/078 `blocked`, Webview isolation untested — a P1 risk cluster |
| **Persistence, recovery and budget** | 7.0 | Run/Attempt state machine, durable events + stream-gap cursors, startup reconciliation, closed-loop cancellation, task-level budget reservation (AZ-017~024 done) exceed most comparable products; but AZ-026 kernel contract version still `preparing`, and the budget does not cover every model entry point (title/memory/curator side paths) |
| **Multi-agent and workflows** | 5.0 | Pool/Koi/Fish/Team/Workflow concepts are complete, even ahead of their time; member environment isolation AZ-070 and team budget AZ-071 are `deferred/partial`, no comparison data for multi-agent benefit AZ-075 — **today there is "multi-agent UI and scheduling", not evidence that "multi-agent is more reliable"** |
| **Observability and evaluation** | 5.0 | Event sourcing, diagnostic export candidate, AZ-004's 12 tasks × 3 real-kernel baseline (automatic 33/36, semantic review 36/36) are a good foundation; trend gating AZ-062 and diagnostic correlation AZ-061 both `pending`, no continuous measurement of real desktop task success rate |
| **Performance and resources** | 4.5 | Point optimisations such as `path_filter`, refresh coalescing and perfCounters; the main chunk once reached ~5.95MB (unminified build), AZ-059 first paint / AZ-060 long-task resources both `blocked/pending`, no cold-start or 2-hour soak baseline |
| **Testing and quality gates** | 5.5 | Frontend ~45 files / 310 tests (bounded concurrency, 2026-10-13), 152+2 task-state domain tests and 350+ kernel tests are respectable; but the core-lifecycle E2E gate AZ-056 is `blocked`, CI is Ubuntu-only, and default-concurrency Vitest once had worker exits — **the gates exist but are not closed** |
| **Distribution and cross-platform** | 3.5 | Lowest score overall: unsigned and unnotarised, no trusted updates (AZ-065 `deferred`), data migration/backup-restore AZ-063 `pending`, macOS/Linux never accepted on real machines (AZ-058 `blocked`). Distribution today is effectively "unsigned Windows installer + disclaimer" |
| **Product information architecture (CodeZ/WorkZ dual workbench)** | 7.0 | The "workbench you operate / workbench you delegate to" dual view + same-task handoff is a real differentiator from single-view Cursor-like products; but the handoff itself (AZ-036 `preparing`, AZ-037 `deferred`) has not yet been delivered as a closed loop |

**Overall: IDE completeness ≈ 5.9/10, Agent Harness completeness ≈ 6.7/10, product deliverability (to external users) ≈ 4.5/10.** The gap between the three is itself the diagnosis: **kernel and harness design runs about one major version ahead of product acceptance, and release engineering lags about two major versions behind.**

> *Status (2026-11-02, ledger only):* AZ-026, 031, 032, 036, 037, 041, 043, 044, 046, 053, 056, 057, 059, 060, 062 and 063 are now `done`; AZ-045, 054, 064, 065, 070, 071, 075, 077, 078 remain `deferred`. CI is still Ubuntu-only in `ci.yml`, with a separate Windows `candidate.yml` workflow. Scores are not re-rated here.

### 0.3 Three sentences for decision makers

1. **Stop adding new feature surfaces.** Browser, desktop operation, teams, gateway channels (8+ channels such as Feishu/WeChat/Slack), marketplace and skill evolution already spread far wider than acceptance capacity; all the marginal return in the next cycle is in "closing the contracts".
2. **1.0 should be defined by acceptance events, not a version number.** From 0.9.7 to 0.9.19, 13 versions shipped in two weeks, and from 0.9.12 onward several consecutive versions carried a "did not pass the release acceptance gate" disclaimer — the release cadence and the acceptance cadence have decoupled. Recommendation: freeze features and use AZ-056 (E2E gate) + AZ-057 (Windows long-run stability) + AZ-063/064 (migration/signing) as the 1.0 gate.
3. **The most valuable differentiator is "long tasks you can trust", not "more agents".** Durable execution (AZ-017~025) is already unusually deep among peers; polish the reliable single-agent loop into the flagship before talking about multi-agent benefit (AZ-075).

> *Status (2026-11-02):* the product shipped as 1.0.x with AZ-056/057/063 marked `done` in the ledger, but AZ-064 (signing) is still `deferred`, so the recommended 1.0 gate was only partially met.

---

## 1. System overview (scope of evaluation)

### 1.1 Architecture

```
┌──────────────────────────── Single Tauri WebView (React 18 + TS) ───────────────────────┐
│  App.tsx (47KB, global shell)                                                            │
│  ├─ CodeZ workbench: Monaco editor / file tree / Git / search / terminal (xterm) / LSP   │
│  │   client / DAP debug view / browser panel / extension UI (Webview) / CodeMap          │
│  │   (reactflow+elk) / settings / workflow designer                                       │
│  └─ WorkZ workbench: session tasks / Pool team board / workflow runs / artifact drawer / │
│      delivery review                                                                      │
│  Service layer: src/services/tauri/* (72 files, per-domain invoke wrappers + pure-fn     │
│  unit tests)                                                                              │
└──────────────────────────────────────┬───────────────────────────────────────────────────┘
                                       │ Tauri invoke (~300 commands, lib.rs generate_handler)
┌──────────────────────────────────────▼───────────────────────────────────────────────────┐
│  Rust host (src-tauri, Tauri 2)                                                            │
│  ├─ commands/ (68 files): chat/chat_turn (3.2k lines)/agent_task/ide/session/workflow/pool…│
│  ├─ runtime/: durable_run / event_replay / approval / owner_lease / task_budget / koi …    │
│  ├─ crates/agentz-task-state: Run/Attempt state machine · budget reservation · approvals · │
│  │   artifacts · archive · external-action idempotency (SQLite)                           │
│  ├─ Infrastructure: LSP manager / DAP broker / PTY (portable-pty) / watcher               │
│  │   (notify+path_filter) / remote (SSH/VFS/forwarding/devcontainer) / browser            │
│  │   (chromiumoxide CDP) / gateway (8+ IM channels)                                       │
│  └─ tools/: ~18 IDE-coupled tools (LSP/graph/symbols/repo memory/chat_ui/plan UI/delegate…)│
└──────────────────────────────────────┬───────────────────────────────────────────────────┘
        ┌──────────────────────────────┼──────────────────────────────┐
        ▼                              ▼                              ▼
┌─ piscis-engine v0.8.68 ─┐  ┌─ Node Extension Host ─┐   ┌─ External surfaces ──┐
│ (vendored, pinned +      │  │ VS Code API compat     │   │ MCP servers /         │
│  audited patches)        │  │ layer (apiFactory/     │   │ connectors (OAuth) /  │
│ piscis-kernel: AgentLoop │  │ mainThreads/Webview/   │   │ ClawHub / cloud       │
│ (agent/loop_.rs ~313KB!) │  │ Debug/Notebook, stdio  │   │ marketplace / RobotZ  │
│ Harness config · tool    │  │ JSON-RPC)              │   │ browser (git-pinned   │
│ registry · PolicyGate ·  │  └────────────────────────┘   │ v0.1.13)              │
│ LLM client · 20+ neutral │                               └───────────────────────┘
│ tools · Pool coordinator │
└──────────────────────────┘
```

> *Status (2026-11-02):* `src/App.tsx` is now ~58KB / 1,496 lines; `src/services/tauri/` has 75 files; `src-tauri/src/commands/` has 74 files and `chat_turn` is a directory module (`mod.rs`, `post_turn.rs`, `tool_approval.rs`, …; ~3.7k lines total); the kernel loop is at `vendor/piscis-engine/piscis-kernel/src/agent/loop_.rs` (~332KB / 7.5k lines); the pinned kernel is v0.8.71.

### 1.2 Sense of scale (inventory basis, not a precise LOC audit)

- Frontend: hundreds of files under `src/`, including ~45 Vitest test files; `App.tsx` alone is 47KB, `chat_turn.rs` 3,249 lines, kernel `loop_.rs` about 313KB — **these three "god files" are the main maintainability tax** (see §4.6). *(2026-11-02: 69 test files; `chat_turn` split into a module; `App.tsx` and `loop_.rs` have grown.)*
- Host command surface: `lib.rs` registers about 300 Tauri commands covering files/Git/terminal/LSP/DAP/sessions/tasks/pool/workflows/marketplace/gateway/skill evolution — a command surface this wide is rare among peers, and it also means **an equally wide attack surface and regression surface**.
- Preinstalled assets: `bundled/preinstall/` contains a large set of agents/skills/commands imported from CodeBuddy — rich content supply, but it also brings governance burden around provenance, versions and permission declarations (the existing `verify:preinstall / lint:preinstall --strict` entry points are a good start).

### 1.3 Product positioning

The README's "Cursor-like AI IDE" analogy makes the entry point easy to grasp but undersells the product: AgentZ is actually a four-in-one of **IDE (CodeZ) + task delegation desk (WorkZ) + multi-agent/workflow runtime + extension/skill/connector platform**. The ambition is reasonable (it tracks the convergence of Cursor and Codex/Devin-style delegation), but the four product lines share the same acceptance bandwidth, which is the structural reason for today's "implemented everywhere, accepted nowhere".

---

## 2. IDE capability evaluation (professional IDE developer perspective)

Benchmark: **everyday engineering usability** of the VS Code / Cursor / JetBrains family — not "is there a panel for it" but "is it trustworthy on a 100k-line repository, with a dirty working tree, crash recovery and concurrent tabs".

### 2.1 Editor core and document model 【6.5/10】

**In place:** Monaco integration (including `@monaco-editor/react`, themes and VS Code theme conversion, Markdown/Mermaid/KaTeX rendering), multiple tabs (`EditorTabs`, tabNavigation has unit tests), editor hot-exit recovery (`editorRecovery` has an integration test), saving unsaved buffers, inline edit (Cmd-K) and AI ghost-text completion, image preview, a Notebook (`.ipynb`) editor.

**Not closed (ordered by impact):**

1. **Dirty buffer × agent/external write conflict (AZ-041, `preparing`)** — the "constitutional question" of an IDE: when an agent writes a file while the user has unsaved edits to the same file open, who wins, how is it surfaced, and what is the on-disk base? Today there is only the `editorConflict.ts` helper with unit tests; the interleaving of typing/save/watcher/recovery has not been re-run on current source. **Until this is closed, the core "human and agent editing the same repo" selling point carries data-loss risk and should be stated explicitly in the UI.** — *Status (2026-11-02): AZ-041 is `done` in the ledger; `src/workspaces/codez/editorConflict.ts` + `editorConflict.test.ts` exist. Not natively re-verified here.*
2. **Single-WebView rendering architecture:** the editor, chat stream, Markdown, graph layout and extension RPC share one render thread (acknowledged in `docs/performance.md`). Long sessions have no virtualised list (only a light `content-visibility` optimisation), and CodeMap for large repositories runs reactflow+elk layout on the main thread — the jank ceiling is set by the heaviest piece. This is an **architecture-level gap** versus Electron's multi-process model (Cursor/VS Code); lazy loading and chunk splitting can only mitigate it (AZ-059). — *Open (architectural).*
3. Giant components such as `App.tsx`, `codez/index.tsx` and `AssistantPanel.tsx` couple editor state (tabs, watcher, Git refresh, layout) into one state tree; the repeated refresh on focus change (0.9.12 UT-R03) is a symptom of that coupling. — *Partially fixed (2026-11-02): `codez/index.tsx` is now ~24KB (logic moved to hooks such as `useCodezWorkspace.ts`, `useCodezShortcuts.ts`) and `AssistantPanel.tsx` ~12KB; `App.tsx` grew to ~58KB.*

### 2.2 Language intelligence 【6.0/10】

**In place:** LSP bridge (WebSocket ↔ language server), multi-language management, Monaco provider wiring for diagnostics/completion/navigation; **LSP crash recovery AZ-042 `done`** (recovery budget, reset after reconnect, 10/10 multi-round crash regressions) is a detail beyond most home-grown IDEs; tree-sitter (Rust/TS/JS/Python/Go, etc.) drives the symbol graph, call graph, CodeMap and 0.9.19's DeepWiki/Repo Memory, forming a "structural understanding layer" that directly feeds the agent's `graph_search / symbols / codebase_search` tools — **this is a real differentiator versus a pure-LSP approach: the graph serves both humans and the agent.**

**Not closed:** LSP `didChange` still sends the full document (acknowledged in `docs/performance.md`, 300ms debounce); the LSP-01~04 class defects from the 2026-09 audit (URI consistency, reconnect on tab switch, initialize race, Content-Length byte length) need to be filed with explicit evidence from real TS/Rust dual-server journeys on the current version, not just code reading (AZ-082 fixed LSP return-path filtering, but live LSP is tested separately); language coverage depends on users installing language servers/extensions themselves, and there is no acceptance of an "out-of-the-box TS/Python experience". — *Not re-checked (2026-11-02).*

### 2.3 Debugging (DAP) 【5.5/10】

**In place:** a standalone DAP broker (`dap.rs`/`dap_frame.rs`), debug controller and breakpoint store (`src/extensions/debug/`, with `dapClient.test.ts` and `debugController.test.ts`), debug view UI.

**Not closed:** AZ-043 `preparing` — launch/attach, breakpoint hits, no leftover processes after abnormal exit, jumping to files in the **correct task environment**: none has native evidence. Debugging is the IDE feature most dependent on correct process trees and path mapping; until AZ-036 (CodeZ opens the task environment) is closed, "environment correctness" for debugging cannot be claimed. For professional users, **an unaccepted debugger is more dangerous than no debugger** (breakpoints in the wrong environment, leftover processes). — *Status (2026-11-02): AZ-043 and AZ-036 are `done` in the ledger.*

### 2.4 Integrated terminal 【5.5/10】

**In place:** portable-pty multi-terminal, xterm + fit/web-links add-ons, terminal lifecycle logic (`terminalLifecycle.ts` has unit tests), a resource snapshot command (`ide_terminal_resource_snapshot`), process reaping on close and the kernel Job Object tree-termination contract (AZ-022 verified on Windows with a real two-level cmd tree — hard evidence).

**Not closed:** AZ-044 `preparing` — cwd crosstalk across multiple terminals × task switching and PTY reclamation after closing a task need native cases re-run on the current Job/terminal changes. Binding terminal cwd to the task environment is likewise gated by AZ-036. — *Status (2026-11-02): AZ-044 `done` in the ledger.*

### 2.5 Git and change review 【7.0/10, the most mature IDE-side area】

The M1 milestone (gate passed) did fix the F02/F03/F04/F05 group from the 2026-10 review: complete ChangeSet collection (four change kinds + NUL paths + renames/binaries/structured errors, AZ-011), journal routed by task environment (AZ-012), archive snapshots (separate index + original blobs + reachable ref, AZ-014), constrained cleanup (rejects cross-task paths, step failures visible, AZ-015), merge transaction pre-checks (repository mutex, frozen-HEAD authorisation, run lease, AZ-016), and a review UI that distinguishes change sources and read failures (AZ-013).

**Residual risk:** the `done` items above are all annotated `bounded_source_review` in the reassessment and were not re-run item by item on current source; **native GUI checks** of review and archive are waived by ADR-007/008 rather than tested. Engineering judgement: **backend contract confidence is high; frontend presentation confidence could be raised substantially by a single native A/B two-task journey** — the most cost-effective acceptance investment in the whole system (and it unlocks half of AZ-056).

### 2.6 Search, navigation and code understanding 【7.0/10】

File search / global search panels, command-palette-style interaction, symbol lookup and call graph (`symbol_find / symbol_call_graph`), CodeMap graph layout, codebase indexing and semantic search (`codebase_index_build/search`), RepoWiki/DeepWiki generation — two-channel navigation for humans and agents is complete. The risk points are index freshness and large-repository performance: peak memory of the index worker and graph build, and reclamation on close, are within AZ-060's unaccepted scope; the exclusion rules of `path_filter` (consistency between host pre-check and kernel traversal, AZ-081 done) are carefully done and deserve credit. — *Status (2026-11-02): a user-facing `CommandPalette` (`src/workspaces/codez/CommandPalette.tsx`, Ctrl/Cmd+P files, Ctrl/Cmd+Shift+P commands) exists; AZ-060 `done` in the ledger.*

### 2.7 Extension system 【5.0/10】

**The architecture is sound and the ambition correct:** a Node sidecar extension host (stdio JSON-RPC, `apiFactory/mainThreads` bridge, Webview/DAP/Notebook/Debug contribution points), VSIX ingestion and install management, compat-layer DTOs/type converters, an extension UI store (with unit tests). This route (compatibility with a subset of the VS Code extension API) is required coursework for every AI IDE after Cursor, and AgentZ did not start late.

**But compatibility is an evidence business, not an API-list business:**

- AZ-032 Webview isolation still `review`: the CSP has been tightened to `script-src 'self'` with a controlled `agentz-resource:` protocol, per-panel tokens and a Rust path allowlist (code-level evidence), but **positive and negative tests in real Tauri for "renders ∧ cannot reach host DOM/IPC/other panels" have not been done**. Webviews are the IDE's number-one RCE surface; until this is closed, distribution claims for third-party Webview extensions should be restricted. — *Status (2026-11-02): AZ-032 `done` in the ledger; the CSP in `src-tauri/tauri.conf.json` still matches the description above; `src/extensions/ui/webviewSecurity.ts` has tests.*
- AZ-046 `deferred`: the install-run-upgrade matrix for 6 real extensions (language/formatter/Git/debug kinds) is empty. **Any "compatible" claim about extensions today is an unverified claim**; the support matrix should explicitly label the `unverified` tier (AZ-005 defined the tiers; AZ-067 is to apply them to release docs). — *Status (2026-11-02): AZ-046 `done` in the ledger.*

### 2.8 Remote development 【4.5/10】

SSH target management, key configuration, remote FS/VFS, remote LSP, port forwarding, devcontainer, extension sync — the surface is fully laid out. But AZ-045 is `blocked`: disconnect/reconnect, rejection on host-key change and zero duplicate writes after reconnect have no real SSH evidence. The failure modes of remote development (network partition, half-open connections, leftover remote processes) cannot be derived from local code reading; **this area can only be labelled "implementation candidate", not "supported"**. — *Open (2026-11-02): AZ-045 `deferred`.*

### 2.9 Browser and desktop automation 【4.0/10】

The built-in CDP browser panel (navigate/screenshot/click/viewport/scroll) + RobotZ `browser` tool (git-pinned `v0.1.13`) is practical for agent web tasks; but the fixes for the issues AZ-053 identifies — generational ref invalidation (old element refs must be explicitly invalidated after navigation) and the agent stopping when the user takes over — exist only in the RobotZ sibling repository's working tree and are **not pinned into AgentZ's dependencies**. This is a classic "demo works, delegation fails" gap. AZ-054 Windows desktop operation (UIA) `not_started` and AZ-055 cross-task foreground resource leases `deferred`: until focus identity and preemption gating are designed, **no desktop auto-operation capability should be enabled**; the ledger's "better nothing than unsafe" judgement is correct. — *Status (2026-11-02): AZ-053 `done` in the ledger (RobotZ pin not re-checked here); AZ-054 still open (`deferred`).*

### 2.10 Interaction, usability and accessibility 【5.5/10】

Bilingual Chinese/English i18n (with semantic consistency tests), keyboard paths (mode-switch shortcuts, BottomPanel keyboard tests), first-run onboarding candidates (AZ-040/AZ-066) and a notification inbox candidate (AZ-073) all show investment. But AZ-047's overall walkthrough ("complete the core journey by keyboard + every error includes a next action + Chinese/English consistency") has not been done; AZ-040's 5-person first-use walkthrough and AZ-066's 5 clean-machine first tasks have zero records. For an IDE, **usability is not a nice-to-have: whether approval cards, permission prompts and conflict dialogs are understandable directly decides whether users click "allow all" and thereby hollow out the security model**.

---

## 3. Agent Harness completeness (core chapter)

The harness is the whole engineering layer wrapping the LLM: execution loop, context assembly, tool dispatch, policy gating, state persistence, recovery and budget. **This is the part of AgentZ that most deserves serious praise, and it is the dividing line from "chat-wrapper IDEs".**

### 3.1 Execution loop and scenario system 【7.5/10】

The kernel assembles the execution environment uniformly via `HarnessConfig` and provides scenario factories: `for_main_chat / for_main_headless / for_koi (sub-agent) / for_fish (stateless short task) / for_scheduler / for_debug`. Each scenario explicitly declares: whether it persists, confirmation flags, summary/state-frame wiring, plan state, streaming, vision delegation. This "**scenario as configuration, capabilities wired on demand**" design avoids the decay path most products take ("one loop for everything, `if`s everywhere").

The loop itself (`loop_.rs`) has: SSE streaming passthrough, a fallback model chain, same-model transient retry (configurable, reduced to 1 in gateway scenarios), cancellation-interruptible backoff (AZ-023 verified with a local HTTP fault matrix: 401/403 and quota = permanent, 5xx = transient, 429 = rate-limited, with upper/lower-bound assertions on backoff cancellation), tool result receipts (`tool_receipt.rs`), and preventing the model from exiting on its own while the plan is unfinished. Plug points are complete: `LoopStrategy`, `CompactionStrategy`, `ContextManager`, `MemoryPlugin`, lifecycle hooks (the host hangs its file journal on these).

**Deductions:** `loop_.rs` as a ~313KB single file is a kernel-level maintenance risk — reviewing any harness behaviour, locating regressions and making secondary patches (the current vendor-patch model) are all expensive; and AZ-026 "publish and adopt the kernel contract version" is still `preparing`, meaning **the mapping between the kernel version run by the desktop app and the contract has not yet been formally frozen and re-checked**. Harness design 8, kernel engineering 6.5, combined 7.5. — *Status (2026-11-02): AZ-026 `done` in the ledger; `agent/loop_.rs` is still a single file (~332KB / 7,522 lines) — open.*

**Compared with the mainstream:** a scenario-based harness + pluggable compaction/memory/loop strategy is no less complete in design than what is publicly observable of Claude Code / Codex-class products; the gap is in **evidence at scale** (see §3.7 on evaluation).

### 3.2 Context engineering 【7.0/10】

- **Layered prompts:** `LayeredPrompt` (L0…Lhint) replaces a single system prompt block; the host then assembles agent/plan-mode/todo/skill context (`system_prompt.rs`).
- **Layered budget and three-level compaction:** `LayeredBudget` derives the total from the context window, with three thresholds (default micro 60% / auto 80% / full 95%) + a per-tool-result token cap + an optional summary model; compaction strategies are pluggable, and there are `compaction_eval.rs / bench_compact.rs` — **building evaluation and benchmarks for compaction is rare among peers and deserves explicit credit.**
- **State frames and rolling summaries:** `StateFrameProvider` and `SummaryStore` (p6/p7 design) let long sessions survive on structured state rather than a plain text sliding window.
- **Repository understanding injection:** symbol graph, CodeMap, RepoWiki/anchored repository memory (`repo_memory` tool, 0.9.19) give the agent project-level priors; the direction is right (same race as Cursor's codebase indexing and Codex's repo map).

**Not closed:** the actual degradation curve of long sessions (after how many turns task success decays) and the rate at which key constraints are lost after compaction have no continuous evaluation (AZ-062); there is no user-facing visualisation of token attribution in context assembly (how much each layer consumes) — `ContextUsageRing` is only a total ring. **Context engineering: "mechanisms complete, measurement absent".** — *Status (2026-11-02): AZ-062 `done` in the ledger.*

### 3.3 Tool system 【7.0/10】

- **Kernel-neutral tools (20+):** file read/write/edit/diff/search/list, shell, code_run, process control, web_search/web_fetch, email, ssh, pdf, vision_context, memory_store/recall, plan_todo/plan_write, pool_org/pool_chat, dynamically loaded user JSON tools — all registered by switches via `NeutralToolsConfig`, with desktop and CLI sharing the same registration path; a clean portability design.
- **Host-coupled tools (~18):** LSP diagnostics, codebase search, graph explore/search, symbols, terminal read, repository memory, chat_ui interactive cards, plan-mode UI, delegate, remote FS, skill management, settings catalogue, etc. — AgentZ's differentiating tool band (especially the graph trio and chat_ui structured interaction).
- **Extension channels:** dynamic MCP tool registration, connectors (OAuth services exposed as MCP tools), user tools, skills and sub-agent personas (Koi/Fish/agent packages).

**Risks and deductions:**

1. **The tool surface is too large and high-risk tools are broadly available by default:** when `shell / code_run / process_control / elevate / ssh / email / web_*` coexist, security rests entirely on actual enforcement by PolicyGate and approvals (see §3.4). **Least-privilege trimming of tools per scenario** (Plan mode already disables write tools via `PLAN_MODE_DISABLED`, a good example — now in `src-tauri/src/commands/chat_turn/tool_approval.rs`) should be extended to all scenarios and sub-agents.
2. Tool contract versioning is not frozen (AZ-026); schema drift between kernel tools and host tools relies on manual review; kernel defects such as same-turn repeated-edit Undo have to go through the vendor patch process (AZ-079 went through it once; the process cost is visible).
3. **Structuring of tool results** is uneven: recovery_v1's structured outcome (AZ-022) should become the unified receipt standard for all side-effecting tools (merging with `tool_receipt.rs`); today it is met only in spots.

### 3.4 Policy, permissions and sandboxing 【6.0/10】

**Contract layer (strong):** AZ-027 froze a scope/duration/revocation/escalation matrix of four authorisation levels × six resource classes (files/processes/network/browser/desktop/connectors); AZ-028 persistent approval intents are bound to an argument digest + task + environment + expiry, old authorisations are voided on restart and changed arguments require re-approval (17 host integration + 18 domain tests pass); post-approval path substitution (junction TOCTOU) re-check AZ-080, kernel traversal exclusion of directories AZ-081, LSP return-path filtering AZ-082 — **this set of "the world changes after approval" defences shows the team really understands that the hard part of agent permissions is timing, not the dialog.**

**Enforcement layer (medium):** the Windows AppContainer reference policy (AZ-034) implements workspace ACLs, no-network/out-of-bounds rejection, Job timeout cancellation and reaping, with automated coverage — but Windows only, and only wired into `code_run/shell`; the honest statement that worktrees are not claimed to be a sandbox is appreciated. Non-Windows platforms have no OS-level isolation, and LSP/extension processes are outside the isolation.

**Gap layer (weak):**

- AZ-031 secret references and full-egress redaction `blocked`: model/MCP egress has candidates, but cross-egress scanning (logs/diagnostics/export/app_control/conversation) is unfinished — **until it is complete, "secrets never enter the model context" is only a design goal.** — *Status (2026-11-02): AZ-031 `done` in the ledger.*
- Issues like AGENT-01 from the 2026-09 audit (ToolContext bypassing permissions by default) must be filed with evidence on the current version in the form of a "per-execution-entry `require_binding` propagation list" (wording from the AZ-008 reassessment); the trustworthiness of the permission system equals that of its weakest entry point.
- Connector OAuth lifecycle AZ-051 `pending`: PKCE/refresh/per-call validation have candidates, but expiry-revocation-in-flight-call journeys against real providers have not been run. Connectors are the agent's channel to the user's real assets (mail/calendar/code hosting); this should be treated as P1, not P2. — *Open (2026-11-02): AZ-051 `deferred`.*

### 3.5 State, events and recovery 【7.0/10, this system's moat】

`crates/agentz-task-state` is the home-grown component with the highest engineering content in the whole system:

- **Run/Attempt state machine** separates execution state from delivery state, exhaustively rejecting illegal 7×7 transitions (AZ-017, 147+2 tests);
- **Durable events** carry task/run/attempt/environment identifiers and increasing sequence numbers, with stream-gap cursor backfill, deduplication and cross-task isolation (AZ-018, 149+4+6 tests);
- **Startup reconciliation:** OS exclusive-lock owner lease + under kill-process fault injection, zombie `running` is marked `interrupted`, tools are not replayed, and runs without a lease are marked `unverifiable` (AZ-019);
- **Closed-loop cancellation:** both queued and executing paths, `cancelling→cancelled` with a grace-period wrap-up, forced interruption of non-cooperative tools, real shell/code_run fixtures (AZ-021, five review rounds);
- **External-action idempotency:** intent + single-use permit + no blind resend without reconciliation when a result is lost (AZ-025, 12 SQLite fixtures including a two-connection race) — **completely absent in most agent products, and exactly where real incidents (duplicate emails/duplicate releases) are most frequent**;
- **Plan persistence** fixed resume-clearing semantics (AZ-020), process/tool recovery contract (AZ-022), model retry classification (AZ-023), task-level budget reservation and fail-closed (AZ-024).

**Deductions:** AZ-025 is domain-layer only, with no host provider adapter; AZ-052 external-send flow `deferred` means the idempotency contract does not yet protect any real send; the budget does not cover side-path model calls such as auto titles/memory/skill retrospectives/curator/inline edit (acknowledged in the AZ-024 reassessment) — **the real cost of long tasks can still leak through these side doors**; team/pool events are outside AZ-018's scope. — *Open (2026-11-02): AZ-052 still `deferred`.*

### 3.6 Multi-agent and orchestration 【5.0/10】

The family of forms is complete: Koi (stateful sub-agent persona), Fish (stateless short task), Pool (swarm board/coordinator), Team templates, coordinator-free deterministic workflows (node persistence + manual resume), Gateway headless turns (IM entry). The `pool_org/pool_chat` tools + coordinator configuration (timeouts/worktrees) show this is not a pure UI toy.

But by the engineering standard "does multi-agent complete tasks more reliably than single-agent": member environment isolation AZ-070 and team-shared budget and cancellation fan-out AZ-071 have not landed (the ledger states plainly that the existing swarm path shares one directory and the budget layer explicitly rejects multiple members) — **i.e. multi-agent currently runs on top of the very pitfalls single-agent just fixed (environment crosstalk, duplicated budgets)**; the AZ-075 comparison evaluation (single agent vs read-only parallel vs implementation team) has a protocol ready and zero data. Conclusion: **orchestration mechanism 6.5, orchestration trustworthiness 3.5.** Until AZ-070/071 close, team mode should be off by default or clearly labelled experimental. — *Open (2026-11-02): AZ-070/071/075 all `deferred`. Note that `src-tauri/src/runtime/` now has `subagent_worktree.rs` and `subagent_policy.rs`; their coverage of team members was not verified here.*

### 3.7 Observability and evaluation 【5.0/10】

Positives: durable events naturally provide replay-style observability; the diagnostic export candidate includes a manifest and redaction (AZ-061); AZ-004 established a real-kernel baseline of 12 fixed tasks × 3 (including duration/tokens/price estimates, with automatic and semantic review reported separately) — **few products have a baseline mindset.**

Gaps: the baseline is a 2026-10 historical measurement (`historical_limited_measurement`), synthetic kernel tasks only, no real desktop journeys; evaluation trend gating AZ-062 `pending` (rules such as "missing samples are not counted as success" are not wired to a stable harness); no per-version regression curves for success rate/cost/human intervention. **Without continuous evaluation, every "optimisation" to multi-model fallback, compaction thresholds or tools is unfalsifiable.** This is the gate the harness must pass to go from 7 to 8.5. — *Status (2026-11-02): AZ-061 and AZ-062 `done` in the ledger.*

### 3.8 Memory and skill evolution 【6.0/10, potential area】

A long-term memory plugin interface, memory_store/recall tools, skill provenance and authorisation change records (AZ-033: source/version/SHA-256/fail-closed on newly added tools), and skill evolution (draft promotion/locking/pinning/periodic curator tidy-up/revision and usage lists) make up the "it understands you better the more you use it" story. Risk: **wrong solidification and permission drift** of memory/skills (a skill upgrade silently widening permissions) already has a fail-closed design, but memory pollution rate and recall precision under real usage are unmeasured; background model calls such as the curator are outside the task budget (same as §3.5). High potential, early evidence — keep it positioned as experimental.

---

## 4. Cross-cutting evaluation

### 4.1 Overall security 【6.0/10】

Defence in depth has taken shape: a tightened CSP (`default-src 'self'`, `object-src 'none'`, controlled `agentz-resource:` protocol), Webview panel tokens and path allowlists, secret encryption at rest (ChaCha20-Poly1305) + Windows DPAPI master-key protection + fail-closed encryption (AZ-029/030), OS-protected sealing of connector tokens (0.9.11/12), diagnostic export redaction, AppContainer execution isolation, the approval TOCTOU defence series.

**Three gates not yet closed (ordered by exploitability):** ① AZ-032 real Webview isolation tests (the execution boundary of third-party extension code); ② AZ-031 full-egress secret scanning (keys leaking via logs/exports/model context); ③ AZ-077/078 real-machine adaptation for macOS Keychain / Linux Secret Service (credential protection on those two platforms has only cross-compile checks). Until ① and ② close, public distribution of third-party extensions and connectors is not recommended. — *Status (2026-11-02): ① and ② `done` in the ledger; ③ still `deferred`.*

### 4.2 Reliability and data safety 【6.5/10】

File journal + per-turn Undo (including the same-turn repeated-edit fix AZ-079), recoverable archive snapshots, editor hot-exit recovery, workspace-close gating, session state self-healing (0.9.17's `SessionRunningGuard` + list reconciliation, fixing details like "phantom running", showing the team takes state consistency seriously) — solid investment on the data side. **The biggest unexploded bomb is AZ-063: versioned data migration and backup/restore are still `pending`** (an offline encrypted backup candidate exists; fixtures for two historical versions, write quiescing and consistent snapshots are missing). For a desktop product holding users' task history, credentials and artifacts, **without a rehearsed migration/restore, every schema upgrade is a gamble**; this should sit alongside signing as a 1.0 gate. — *Status (2026-11-02): AZ-063 `done` in the ledger; `SessionRunningGuard` is now in `src-tauri/src/commands/chat_turn/`.*

### 4.3 Performance 【4.5/10】

Known facts: a production build once produced an unminified main chunk of ~5.95MB (gzip ~1.18MB) due to a minification failure; the build needs `NODE_OPTIONS=--max-old-space-size=4096` and CI notes OOM risk for the heavy frontend; single WebView + Monaco/Mermaid/KaTeX/elk on the same thread. There is point governance (lazy-loaded CodeMap, refresh coalescing, path_filter, perfCounters). AZ-059 (before/after first-paint comparison + offline check) `blocked`, AZ-060 (2-hour resource growth) `pending`, AZ-057 (Windows benchmark incl. hardware/frozen build/real endpoints) `blocked`. **Until baselines exist, every "smooth/janky" discussion is unfounded; performance is one of the domains with the thinnest evidence today.** — *Status (2026-11-02): AZ-057/059/060 `done` in the ledger; their numbers were not re-checked here.*

### 4.4 Testing and CI 【5.5/10】

Assets: frontend ~45 files / 310 tests (bounded concurrency), hundreds of Rust task-state domain and kernel tests, extension host smoke tests, script-level gate tests (`upgrade-release-gate.test.mjs`), and the ledger's own `validate/render` governance. CI (Ubuntu) covers frontend lint/typecheck/test/build + extension host smoke + Rust clippy (`-D warnings`)/test — **static gate density is higher than average for a project this size.**

Gaps: ① the core-lifecycle E2E (AZ-056: isolated create → two tasks → resume → restart → review → archive) is `blocked`, and releases are not blocked by it; ② CI has no Windows/macOS desktop behaviour verification (packaging ≠ acceptance); ③ Vitest at default concurrency once had abnormal worker exits, meaning the test infrastructure itself is flaky; ④ from 0.9.12 on, several versions shipped despite "did not pass the release acceptance gate" (owner waiver) — **the authority of the gate has been diluted by a waiver habit** — this is a process risk, not a technical one, and harder to fix. — *Status (2026-11-02): AZ-056 `done` in the ledger; frontend now has 69 test files; `ci.yml` still runs on Ubuntu only, with a separate Windows `candidate.yml`.*

### 4.5 Release engineering 【3.5/10】

Version governance has highlights: a version consistency check script, release notes/checklist per version, ledger-driven release decisions, honest labelling of unsigned and unaccepted builds (the documentation integrity deserves credit). But the hard gaps line up neatly: unsigned/unnotarised (AZ-064 `blocked`, no certificate), no automatic updates or bad-signature rejection (AZ-065 `deferred`), `release-readiness` not wired into `release.yml` (the 0.9.17 release notes acknowledge "otherwise the installer jobs will not start"), macOS/Linux installers never run on real machines. **The current distribution form is only suitable for "voluntary beta with known risks"; any public download page should state this equally prominently.** — *Status (2026-11-02): fixed — `.github/workflows/release.yml` defines a `release-readiness` job and every package job `needs: release-readiness`. Still open: AZ-064/065 `deferred`.*

### 4.6 Engineering governance and maintainability 【6.5/10】

A rare duality: on governance it is top of its class for a project this size — an 83-item atomic ledger (dependencies/acceptance/evidence/reviewer/rollback all present), a systematic set of evidence files, records of multiple independent BLOCK→PASS review rounds, ADR waiver trails, and the discipline of not retroactively endorsing historical `done` items in reassessments. **This "no self-deception" mechanism is AgentZ's most underrated asset.**

The code side, by contrast, is under strain: `App.tsx` 47KB, `chat_turn.rs` 3.2k lines, kernel `loop_.rs` ~313KB, ~300 host commands registered flat; the domain service split (TaskService/EnvironmentService/EventStore, proposed in the 2026-10-04 review) has only partly landed in `agentz-task-state`, and the host's `commands/chat*.rs` remain multi-responsibility aggregates. Also note: **the current working tree has a large number of untracked/uncommitted files (`git status` shows most core source as `??`)**, so the mapping between source baseline and release artifacts relies on ledger fingerprints rather than commit history — a substantive risk for audit, rollback and collaboration; these should be brought into the repository first and the "release = a specific commit" discipline restored. — *Status (2026-11-02): `chat_turn` is now a module directory (partially fixed); `App.tsx` (~58KB) and `loop_.rs` (~332KB) are still large; the untracked-files situation still applies to this working tree (most of `src/` shows as `??`).*

---

## 5. Competitive coordinates (qualitative, not benchmarks)

| Comparison | AgentZ's relative position |
|---|---|
| **VS Code + Copilot class** | Far behind in editor maturity, evidence of extension compatibility, and real-machine verification of debugging/remote (they count in decades); but the agent task model (environment binding/durable execution/delivery review) is more complete. **Do not compete head-on on IDE completeness; compete on "trustworthiness of delegation".** |
| **Cursor** | Behind in polish of the single-view agent experience, completion (Tab) and indexing; the dual workbench (CodeZ/WorkZ) + same-task human–agent handoff + an open extension host are differentiators, but the handoff is not a closed loop (AZ-036/037), so the differentiators are still "potential energy". |
| **Codex / Claude Code-class CLI agents** | They win on large-scale evidence for their harness and minimalist reliability; AgentZ's durable execution/approval/idempotency design is on par or finer on paper, but lacks real task success data of comparable scale. **The value of a GUI must be cashed in through "review/handoff/artifacts", otherwise it cannot beat the reliable + cheap CLI.** |
| **Devin-class cloud agents** | Cloud sandboxes and asynchronous delegation are their home turf; AgentZ's home turf is local data sovereignty + local repositories/credentials + takeover inside the IDE. Until desktop operation/browser delegation (AZ-053/054) is ready, it should not be marketed on that dimension. |

---

## 6. Risk register (Top 10, by expected loss = impact × likelihood)

| # | Risk | Level | Trigger scenario | Ledger items |
|---|---|---|---|---|
| R1 | Dirty buffer overwritten by agent/external write, user's unsaved work lost | **P0** | Human and agent editing the same file (the core selling-point scenario itself) | AZ-041, AZ-037 |
| R2 | Third-party Webview extension reaches the host or leaks across panels | **P0** (before open distribution) | Installing a VSIX with a Webview | AZ-032, AZ-046 |
| R3 | Secrets leak via an unscanned egress (logs/export/conversation/MCP) | **P1** | Exporting diagnostics, pasting conversations, connector calls | AZ-031, AZ-061 |
| R4 | Data migration fails or backups cannot be restored, corrupting task history/credentials | **P1** | Any schema upgrade | AZ-063, AZ-065 |
| R5 | CodeZ opens a task environment unbound; the human edits/debugs/opens terminals in the wrong directory | **P1** | WorkZ → CodeZ handoff | AZ-036, AZ-043, AZ-044 |
| R6 | Team mode members cross environments / budget duplicated per member; cost and changes out of control | **P1** (before enabling teams) | Pool/Team on a real repository | AZ-070, AZ-071 |
| R7 | External sends lack host idempotency adaptation; duplicate sends/publishes after a lost response | **P1** | Connector/gateway send actions | AZ-052 (depends on hosting AZ-025) |
| R8 | Unsigned distribution + no trusted updates; users install tampered packages or cannot upgrade safely | **P1** | Wider public distribution | AZ-064, AZ-065 |
| R9 | Long-task resource growth / single-WebView jank worsens with session length; no baseline, no budget | **P2** | 2-hour tasks, long conversations | AZ-057, AZ-059, AZ-060 |
| R10 | Unfrozen kernel contract + vendor patch drift; kernel upgrades introduce silent regressions | **P2** | Kernel version iteration | AZ-026; giant `loop_.rs` file |

R1 and R2 share one trait: **they occur in exactly the scenarios the product most wants to advertise** (human–agent co-editing, installing extensions). That makes them not entries on a "known issues list" but qualifiers on the release statement.

> *Status (2026-11-02, by ledger):* R1, R2, R3, R5, R9 — all listed items now `done` except AZ-037 (also `done`) — treat as mitigated pending native re-verification. R4 partially (AZ-063 `done`, AZ-065 `deferred`). R6, R7, R8 open (AZ-070/071, AZ-052, AZ-064/065 `deferred`). R10 partially (AZ-026 `done`; `loop_.rs` still a single large file).

---

## 7. Roadmap recommendations (milestones are acceptance events, not version numbers)

### Phase A: close the trustworthy single-agent loop (1.0 gate; freeze new feature surfaces)

1. **Handoff and edit safety:** AZ-036 (task binding for four kinds of operations) → AZ-041 (dirty-buffer conflict) → AZ-037 (human edits enter the next turn's context). Acceptance: a recorded real bug-fix journey "delegate in WorkZ → take over in CodeZ and change one line → agent resumes and notices → review and apply".
2. **Native E2E gate:** land AZ-056 and wire it into the release pipeline (failure blocks), reusing the journey from A1 + the A/B fixtures from AZ-003.
3. **Security closure:** AZ-032 real-machine positive/negative tests → AZ-031 full-egress secret scanning → decide the distribution scope for third-party extensions/connectors accordingly.
4. **Data and distribution:** AZ-063 two-historical-version migration fixtures + restore drill; AZ-064 obtain signing/notarisation and wire `release-readiness` into `release.yml`; AZ-065 the update channel can come later, but **downgrade protection (ADR-004) must take effect with 1.0**.
5. **Performance baseline:** AZ-057 cold start + 2-hour soak on a frozen build, producing budget numbers for AZ-059/060; no performance claims without numbers.
6. **Repository hygiene:** bring untracked source into the repository and restore "release artifact ↔ commit" traceability; split `chat_turn.rs` and `App.tsx` by domain (riding on AZ-035's Task DTO service extraction rather than a standalone big refactor).

**Suggested conditions for claiming 1.0:** A1–A5 all closed with evidence; of the three platforms, at least a complete Windows journey + install/launch/core-journey records on the other two (AZ-058); the support matrix labelled honestly by AZ-005 tiers; multi-agent/desktop operation/remote kept labelled "experimental/unverified".

> *Status (2026-11-02):* A1, A2, A3 and A5 items are `done` in the ledger; A4 partially (AZ-063 `done`, `release-readiness` wired into `release.yml`, AZ-064/065 `deferred`); A6 partially (`chat_turn` split; untracked source remains).

### Phase B: turn the harness advantage into citable data

- Wire AZ-062 evaluation trends to a stable harness: a fixed task set (covering four kinds — real-repository bug fixes/refactors/research/docs) regressed per version, publishing internal success-rate/cost/intervention curves;
- Budget covers every model entry point (including titles/memory/curator/completion), with cost attributable to tasks;
- AZ-045 real SSH journey, AZ-043/044 native debug/terminal regressions, completing acceptance of the IDE's everyday surfaces.

### Phase C: controlled expansion (each item off by default, enabled by evidence)

- AZ-070/071 team isolation and shared budget → enable teams by default only after the AZ-075 comparison evaluation meets the bar;
- AZ-053 browser preemption (pin a new RobotZ version) → AZ-054/055 desktop operation and resource leases;
- AZ-072~074 automation/notifications/workflow checkpoints opened under AZ-025 host idempotency and budget constraints;
- AZ-068 real beta and support triage start after the Phase A gate.

---

## 8. Conclusion

What makes AgentZ rare is that it does three hard things at once: the shell of a real IDE, a modern agent harness design, and a "no self-deception" acceptance ledger. The first two decide that it is **worth continuing**; the third decides that it **can continue** — as long as the ledger's `blocked/preparing` items are treated as the boundary of release statements rather than decoration on a to-do list.

Scored against professional IDE standards, today it is **a 5.9 IDE, a 6.7 harness and a 4.5 deliverable product**. The shortest stave (distribution and cross-platform acceptance) means it can only be in beta for now; the longest (the durable-execution harness) means that once acceptance catches up, it has the right to tell a story different from both Cursor and Codex: **not an editor that chats faster, but a local workbench you can actually delegate to.**

The release note most worth writing for the next version is a single line: **"This version passed the AZ-056 core lifecycle gate."** Until then, going from 0.9.19 to 0.9.29 does not change this report's conclusions.

---

## Appendix A: Ledger status summary (as of 2026-10-18/19)

- 83 items total: 37 `done`, 11 executing/in review, 11 blocked, the rest `preparing/pending/deferred`; only gates M0 and M1 `passed`, M2–M9 and MB (real-task baseline) all `pending`.
- The 46 unaccepted items and user test cases are in `docs/upgrade/acceptance-0.9.12.md` (UT-001~046, UT-R01~R07); most still have no evidence written back.
- The 2026-10-13 reassessment attached qualifiers to several historical `done` items: `bounded_source_review / historical_* / current_source_read_only_no_tests_rerun` — this report's scores use the "qualified" confidence.
- *2026-11-02 update:* `docs/upgrade/plan.json` now shows 83 items: 65 `done`, 18 `deferred`.

## Appendix B: Relationship to existing documents

- `docs/agentz-product-maturity-review-2026-10-04.md` (F01–F12 review): most of its M0/M1 recommendations have landed; §2.5/§3.5 of this report are updated footnotes to it; F-class issues not re-verified item by item on current source are not claimed closed here.
- `docs/ai-ide-engineering-audit.md` (2026-09-28, 0.6.3): its LSP/Git/performance conclusions are a historical baseline; this report updates status with ledger evidence and did not re-run all of its reproductions.
- `docs/upgrade/reassessment-2026-10-13.md`: the test figures in this report (310 frontend tests, 152+2 domain tests, etc.) are quoted from that reassessment log and were not re-run in this round.
