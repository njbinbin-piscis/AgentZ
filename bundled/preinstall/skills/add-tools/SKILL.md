---
name: "add-tools"
description: "Add MCP server tools to the WeData agent configuration. Use when: (1) User wants to add, connect, or configure an MCP server for the agent, (2) User has discovered MCP servers and wants to use them, (3) User says 'add tool', 'add MCP', 'connect MCP', 'use this MCP server', '添加工具', '添加MCP', '接入MCP', '把这个MCP加上', '连接工具', (4) User provides an MCP server URL and wants the agent to use it, (5) After running discover-tools, user wants to wire up the results. If the user wants to *discover* what's available, use discover-tools instead. If the user wants to *generate local function_tool wrappers*, use wedata-local-mcp-example instead. This skill is specifically for adding MCP server URLs to the agent's runtime configuration."
description_zh: "Wedata3.0 官方插件 —— Data+AI一体化平台，内置Wedata各种能力调用的Skills，通过DataAgents相关Skills可以快速开发调用Wedata引擎、数据、AI资源的数据智能体，并可自动部署到云端直接使用。"
version: "1.0.0"
source: codebuddy
source_plugin: "wedata3"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# Add MCP Tools to Agent

Add MCP tools to the agent so it can use remote MCP servers at runtime.

## How It Works

In `agent_server/agent.py`, MCP tools are configured using a **base URL + path** pattern:

```python
# ── MCP base URL (read from platform-injected env var) ──
WEDATA_MCP_ENDPOINT = wedata_config.get("WEDATA_MCP_ENDPOINT", "")

# ── MCP tool path list (developers add MCP tools to connect here) ──
MCP_TOOL_PATHS: list[str] = [
    "/api/1.0/mcp/http/sql",
]
```

At runtime, paths are joined to form full URLs: `WEDATA_MCP_ENDPOINT + path`.

Adding a tool = appending a path string to the `MCP_TOOL_PATHS` list. At runtime, the framework automatically:
- `_build_mcp_server_urls()` — joins each path with the base URL to form full URLs
- `_create_mcp_servers()` — creates a `WedataMcpServer` instance for each URL
- Injects credentials automatically (reads `.env.local` locally, reads platform headers in cloud mode)
- `safe_mcp_context()` — connects to each server; failures are skipped without affecting the others

No permission configuration or YAML changes needed.

## AI Execution Instructions

When this skill is triggered, follow these steps in order.

### Step 1: Determine which MCP tool(s) to add

The user might specify what to add in several ways:

| User says | What to do |
|-----------|-----------|
| Provides a complete URL | Extract the path portion (strip the `WEDATA_MCP_ENDPOINT` prefix) and add it to `MCP_TOOL_PATHS` |
| Provides a path (e.g. `/api/1.0/mcp/http/metadata`) | Add it directly to `MCP_TOOL_PATHS` |
| References a discover-tools result (e.g. "add the tavily one") | Look up the matching ServerUrl from the discover-tools output and extract the path portion |
| "Add all of them" | Collect all ServerUrls from the discover-tools output and extract the path portions |
| Vague request (e.g. "I want to use MCP tools") | Run the **discover-tools** skill first to show available tools, then let the user choose |

If the user provides a full URL, extract the path portion (strip the `WEDATA_MCP_ENDPOINT` prefix) and add it to `MCP_TOOL_PATHS`.

If the user hasn't run discover-tools and hasn't provided a specific path or URL, suggest:
> It's recommended to run discover-tools first to see the available MCP Servers in the workspace, then choose which tools to add.

### Step 2: Read current configuration

Read `agent_server/agent.py` and find the `MCP_TOOL_PATHS` list. Note which paths are already configured to avoid duplicates.

### Step 3: Add the path(s)

Edit the `MCP_TOOL_PATHS` list in `agent_server/agent.py` to include the new path(s).

**Rules:**
- Skip paths that are already in the list (tell the user it's already configured)
- Keep one path per line for readability
- Preserve any existing paths — only append, never remove unless the user explicitly asks
- Path should start with `/`

**Example — before:**
```python
MCP_TOOL_PATHS: list[str] = [
    "/api/1.0/mcp/http/sql",
]
```

**Example — after adding a tavily tool:**
```python
MCP_TOOL_PATHS: list[str] = [
    "/api/1.0/mcp/http/sql",
    "/api/1.0/mcp/http/external/f07b2efa1774424096340dd159739",
]
```

### Step 4: Confirm and show next steps

After editing, tell the user:

1. **What was added** — list the new path(s) and their corresponding server names (if known from discover-tools)
2. **Restart to take effect** — the agent needs to be restarted:
   ```bash
   ./scripts/start.sh
   ```
3. **How to verify** — after restart, the agent will log MCP server connection status. Look for:
   ```
   [MCP] N/N MCP Server(s) connected successfully
   ```
4. **Connection failures are non-fatal** — if a newly added server fails to connect, the agent still runs with the other servers. Check the logs for connection error details.

## Edge Cases

### User wants to remove an MCP tool

Read `agent_server/agent.py`, find the path in `MCP_TOOL_PATHS`, and remove it. Remind the user to restart.

### WEDATA_MCP_ENDPOINT is not configured

If `WEDATA_MCP_ENDPOINT` is empty (not set in `.env.local` or on the platform), the runtime will produce an empty URL list and MCP tools will have no effect. Tell the user:
- Local mode: configure `WEDATA_MCP_ENDPOINT` in `.env.local`
- Cloud mode: verify the environment variable is declared in the platform's `app.yaml`

## Troubleshooting

| Issue | Solution |
|-------|----------|
| MCP server fails to connect after adding | Check the path is correct; check `WEDATA_MCP_ENDPOINT` is configured; check logs at `data/logs/app.log` |
| Agent starts but new tool doesn't appear | Verify the path was added to `MCP_TOOL_PATHS`; verify `WEDATA_MCP_ENDPOINT` is not empty; restart with `./scripts/start.sh` |
| Authentication error on the new MCP server | WeData MCP servers use auto-injected credentials; verify `.env.local` has valid credentials in AgentZ connector settings |