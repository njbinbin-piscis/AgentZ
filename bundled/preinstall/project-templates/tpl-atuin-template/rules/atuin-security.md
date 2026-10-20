# Security Auditing & Vulnerability Investigation Rules (`atuin`)

When auditing code, investigating security issues, or hardening components in this project:

1. **Attack Surface Mapping**:
   - Use `graph_explore`, `symbol_search`, and `file_search` to trace all untrusted input entry points (HTTP handlers, IPC commands, CLI arguments, file imports, WebSocket/WebView messages, deserializers).
2. **High-Risk Sinks & Boundary Checks**:
   - Audit every path from untrusted input to command execution (`Command::new`, `shell`), filesystem paths (zip-slip, `..` traversal, symlink following), SQL/database queries, HTML/DOM rendering (`dangerouslySetInnerHTML`, WebView CSP), and credential storage.
3. **Evidence-Based Remediation**:
   - Document the exact file, line range, and exploit precondition for each finding.
   - Verify fixes with a targeted regression test covering the malicious or malformed input case, and run `read_lints` on all touched files.
