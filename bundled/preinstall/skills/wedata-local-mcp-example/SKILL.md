---
name: "wedata-local-mcp-example"
description: "Generate local function_tool wrappers that call the remote WeData MCP Server, and register them into the Agent. Detects user intent automatically and generates only the relevant tool — execute_sql_query for data queries, search_table_by_name only for explicit schema/metadata requests. Most '查表' or '查表详情' mean the user wants actual data, not metadata. Trigger on: '查表', '查表数据', '执行SQL', '跑SQL', 'SELECT', '生成工具', 'run query', 'execute sql', '查表结构', '表Schema', or when called from wedata-quickstart Option 2. If the user mentions tables, SQL, or querying data in the WeData context, this skill is likely what they need."
description_zh: "Wedata3.0 官方插件 —— Data+AI一体化平台，内置Wedata各种能力调用的Skills，通过DataAgents相关Skills可以快速开发调用Wedata引擎、数据、AI资源的数据智能体，并可自动部署到云端直接使用。"
version: "1.0.0"
source: codebuddy
source_plugin: "wedata3"
---

> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.

## Path Convention

**Note**: `<skill_dir>` is the base directory path prefix of this skill at load time (do not hardcode paths). All references to files inside this skill (templates, resources, etc.) use `<skill_dir>` as the prefix.

## Goal

Based on user intent, generate **one or both** local `function_tool`s and register them in `agent_server/agent.py`:

1. **`search_table_by_name`** — When the user only provides a table name (e.g. `t_corp`) without the full `CatalogName.SchemaName.TableName` path, this tool searches across all Catalogs and Schemas in a workspace to locate the table and return its metadata. If the user already knows the full three-part path, the remote MCP `GetTable` tool is more efficient and should be preferred.
2. **`execute_sql_query`** — Execute read-only SQL queries in a workspace and return result download URLs.

Both tools wrap remote WeData MCP Server calls via local orchestration logic.

---

## Intent Detection — Which tool(s) to generate

Detect the user's intent automatically from their message rather than presenting a menu — most users already know what they want, and a menu adds unnecessary friction.

### Detection rules

| User intent signals | Tool to generate |
|---|---|
| "table schema", "describe table columns", "get column/field info", "field definitions", "column definitions" — **only** when user explicitly asks for schema/column/field metadata **and** only provides a table name without the full three-part path (CatalogName.SchemaName.TableName). If the user gives the full path, prefer the remote MCP `GetTable` tool instead. | **`search_table_by_name` only** |
| "look up table", "table details", "table data", "check table XX", "execute SQL", "query data", "run SQL", "SELECT", "sql query", "run query", "execute sql", mentions running/executing queries, fetching data, or looking up table content | **`execute_sql_query` only** |
| "generate tools", "generate all tools", "I want both", "both tools", ambiguous or general tool generation request from quickstart flow | **Both tools** |
| Called from **wedata-quickstart** skill Option 2 (generate example tools) | **Both tools** |

**Key principle**: Most "look up table" requests mean the user wants to **see actual data**, not metadata. Only generate `search_table_by_name` when the user **explicitly** asks for schema, column definitions, or metadata structure. In all other cases — including "table details", "check table", "table info" — default to `execute_sql_query`.

Disambiguation:
- "table **schema/structure/field definitions/column info**" → `search_table_by_name` (metadata)
- "look up table" / "table details" / "table data" / "check table XX" / "table info" → `execute_sql_query` (SQL query, the user wants to see data)
- "query data from table XX" / "SELECT" → `execute_sql_query` (SQL query)

---

## Tool 1: search_table_by_name

The lookup flow is fixed:

```text
ListCatalogs(WorkspaceId, Types="TABLE")
→ ListSchemas(CatalogName)
→ ListTables(CatalogName, SchemaName)
→ GetTable(CatalogName, SchemaName, TableName)
```

Use `GetTable` only after `ListTables` finds an exact `Name == table_name` match.

### Source template

```text
<skill_dir>/resources/local_table_tool.py.template
```

### Generated file

```text
agent_server/local_table_tool.py
```

### Required rules

