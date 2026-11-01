# AgentZ 0.9.19 Evaluation: A Vibe-Coding Programmer's Perspective

**English** | [简体中文](vibe-coder-evaluation-2026-10-19.zh-CN.md)

Date: 2026-10-19. Subject: `main` branch 0.9.19 (commit `1b17637`).

> **Status (2026-11-02, AgentZ 1.0.40):** This is a dated evaluation; its conclusions are kept. Since then: a user-facing command palette / quick open now exists (`src/workspaces/codez/CommandPalette.tsx`, bound in `useCodezShortcuts.ts` to Ctrl/Cmd+P for files and Ctrl/Cmd+Shift+P for commands); hard-coded Chinese literals in `.tsx` files are gone (0 found outside tests); the giant `codez/index.tsx`, `workz/index.tsx` and `AssistantPanel.tsx` components have been split (`App.tsx` has grown instead); and the ledger (`docs/upgrade/plan.json`) marks the dirty-buffer conflict (AZ-041), human-edit feedback (AZ-037) and the E2E gate (AZ-056) as `done`. Still open: code signing (AZ-064), multi-agent team isolation/budget/benefit evidence (AZ-070/071/075), and real remote reconnect (AZ-045) — all `deferred`. Ledger `done` items were not natively re-verified for this note.

## 0. Method and scope

- Perspective: a programmer whose main way of working is "driving AI to write code in natural language". Day-to-day comparisons are Cursor, Claude Code, Codex and Windsurf. What matters: how fast you can run your first task, whether AI code changes are controllable, whether you can roll back quickly when things go wrong, and whether the UI keeps you focused on the "state the requirement → look at the diff → accept" loop.
- Basis: static reading of README, CHANGELOG, the `docs/upgrade/acceptance-0.9.12.md` acceptance checklist, frontend source in `src/`, and the backend command list in `src-tauri/src/commands/`.
- Not done: no actual run or screenshots on an installer. UI conclusions come from code structure and release records; any judgement about "feel" should be revised after a hands-on walkthrough.

## 1. One-line conclusion

AgentZ's **feature ambition and underlying capabilities already match the breadth of first-tier AI IDEs**, and on the "task isolation (worktree) + multi-agent teams + code graph/repo memory" line it is even more aggressive than mainstream products; but **what matters most to a vibe coder — an "out-of-the-box, quickly trustworthy single-person code-change loop" — is not polished yet**: it has not passed the acceptance gate, the installer is unsigned, muscle-memory entry points like Ctrl+P / a command palette are missing, and first-run onboarding has not been walked through by real users. For now it suits early adopters who like to tinker, not replacing Cursor as a main IDE.

> *Status (2026-11-02):* Ctrl+P / command palette — fixed (see above). Acceptance gate — AZ-056 `done` in the ledger. Unsigned installer — still open (AZ-064 `deferred`). First-run walkthrough with real users — not verified.

Overall scores (out of 10, higher is better):

| Dimension | Score |
|---|---|
| Feature breadth | 9 |
| AI code-change loop (request → diff → accept/roll back) | 7 |
| Task isolation and parallelism | 8.5 |
| Context and code understanding | 8 |
| UI consistency and look | 6 |
| First-run experience | 5 |
| Stability and trustworthiness | 4.5 |
| Extensibility | 7.5 |

## 2. Product shape: CodeZ + WorkZ dual mode

AgentZ splits one desktop app into two ways of working:

- **CodeZ**: editor-centred, comparable to Cursor. Monaco editor, file tree, Git, terminal, built-in browser, LSP/DAP, AI chat on the right, supporting `@file`, `@codebase`, `@graph`, Ctrl/⌘+K inline edit, Tab ghost-text completion.
- **WorkZ**: task-centred, comparable to Codex. Each task gets a git worktree and branch, multiple agents can run in parallel, and afterwards you review the diff, merge, or open a PR.

**How it feels to a vibe coder:**

- Pro: the split maps exactly onto the two states of vibe coding — "I'm watching it change things" and "I hand it to the AI and check back later". Since 0.9.15 both modes share the same project and session database, so a session started in CodeZ can be continued directly from WorkZ. This is the right call and avoids the "two products glued together" feeling.
- Risk: a new user opening it for the first time has to figure out "which mode should I go into". No explicit mode-selection guidance was found, so it's easy to click into both and get lost.

## 3. Feature evaluation

### 3.1 AI code-change loop (the core)

