# AgentZ 1.0.71 release notes

Release date: 2026-11-18. See the 1.0.71 entry in `CHANGELOG.md`.

## Fixed

- **Built-in browser links work (#27).** `target="_blank"` links and `window.open` popups are adopted into the session as tabs and the panel follows them; previously the popup target was unowned and the panel stayed on the old page, so such links appeared unclickable. Same-tab links, buttons and inputs were already working and are unchanged.

## Changed

- RobotZ v0.1.14 is vendored at `vendor/robotz` (workspace `[patch]`, same pattern as piscis-engine) with three new manager APIs for popup adoption. Its crate manifests were flattened from workspace inheritance and its `piscis-kernel` pin aligned to v0.8.74.
- Stop-flow test harness uses a complete in-memory `Storage` stub.

## Known limitations

- Popup adoption matches by URL: a popup sharing its opener's exact URL is not adopted (identical content, panel view still correct). Popups opened without a user click (timers/onload) are adopted on the next forwarded click.
- Not yet hand-checked in the running app on a real site with `_blank` links; verified by live Chromium regression test only.