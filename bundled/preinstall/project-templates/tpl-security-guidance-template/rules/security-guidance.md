# Secure Coding Guardrails (`security-guidance`)

Apply these security checks before every `file_write` or `file_edit` in this project:

1. **No Command or Code Injection**:
   - Never interpolate unsanitized user or external input into shell command strings, `eval`, `Function`, or child process invocations. Pass arguments as separate array elements and validate paths.
2. **No XSS or Unsafe DOM Injection**:
   - Avoid unescaped HTML rendering (`innerHTML`, `dangerouslySetInnerHTML`) on untrusted content; sanitize or use text bindings.
3. **No Path Traversal or Arbitrary File Access**:
   - Canonicalize file paths and verify they remain within the intended workspace or allowed directory boundary (`..` and symlink checks).
4. **No Hardcoded Secrets**:
   - Never write API keys, tokens, passwords, or private keys into source files, logs, or commits.
