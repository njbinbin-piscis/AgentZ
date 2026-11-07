# AgentZ 1.0.60 release notes

Release date: 2026-11-07. See the 1.0.60 entry in `CHANGELOG.md`.

## Fixed

- **A timed-out CodeZ form stays where it was asked.** `chat_ui` no longer remains pinned under the whole session after it expires. The form collapses into the reply at the question, and later text and tool calls render below it.
