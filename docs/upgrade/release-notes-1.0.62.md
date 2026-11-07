# AgentZ 1.0.62 release notes

Release date: 2026-11-08. See the 1.0.62 entry in `CHANGELOG.md`.

## Fixed

- **WorkZ no longer unlocks the input while a session is still running.** After a reload or switching tasks, WorkZ follows the backend's running state. Messages sent meanwhile wait in a queue list that can insert, steer, edit, or delete them, like CodeZ.
- **Stop works per session.** Cancel can target a session id, so stopping one session cannot hit another. BotZ chats and bot homes wait behind a held session gate instead of showing the raw lock error.
- **Steering a queued message no longer stalls.** If the turn ends while the steer is in flight, the message now starts as the next turn.
- **Answers that mention a `<think>` tag keep their text.** Only an unclosed tag at the very start counts as an unfinished thought.
- **A turn that ends with no answer says so.** The UI shows a notice when the model produced only reasoning.
- **A sub-agent always hands back something.** If the direct "write the report now" request fails, its last output is returned, labelled as unverified. Thinking and tool steps that never finished are settled instead of staying "running".
- **Browser tab switching is validated.** A session can only switch to tabs it opened, a failed switch keeps the old tab, closed sessions release their surface and URL records, and a panel lists only its own tabs.
- **Smaller fixes.** Queue re-keying merges instead of overwriting, a poisoned browser lock no longer panics every later call, and two tests left stale by earlier releases were corrected.
