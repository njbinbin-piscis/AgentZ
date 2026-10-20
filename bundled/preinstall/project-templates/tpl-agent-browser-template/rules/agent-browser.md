# Browser Automation & Web Verification Rules (`agent-browser`)

When inspecting web pages, debugging frontend UI, or automating browser workflows in this project:

1. **Choose the Right Tool**:
   - Use `web_fetch` for static documentation, raw HTML/JSON endpoints, or READMEs.
   - Use the built-in `browser` tool (CDP control of the IDE Browser panel) for SPAs, interactive flows, DOM inspection, form filling, and visual verification.
2. **Deterministic Snapshot-First Interaction**:
   - Follow the sequence: `navigate` → `lock` → `snapshot` → interact using current element `ref` handles → `assert_url` / `assert_visible` / `assert_text` → `screenshot` (with `save_path` when recording evidence) → `unlock`.
   - After any navigation or major DOM mutation, take a fresh `snapshot` before reusing element references.
3. **Human-in-the-Loop**:
   - Never brute-force captchas, MFA, or destructive confirmation modals; pause and ask the user to complete authentication or confirmation when encountered.
