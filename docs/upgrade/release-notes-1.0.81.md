# AgentZ 1.0.81 release notes

Release date: 2026-11-28. See the 1.0.81 entry in `CHANGELOG.md`.

## Fixed

- Opening a link in the built-in browser no longer hangs for ~30 seconds and then fails with a bare `Navigation failed.` when a page has a slow or never-finishing image, iframe, or third-party subresource.
- Navigation settles on document `interactive` / `complete` (12s cap) via `robotz-browser::nav::navigate_until_interactive` instead of `Page::goto`'s all-frames `load` wait.
- Chat URL clicks show the browser panel immediately while the page loads; command errors surface the full cause chain.

## Changed

- Vendored `robotz-browser` is at v0.1.15; agent `browser` tool `navigate` / `new_tab` use the same settle helper.

## Known limitations

- Chrome error pages without a visible `ERR_*` code get a generic "could not be reached" message.
- Agent tool actions that still call other `chromiumoxide` APIs waiting on full `load` are out of scope for this release.