| Capability | Current state | Assessment |
|---|---|---|
| Chat-driven code changes | Yes, multi-model (Anthropic, OpenAI, DeepSeek, Qwen (Tongyi), MiniMax, Zhipu, Kimi and any OpenAI-compatible endpoint) | Coverage of Chinese models is a clear advantage over Cursor and friendly to developers in China |
| Inline edit Ctrl+K | Yes, inline diff, Enter to accept, Esc to undo | Interaction matches Cursor, low migration cost |
| Tab completion | Ghost text | Completion quality and latency need hands-on testing; this is the metric vibe coders are most sensitive to |
| Session fork / checkpoints | Yes; 0.9.16 fixed fork naming | Very valuable — "this path went wrong, go back to a checkpoint and rephrase" is daily vibe coding |
| Plan mode | Yes: `chat_plan`, `DurablePlanActions` | Good for aligning on approach before big changes. *(2026-11-02: plan mode exists — `src/services/tauri/chatPlan.ts`, `src-tauri/src/runtime/durable_plan.rs` — but no `DurablePlanActions` symbol was found in current code.)* |
| Dirty buffer vs. external change conflict | Acceptance item UT-012 unverified | **High risk**: if the agent overwrites my unsaved edits when changing a file, trust collapses immediately. *(2026-11-02: ledger AZ-041 `done`; `editorConflict.ts` + tests exist; not re-verified natively.)* |
| Human edits after pausing a task fed back to the agent | UT-008 not implemented | A gap for the frequent "AI writes half, I fix it by hand, then let it continue" scenario. *(2026-11-02: ledger AZ-037 `done`.)* |

### 3.2 Task isolation and parallelism (WorkZ)

- Each task has its own worktree (`../.agentz-worktrees/task-<id>`) + branch `workz/task-<id>`; afterwards a `base...branch` diff, a no-ff merge, or a PR via `gh`. This is currently the safest way to let AI "go ahead and change things", and **is AgentZ's most marketable selling point**. *(Paths confirmed in `crates/agentz-task-state/src/cleanup.rs`.)*
- Team templates (`fullstack-squad`, `research-duo`) and built-in roles (Architect, Coder, Reviewer, etc.) are good concepts, but UT-040/041/045 in the acceptance checklist (member environment isolation, team budget, relative multi-agent benefit) are all "not started / no data". In other words: **multi-agent is currently more of a demo capability and cannot yet show it is faster or cheaper than a single agent**. Recommend team mode be off by default until there is comparison data. *(2026-11-02: still open — AZ-070/071/075 `deferred`.)*
- Delivery review (`DeliveryReviewPanel`, `AgentTaskReview`) has components and tests, and 0.9.19 can also sort changed files by "reference count", which helps when reviewing big diffs. *(2026-11-02: `src/workspaces/workz/AgentTaskReview.tsx` + test exist; no `DeliveryReviewPanel` component was found in current code.)*

### 3.3 Context and code understanding

- Code graph: Tree-sitter symbols, call edges, PageRank to rank hub files, and it can even link frontend `invoke` calls to Rust `#[tauri::command]`. Very well suited to Tauri projects.
- Repository memory `.agentz/REPO_MEMORY.md`: records invariants and pitfalls, and lines it cites are automatically marked stale when modified. This design is smarter than a plain rules file and solves "the rule was written but the code has long since changed". *(Tool at `src-tauri/src/tools/repo_memory.rs`; the exact file name was not confirmed in Rust code.)*
- Dual-track wiki (fast architecture wiki + incremental DeepWiki), local vector + keyword hybrid memory.
- Compatible with `.cursor/rules/`, so rules migrated from Cursor work as-is — migration-friendly.

**Assessment**: the design level here is above average. But the more features there are, the more each agent turn stuffs into context, so token consumption and time-to-first-token deserve attention. The UI should show "which context this turn used and what it cost" — the `BudgetObservation` component already exists, which is the right direction. *(2026-11-02: no `BudgetObservation` symbol found in current code; `src/components/ContextUsageRing.tsx` shows total context usage.)*

### 3.4 Engineering environment

- Built-in terminal (xterm + PTY), Git panel, LSP, DAP debugging, built-in browser (RobotZ), automatic detection of Rust/Node/Python/Go/Java/.NET/C++ and other toolchains (`devenv`).
- Remote development: SSH / Dev Container / Docker / WSL, auto-deploys agentz-server, automatic port forwarding. A host key change rejects the connection — a sensible secure default. But real remote disconnect recovery (UT-015) has not been verified. *(2026-11-02: still open — AZ-045 `deferred`.)*
- Extensions: a home-grown VS Code-compatible extension host, installing from Open VSX or `.vsix`. The wired surface is broad (commands, completion, diagnostics, tree views, Webview, debugging, testing…), but **there is no compatibility profile for real extensions yet** (UT-016). Whether a vibe coder's everyday ESLint, Prettier and Tailwind IntelliSense work is key to whether it can be a main IDE. *(2026-11-02: ledger AZ-046 `done`.)*

