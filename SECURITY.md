# Security Policy

**English** | [简体中文](SECURITY.zh-CN.md)

## Supported versions

Security fixes are made on the latest release only. Please upgrade to the newest version on [GitHub Releases](https://github.com/njbinbin-piscis/AgentZ/releases) before reporting.

## Reporting a vulnerability

Please do **not** open a public issue for security problems.

Report privately through GitHub: open the repository's **Security** tab and choose **Report a vulnerability** ([private vulnerability reporting](https://github.com/njbinbin-piscis/AgentZ/security/advisories/new)). Include:

- the affected version and OS,
- what an attacker can do and the conditions needed,
- steps or a proof of concept to reproduce,
- any suggested fix.

Do not include real API keys, tokens or other people's data. We aim to acknowledge a report within 7 days and will keep you updated until it is resolved. Once a fix is released, we credit reporters who want to be credited.

## Scope

In scope are AgentZ itself and the code in this repository, including the vendored kernel under `vendor/` and the extension host. Particularly relevant areas:

- agent tool execution escaping the workspace, approval or policy checks,
- exposure of stored API keys or secrets,
- remote development (SSH, containers, WSL) and the agentz-server control plane,
- the extension host and webview isolation.

Vulnerabilities in third-party extensions, MCP servers or model providers should be reported to their maintainers.
