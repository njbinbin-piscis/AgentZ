# AgentZ 1.0.69 release notes

Release date: 2026-11-16. See the 1.0.69 entry in `CHANGELOG.md`.

## Fixed

- **Freeze, spinning preview and "Git approval identity timed out" in long sessions.** Root cause found by sampling a hung process with symbols: the `background_list` poll opened the project database on every call, and every open re-ran an O(memories x kois) repair plus write-locking cleanup. The repair now runs once per database file per process.
- **Transient network failures retry.** Connect, TLS-handshake and DNS errors are classified as transient and retried with backoff instead of ending the turn.
- **Turn errors carry the full cause chain** and are logged.

## Known limitations

- The first open of a project in a session still runs the one-time repair (a second or two on large projects).
- Browser tool output that is binary text is dropped from the model context with a warning.
