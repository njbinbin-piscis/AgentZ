# AgentZ 1.0.57 release notes

Release date: 2026-11-06. See the 1.0.57 entry in `CHANGELOG.md`.

## Fixed

- **Restored data of the wrong type no longer blanks the window.** Checkboxes, tags, assistant pickers, text fields, form blocks, options, and buttons are coerced before render. A saved boolean, number, or object is no longer passed to `includes`, `map`, or `trim`. Markdown class names and link URLs are treated as strings. Dropdowns, tag pickers, and team member lists use the same guards.
