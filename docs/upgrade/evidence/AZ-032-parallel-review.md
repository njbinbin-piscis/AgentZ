# AZ-032 parallel frontend security review

2026-10-07. Reviewed prior `AZ-032-webview-isolation-started.md`, current frontend policy generator/tests, and the host message boundary. Only `src/extensions/ui/webviewSecurity.ts`, its test file, and this evidence file were edited by the assigned agent. Existing files had no pending diff at the start of this assignment.

## Fixed actual findings

1. Policy injection searched extension HTML for a head/html tag using a regular expression. A head tag inside a comment or raw-text script could receive the injection, leaving the CSP inert or placing it after extension script execution. Policy and API shim now occupy a trusted wrapper head before all extension-provided bytes; an initial doctype is preserved. Extension document tags remain in the body HTML and are handled by normal HTML parsing. The generator no longer trusts an extension-supplied insertion point.
2. Script `src` and nonce scanning used regular expressions that could match attribute-shaped text inside a quoted attribute. A script with `data-example=' src="agentz-resource://localhost/{panel}/..."'` and a real remote src could incorrectly receive a trusted nonce. Attributes are now tokenized with quoted values retained, the first actual src governs scope, and only actual nonce attributes are removed. Boolean nonce attributes are also removed before assigning the host nonce. Attribute text remains intact.

## Verification actually run

`npx vitest run src/extensions/ui/webviewSecurity.test.ts`: **13 passed, 0 failed**, exit 0 (exec session 58627). Three added tests cover deceptive head/raw-text/early script placement, scoped-src text hidden in another attribute, and nonce-shaped text plus an actual boolean nonce. Existing tests continue to cover disabled scripts, missing nonce, resource token matching, external scripts, doctype, raw-text preservation, isolated state, and message size/serialization rejection.

## Reviewed boundaries and remaining acceptance

The unchanged WebviewHost requires exact iframe `event.source`, opaque `event.origin === "null"`, a protocol marker, and bounded message serialization. Its iframe omits allow-same-origin. Wildcard postMessage targetOrigin supports the opaque frame; this does not replace the host's receiving checks. No origin or sandbox permission was broadened.

Only current-panel token script resources receive automatic nonces. Image/font/media/style scheme permission remains broader than a panel token at the CSP layer; filesystem canonicalization, per-token registered roots, and disposal enforcement belong to the existing Rust resource handler and were not modified or revalidated here. This review does not establish malicious extension isolation if a resource capability token is leaked across panels.

These are source and Vitest results, **not** browser CSP enforcement results. Real Tauri srcdoc document parsing, WebView2/WebKit scheme behavior, custom-scheme CSP handling, resource release races, and desktop message-origin adversarial tests remain unrun. No GUI, Rust build, SSH, configuration, ledger update, or permission expansion was performed. AZ-032 remains review and must not be marked accepted by this package alone.