### 3.5 Peripheral capabilities

Skill marketplace (ClawHub), Hooks, MCP (stdio/SSE), workflow designer, scheduled automation, Feishu/WeChat/DingTalk gateways, connector OAuth, desktop control `app_control`.

**Assessment**: astonishing breadth, but for a vibe coder most of it is "good to know, not using it yet". The problem is that these entry points are all piled into Settings, diluting the core experience (see 4.3). Many items in this group in the acceptance checklist are deferred / pending.

## 4. UI evaluation

> The following comes from reading the source structure and was not checked against real screenshots.

### 4.1 Layout and information architecture

- CodeZ is the standard "activity bar + sidebar + editor area + bottom panel + right-side AI panel + status bar" — zero learning cost for VS Code / Cursor users.
- WorkZ is "session sidebar + conversation stream + artifact drawer + collaboration board". 0.9.18 made chat bubbles proportional to column width (agent replies take 80%), improving reading on large screens; Mermaid scales with the bubble.
- 0.9.13 removed the duplicate chat panel, and 0.9.12 removed the "working directory / configuration details / start your first task" explainer panels from the conversation — the right direction: **a vibe coder wants a clean conversation area, not tutorial cards**.

### 4.2 Consistency and theming

- Only in 0.9.16 was a batch of panels fixed that still had hard-coded dark backgrounds in the light theme (remote connection, project templates, task review, skill revision panels).
- Static count: CSS under `src/` had about 750 hex colour literals and only about 150 uses of `var(--…)` theme variables; components also had several inline `style={{…}}`. This means theming issues are likely to keep popping up. Recommend converging all colours onto design tokens. *(2026-11-02, partially fixed: a line-count recount finds ~1,150 lines with hex literals vs ~1,500 lines using `var(--` in `src/**/*.css`; counting method differs from the original, but variables now outnumber literals.)*
- About 185 lines in frontend `.tsx` files contained Chinese literals directly, bypassing i18n. Switching to the English UI may show mixed Chinese/English (UT-017 also notes Chinese/English semantic consistency has not been walked through). *(2026-11-02: fixed — no Han-character literals found in non-test `.tsx` files.)*

### 4.3 Efficiency entry points (what vibe coders care about most)

- **No user-facing command palette (Ctrl+Shift+P) or quick file open (Ctrl+P) was found.** The code only had a `QuickPick` for extensions. For keyboard-driven programmers this is the most obvious gap, and it should be prioritised above most peripheral features. *(2026-11-02: fixed — `src/workspaces/codez/CommandPalette.tsx` (files mode and `>` commands mode, with tests), bound in `src/workspaces/codez/useCodezShortcuts.ts`.)*
- The settings panel `SettingsPanel.tsx` is about 43KB and hosts a dozen-plus sections: assistant, rules, Hooks, workflow designer, remote, diagnostics, app changes, etc. Everything is there, but "I just want to switch model" means wading through a lot. Recommend making "models and keys" a top-level shortcut, e.g. click the status bar to switch model. *(2026-11-02: `src/workspaces/codez/SettingsPanel.tsx` is now ~55KB — still open.)*
- Keyboard accessibility: there are BottomPanel keyboard tests, but the overall keyboard walkthrough (UT-017) has not been done; the frontend has about 66 `aria-` attributes, so accessibility is at an early stage.

### 4.4 Status feedback

- 0.9.16 and 0.9.17 fixed issues like the stop button spinning forever and the session LED blinking constantly, showing the team cares about the accuracy of "running / stopped" state — which matters a lot.
- The three pieces of in-run information — "blocking reason, latest action, budget" (UT-011) — have a candidate implementation not yet walked through with users.

## 5. Stability and trustworthiness

This is currently the biggest weakness, and the deciding factor in whether a vibe coder dares to hand it a real project:

