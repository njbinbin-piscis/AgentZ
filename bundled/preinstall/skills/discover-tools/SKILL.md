---
name: "discover-tools"
description: "Discover available MCP servers and tools in the current WeData workspace. Use when: (1) User asks 'what tools are available', 'what MCP servers are configured', 'show me available tools', (2) Before configuring agent MCP connections, (3) User says 'discover', 'find tools', 'list MCP', 'MCP servers', '发现工具', '查看MCP', '有哪些工具', '可用工具', '工具列表', '查看可用的MCP', (4) User wants to know what remote MCP services are available in their workspace."
description_zh: "Wedata3.0 官方插件 —— Data+AI一体化平台，内置Wedata各种能力调用的Skills，通过DataAgents相关Skills可以快速开发调用Wedata引擎、数据、AI资源的数据智能体，并可自动部署到云端直接使用。"
version: "1.0.0"
source: codebuddy
source_plugin: "wedata3"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

# Discover Available MCP Tools

**Run tool discovery** to understand what MCP servers are available in the current WeData workspace before configuring agent connections.

## Path Convention

**Note**: `<skill_dir>` is the base directory path prefix of this skill at load time (do not hardcode paths). All references to files inside this skill (scripts, etc.) use `<skill_dir>` as the prefix.

## Run Discovery

```bash
python <skill_dir>/scripts/discover_mcp_servers.py
```

**Options:**
```bash
# Output as JSON
python <skill_dir>/scripts/discover_mcp_servers.py --format json

# Save results to file
python <skill_dir>/scripts/discover_mcp_servers.py --output tools.md
```

## What Gets Discovered

| Field | Description |
|-------|-------------|
| **ServerName** | MCP server display name |
| **ServerUrl** | MCP server endpoint URL (streamable-http) |
| **ServerType** | Server type (e.g. `external`, `internal`) |
| **Status** | Server status (`active`, etc.) |
| **TransportType** | Protocol type (e.g. `streamable-http`) |
| **Description** | Server description |

## AI Execution Instructions

When this skill is triggered, follow these steps in order.

### Step 1: Check prerequisites

Read `.env.local` in the project root. Verify these fields are set (not placeholder values):

- `LOCAL_credentials in AgentZ connector settings` — must not be `your-secret-id`
- `LOCAL_credentials in AgentZ connector settings` — must not be `your-secret-key`
- `WEDATA_WORKSPACE_ID` — must not be `your-workspace-id`
- `WEDATA_SDK_ENDPOINT` — should have a valid endpoint (defaults to `wedata.tencentcloudapi.com` if not set)

If `.env.local` does not exist or required fields are missing/placeholder:
- Tell the user to run the **wedata-quickstart** skill first, or manually configure `.env.local`
- Do NOT proceed until configuration is confirmed

### Step 2: Run the discovery script

Execute:

```bash
python <skill_dir>/scripts/discover_mcp_servers.py
```

The script will:
1. Read credentials and workspace ID from `.env.local` (via `wedata.base.config`)
2. Call the `ListMCPServerConfigs` API with TC3-HMAC-SHA256 authentication
3. Output a formatted markdown table of available MCP servers

If the script fails with an authentication error, check that the credentials in `.env.local` are correct and not expired.

### Step 3: Present results

Show the discovery results to the user. For each MCP server found, highlight:
- **Server Name** and **Description** — what it does
- **Server URL** — the endpoint to connect to
- **Status** — whether it's active
- **Transport Type** — the protocol (usually `streamable-http`)

If no MCP servers are found, tell the user their workspace doesn't have any MCP servers configured yet, and suggest they configure one in the WeData console.

### Step 4: Suggest next steps

After showing results, suggest relevant actions:

1. **Add MCP tools to your agent** — See **modify-agent** skill for code examples, or use **add-tools** skill to quickly add paths to `MCP_TOOL_PATHS`
2. **Test locally** — Start the app with `./scripts/start.sh` (see **wedata-quickstart** skill if not yet set up)

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Authentication error | Verify `LOCAL_credentials in AgentZ connector settings` and `LOCAL_credentials in AgentZ connector settings` in `.env.local` are correct |
| No servers found | Check that MCP servers are configured in the WeData workspace console |
| Network error | Verify `WEDATA_SDK_ENDPOINT` is reachable and correct |
| Permission denied | Ensure your credentials have access to the specified workspace |