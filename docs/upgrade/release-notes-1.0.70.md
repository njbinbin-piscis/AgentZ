# AgentZ 1.0.70 release notes

Release date: 2026-11-16. See the 1.0.70 entry in `CHANGELOG.md`.

## Fixed

- **Extension "Install" buttons work.** `workbench.extensions.search` / `installExtension` install the given extension id from Open VSX, show progress or the error, and restart the extension host.
- **Extension host start is more robust.** Local ready timeout raised to 30 s; failed starts are retried up to three times.
- **Linux credential store.** A locked Secret Service collection is unlocked via the system keyring prompt instead of failing with "Linux credential collection is locked".

## Known limitations

- The Linux unlock path is typechecked/reviewed only; it has not been exercised on a real Linux desktop session. Headless sessions without a keyring prompter still fail.
- The extension-install command has not been confirmed end to end on a built app.