1. **Eight consecutive versions from 0.9.12 to 0.9.19 state "did not pass the release acceptance gate"**, 46 acceptance items are unfinished, and the end-to-end gate (UT-026) has not been run. *(2026-11-02: ledger AZ-056 `done`; 65 of 83 ledger items `done`, 18 `deferred`.)*
2. **The installer is unsigned**, so Windows SmartScreen blocks it; macOS/Linux have not been run on real machines. *(2026-11-02: still open — AZ-064 `deferred`; AZ-058 status not checked.)*
3. Seven versions shipped in two days (10-18 to 10-19). Iteration is fast, but each version changes little and mostly fixes UI regressions, indicating a lack of automated UI regression protection.
4. Long-task memory growth (UT-030), leftover resources (UT-027), and data migration and backup/restore (UT-033) are all unverified. For vibe coders who run agents for hours at a time, these are real risks. *(2026-11-02: ledger AZ-060 and AZ-063 `done`.)*
5. Several core frontend files are large: `workz/index.tsx` about 77KB (46 `useState`), `codez/index.tsx` about 66KB, `AssistantPanel.tsx` about 59KB, `App.tsx` about 46KB. State concentrated in giant components is the breeding ground for "fix A, break B" UI regressions. *(2026-11-02, partially fixed: `workz/index.tsx` ~11KB, `codez/index.tsx` ~24KB, `AssistantPanel.tsx` ~12KB; but `App.tsx` grew to ~58KB.)*

Positives: the frontend already has 51 test files, secure defaults (host keys, token encryption, diagnostic redaction) are carefully designed, and the CHANGELOG is honest in disclosing "known limitations". *(2026-11-02: 69 test files under `src/`.)*

## 6. Comparison with mainstream products

| Dimension | AgentZ | Cursor | Claude Code / Codex |
|---|---|---|---|
| Editor experience | Monaco, close to VS Code | VS Code fork, mature | None; terminal or web |
| Inline edit / Tab | Yes, needs hands-on testing | Industry benchmark | None |
| Parallel task isolation | worktree, native support | Background agents | Codex cloud sandbox |
| Multi-agent teams | Yes, benefit unproven | Weak | Sub-agents |
| Chinese models | Native coverage | Limited | None |
| Code graph / repo memory | Strong | Index + rules | CLAUDE.md |
| Extension ecosystem | Home-grown compat layer, no profiles | Full VS Code ecosystem | MCP |
| Stability | Has not passed the gate | Mature | Mature |
| Local / open source | Apache-2.0, local | Closed source | Closed source |

**Differentiated positioning**: open source, local, friendly to Chinese models, worktree parallelism, repository memory. Combined, these give it a unique place in the market.

## 7. Recommendations (ordered by benefit to vibe coders)

**P0: make the single-person code-change loop trustworthy first**

1. Add Ctrl+P quick open and a Ctrl+Shift+P command palette. *(2026-11-02: done.)*
2. Verify and fix the dirty-buffer conflict (UT-012): the agent must never overwrite unsaved edits. *(2026-11-02: ledger AZ-041 `done`.)*
3. One-click rollback of an entire agent turn (not just session checkpoints, but also file rollback).
4. Get the end-to-end gate (UT-026) running, covering at least the main path "open project → chat-driven change → accept diff → commit", to stop UI regressions recurring release after release. *(2026-11-02: ledger AZ-056 `done`.)*
5. Code signing, to eliminate system blocking on first install. *(2026-11-02: open — AZ-064 `deferred`.)*

**P1: lower the first-run barrier**

6. A three-step first-launch guide: choose a model and enter a key → open a project → run a sample task (UT-036), plus a 5-person walkthrough.
7. Switch models directly from the status bar and show this turn's tokens and cost.
8. Explain the difference between CodeZ and WorkZ in one sentence and a diagram, and let users choose on first entry.

**P2: converge and polish**

9. Converge all colours onto theme tokens; clean Chinese literals out of `.tsx` and hand them to i18n. *(2026-11-02: Chinese literals done; colour tokens partially.)*
10. Split the four giant components and push state down into stores. *(2026-11-02: three of four split; `App.tsx` still large.)*
11. Build compatibility profiles for the top 10 common extensions (ESLint, Prettier, GitLens, Tailwind, Python, rust-analyzer, etc.).
12. Label unverified capabilities such as multi-agent teams, gateways and desktop control as Beta in the UI and collapse them by default, to avoid diluting the core experience.

## 8. Advice for prospective users

- **Good fit**: early adopters who want an open-source local AI IDE, mainly use Chinese models, like using worktrees to let AI try several approaches in parallel, and are willing to report bugs.
- **Not yet a fit**: developers who need a stable main IDE, depend on many VS Code extensions, work on macOS/Linux, or cannot install unsigned software on company-managed devices.
- **Recommended usage**: treat WorkZ as "AI outsourcing in a safe sandbox" first — let it build features or refactor in an isolated worktree, accept the result in the review panel yourself, then merge; for day-to-day hand-written code, keep using your familiar editor for now.