- The MCP configuration in `agent_server/agent.py` (`WEDATA_MCP_ENDPOINT` + `MCP_TOOL_PATHS`) serves a different purpose — it's the runtime server list and may not match the HTTP endpoint these local tools need. Always use the fixed default for `REMOTE_MCP_URL`:
  `(configure your MCP endpoint in AgentZ Settings)
- `ListCatalogs` must pass `Types="TABLE"` directly — without this filter you'll get non-table catalogs mixed in, which breaks the subsequent schema/table lookup chain.
- Only use the current response shape `{"Response": {"Data": {...}}}` — historical flat shapes are no longer returned by the API, and keeping compatibility branches adds dead code that confuses maintenance.
  - `ListCatalogs`, `ListSchemas`, `ListTables` → read list from `Response.Data.Items`
  - `GetTable` → read detail from `Response.Data.Table`
- `ListTables` is only for discovery. Do **not** use `TableType` (such as `Managed`) as the object kind.
- Call `GetTable` only after a matching table candidate is found.
- If `GetTable` fails after a match, it is acceptable to fall back to the matched `ListTables` item.

### Remote MCP tools used by search_table_by_name

| Tool | Purpose | Key Params | Pagination |
|------|---------|------------|------------|
| `ListCatalogs` | List TABLE catalogs in a workspace | `WorkspaceId`, `Types="TABLE"` | Yes (`MaxResults`, `PageToken`) |
| `ListSchemas` | List schemas in a catalog | `CatalogName`, `WorkspaceId` | Yes (`MaxResults`, `PageToken`) |
| `ListTables` | List table candidates in a schema and match by exact `Name` | `CatalogName`, `SchemaName`, `WorkspaceId` | Yes (`MaxResults`, `PageToken`) |
| `GetTable` | Fetch final detail for the matched table | `CatalogName`, `SchemaName`, `TableName`, `WorkspaceId` | No |

---

## Tool 2: execute_sql_query

The execution flow is:

```text
ListComputeResources(WorkspaceId) [always call first]
→ Auto-select analytical compute resource (prefer analytical type, i.e. ResourceType=3)
→ SubmitJob(WorkspaceId, ComputeResource, Sql)
→ QueryJobStatus(JobId) [poll until terminal state]
→ DownloadJobResult(WorkspaceId, JobId) [on SUCCEEDED, return download URLs only]
```

### Source template

```text
<skill_dir>/resources/local_sql_query_tool.py.template
```

### Generated file

```text
agent_server/local_sql_query_tool.py
```

### Required rules

- Use the same fixed `REMOTE_MCP_URL` default as Tool 1 (same reason — don't infer from `agent.py`).
- **SQL safety**: Only allow read-only SQL statements. The tool must validate that the SQL starts with one of: `SELECT`, `WITH`, `SHOW`, `DESC`, `DESCRIBE`, `EXPLAIN`. Reject all write operations (`INSERT`, `UPDATE`, `DELETE`, `DROP`, `CREATE`, `ALTER`, `TRUNCATE`, `MERGE`).
- **Always** call `ListComputeResources` first, regardless of whether `compute_resource_id` is provided. This ensures the resource exists and is available.
- **Compute resource auto-selection strategy** (when `compute_resource_id` is not provided):
  1. Call `ListComputeResources` to get all available resources.
  2. **Prefer analytical compute resources**: Filter for resources whose `ResourceType` equals `3` (analytical type). These are designed for SQL query workloads.
  3. **Determine active/available status**: The API returns `ResourceStatus` as an integer (e.g. 2=stopped, 3=running, 4=running, 13=deleted) and optionally `ExecAvailableStatus` (1=executable). Prefer `ExecAvailableStatus == 1`; fall back to `ResourceStatus in (3, 4)`. Never call `.upper()` on these — they are int, not string.
  4. If multiple analytical resources are found, auto-select the first one that is active.
  5. If no analytical resource is found, auto-select the first active resource regardless of type; if none are active, use the first resource.
  6. **On failure with auto-selected resource**: If `SubmitJob` fails or the job execution fails, return the full list of available compute resources and ask the user to pick one to retry. Format as:
     ```
     ⚠️ Execution failed with compute resource "{resource_name}" ({resource_id}).

     Available compute resources:
     | # | Resource Name | Resource ID | Type | Status |
     |---|---------------|-------------|------|--------|
     | 1 | ...           | ...         | ...  | ...    |

     Please reply with a resource number or resource ID to retry.
     ```
- If `compute_resource_id` is provided, verify it against the resource list. If found, use it; if not found, still attempt to use it (may be a permissions issue).
- LLMs sometimes serialize Python `None` as the string literal `"None"` when calling function tools, which would fail downstream comparisons. Normalize `"None"` to `None` before processing optional parameters.
- When extracting compute resource fields, try multiple candidate keys in order: `ResourceId` → `Id` → `ResourceName` for ID, and `ResourceName` → `Name` for display name.
- Poll `QueryJobStatus` with a 2-second interval, max 45 attempts (90 seconds timeout).
- Terminal states are: `COMPLETED`, `FAILED`, `KILLING`, `KILLED`, `TIMEOUT`. Stop polling when any of these is reached (aligned with `DlcEngineProxyImpl.isJobFinished`).
- On `COMPLETED`, call `DownloadJobResult` to get download URLs, then automatically fetch the CSV file and parse it into structured data (columns + rows) that the LLM can directly read and analyze. Only the first `_MAX_PREVIEW_ROWS` (default 50) rows are returned to avoid exceeding the LLM's token limit. If CSV download or parsing fails, fall back to returning the raw download URLs.
- On `FAILED`, extract and return the error message from `JobErrMsg` field (fallback to `ErrorMessage`), and include `JobErrCode`. **Also include the full compute resource list so the user can retry with a different resource.**
- On timeout, return the last known status and the `job_id` for manual follow-up.
- Use the same `{"Response": {"Data": {...}}}` response shape as Tool 1 — all remote tools follow this convention. Reuse the identical `_unwrap_response_data` helper to keep the code DRY.

### Response field mapping (all under `Response.Data`)

| Remote Tool | Key fields |
|-------------|------------|
| `ListComputeResources` | `Items` (or `Resources` as fallback) |
| `SubmitJob` | `JobId` |
| `QueryJobStatus` | `JobStatus`, `JobErrMsg`, `JobErrCode` |
| `DownloadJobResult` | `DownloadUrls`, `ResultMeta` |

### Remote MCP tools used by execute_sql_query

| Tool | Purpose | Key Params | Pagination |
|------|---------|------------|------------|
| `ListComputeResources` | Discover available compute resources | `WorkspaceId`, `Keywords` (optional) | No (uses default page) |
| `SubmitJob` | Submit SQL query task (async) | `WorkspaceId`, `ComputeResource`, `Sql`, `Name`, `Desc`, `CatalogName` (opt), `SchemaName` (opt) | No |
| `QueryJobStatus` | Poll job execution status | `JobId`, `WorkspaceId` (opt) | No |
| `DownloadJobResult` | Get result download URLs for the finished query | `WorkspaceId`, `JobId` | No |

---

## Execute in order

### 0. Prerequisite check — is the environment ready?

Before generating any tools, verify the local environment is set up. The generated tools need valid Tencent Cloud credentials and a workspace ID to connect to the remote MCP Server — without them the tools will fail at runtime.

Check:
1. Does `.env.local` exist in the project root?
2. Are `LOCAL_credentials in AgentZ connector settings`, `LOCAL_credentials in AgentZ connector settings`, and `WEDATA_WORKSPACE_ID` filled (not empty, not placeholder values like `<your...>` or `your_`)?

If any of these are missing or unfilled:
> ⚠️ The local environment is not fully configured (missing `.env.local` or required credentials are not filled in).
> Please complete the environment setup before generating tools.

Then hand off to the **wedata-quickstart** skill to guide the user through setup. Do not proceed with tool generation.

If all checks pass, continue.

### 1. Detect user intent

Analyze the user's message to determine which tool(s) to generate using the **Intent Detection** rules above. Decide automatically — asking the user to pick from a list adds friction when the intent is usually clear from their message.

Log your decision:
> 🔍 Based on your request, generating the **{tool_name}** tool.

### 2. Check existing files

Based on the detected intent, check only the relevant files:

- **If generating `search_table_by_name`**: Check whether `agent_server/local_table_tool.py` already exists and contains `search_table_by_name`.
- **If generating `execute_sql_query`**: Check whether `agent_server/local_sql_query_tool.py` already exists and contains `execute_sql_query`.
- Verify `agent_server/agent.py` imports the relevant tool(s) and keeps them in the `tools=[...]` list.
- If the relevant files are already correct, skip file generation and go to startup instructions.
- If any side is missing or incomplete, repair it.

### 3. Generate or repair tool files

For each tool that needs to be generated (based on intent detection):

- Use the corresponding `.py.template` as the source.
- Create or repair the corresponding `agent_server/*.py` file.
- **Only generate the tool(s) matching the detected intent**, not both.

### 4. Register tools in `agent_server/agent.py`

Make these changes:

Only import and register tools whose `.py` file actually exists on disk — importing a missing file causes an `ImportError` that prevents startup entirely.

- Add imports **only for the tool(s) that have a corresponding file on disk**:

```python
# Example: only execute_sql_query was generated this time
from agent_server.local_sql_query_tool import execute_sql_query
```

- In `create_coding_agent()`, the `tools=[...]` list should **only contain tools whose files exist**:
  - If only `agent_server/local_sql_query_tool.py` exists → `tools=[execute_sql_query]`
  - If only `agent_server/local_table_tool.py` exists → `tools=[search_table_by_name]`
  - If both files exist → `tools=[search_table_by_name, execute_sql_query]`
- Keep existing tools and `mcp_servers` entries untouched — they serve other purposes and removing them would break unrelated features.
- The agent's `instructions` is dynamically built by `build_agent_instructions(app_name, workspace_id, region)` using platform env vars. Replacing it with a hardcoded string would lose the dynamic configuration. The local tools carry their own `description_override`, so the LLM discovers them without needing explicit mentions in the system prompt.
- `name` uses `WEDATA_APP_NAME` (env var); `model` is passed as a parameter. Hardcoding these would break environment-specific deployments.

Example (when only `execute_sql_query` is generated):

```python
from agent_server.local_sql_query_tool import execute_sql_query

def create_coding_agent(
    model: str,
    mcp_servers: list | None = None,
) -> Agent:
    instructions = build_agent_instructions(app_name=WEDATA_APP_NAME, workspace_id=WEDATA_WORKSPACE_ID, region=WEDATA_REGION)
    return Agent(
        name=WEDATA_APP_NAME,
        instructions=instructions,
        model=model,
        mcp_servers=mcp_servers or [],
        tools=[execute_sql_query],
    )
```

Example (when both files exist):

```python
from agent_server.local_table_tool import search_table_by_name
from agent_server.local_sql_query_tool import execute_sql_query

    ...
    tools=[search_table_by_name, execute_sql_query],
```

Adjust the example to match the real file if the project already contains additional tools or parameters.

### 5. Provide the startup command to the user

Tell the user to run:

```bash
./scripts/start.sh
```

Then tell the user:

- The service runs in the foreground.
- Press `Ctrl+C` to stop it.
- Access the app at `http://localhost:8080` or the configured `WEDATA_APP_PORT`.
- **Show only the relevant test example(s)** based on which tool was generated:
  - If `search_table_by_name`: `Look up table details for table xxx in workspace xxx`
  - If `execute_sql_query`: `Execute SELECT * FROM xxx.xxx.t_corp LIMIT 10 in workspace xxx`
- Logs: `data/logs/backend.log`

---

## Quick validation

- Ask the user to start the service in a terminal with:
  `./agent-openai-agents-sdk/scripts/start.sh --no-mlflow`
- **Only test the generated tool(s)**:
  - If `search_table_by_name` was generated: `Look up table details for table xxx in workspace xxx`
  - If `execute_sql_query` was generated: `Execute SELECT 1 AS test_col in workspace xxx`
- Confirm the generated tool(s) return structured JSON results.
- Check logs at `data/logs/backend.log` if any tool fails.

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `Credential headers not found` | Ensure `.env.local` contains valid local credentials, or confirm the request headers carry the required STS credentials |
| No TABLE catalogs returned | Verify `workspace_id` and permissions |
| Remote MCP connection fails | Replace `REMOTE_MCP_URL` with the official MCP service URL copied from the WeData platform |
| Table not found but exists | Check exact table name casing and workspace |
| No compute resources found | Verify `workspace_id` has compute resources configured |
| SQL rejected by safety check | Only read-only SQL is allowed (SELECT, WITH, SHOW, DESC, EXPLAIN). Use WeData SQL console for write operations |
| SQL execution timeout | The task may still be running. Use the returned `job_id` to check status later |
| SubmitJob fails with auto-selected resource | The auto-selected compute resource may not be suitable. Try an analytical type resource, or let the user pick from the returned candidate list |