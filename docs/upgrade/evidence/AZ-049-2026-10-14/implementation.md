# AZ-049 local research-report contract

Added a versioned JSON report template and a dependency-free local validator/Markdown renderer. Required fields are the question, conclusion, source-backed findings, source locators and limitations. Findings cannot cite unknown source IDs; local locators reject parent-directory traversal. The renderer escapes inline HTML and emits a deterministic report for review. This establishes a local format that can later be captured as a `report` artifact under the existing task/environment binding.

`node scripts/upgrade-report.verify.mjs`, template validation, rendering and Node syntax checks passed. The example render and check log are adjacent. This has not yet been wired to task-bound artifact capture, editable versions or continuation context; no real research report or source verification is claimed. AZ-049 remains deferred pending that integration and unified acceptance.

Follow-up: renderer now escapes Markdown control characters as well as HTML metacharacters in human/model supplied fields. A fixture containing a forged Markdown link and emphasis is rendered as literal text; `node scripts/upgrade-report.verify.mjs` passed after the change. No source locator was opened or fact-checked.
