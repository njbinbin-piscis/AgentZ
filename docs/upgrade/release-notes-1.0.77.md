# AgentZ 1.0.77 release notes

Release date: 2026-11-25. See the 1.0.77 entry in `CHANGELOG.md`.

## Fixed

- A connection dropped while the model streams a tool call is retried instead of ending the turn.
- Clicking an unpreviewable file opens it in its registered program, or reveals it in its folder if none is registered.

## Changed

- Kernel pin advanced to piscis-engine `v0.8.78` (commit `9c5cd60`).

## Known limitations

- The file-opening fix depends on real OS file associations and was verified by compilation only, not hand-checked.
- Repeated mid-stream disconnects can still exhaust the retries; a proxy cutting long connections would need fixing on the network side.
