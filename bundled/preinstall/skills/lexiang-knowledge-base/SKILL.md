---
name: "lexiang-knowledge-base"
description: "Read, search, create and edit documents in the Lexiang (lexiangla.com) knowledge base through its MCP server. Use when the user mentions Lexiang or shares a lexiangla.com link."
description_zh: "通过 MCP 访问乐享知识库：读取、搜索、创建和编辑文档，用户提到乐享或给出 lexiangla.com 链接时使用"
version: "1.0.0"
source: agentz
---

# Lexiang Knowledge Base

Work with the Lexiang (乐享, lexiangla.com) enterprise knowledge base through its MCP server: read and search documents, browse teams, spaces and entries, create, edit and move documents, manage attachments.

Use this skill when the user mentions Lexiang, 乐享, a knowledge base or document space on Lexiang, or gives a link whose host is `lexiangla.com`. When the MCP schema returned by the server differs from this document, trust the server schema.

## Setup

1. Open https://lexiangla.com/mcp and sign in. Copy your `company_from` value and your access token (format `lxmcp_xxx`).
2. In AgentZ open **Settings → MCP Servers** and add a streamable-HTTP server named `lexiang` with this URL:

```
https://mcp.lexiang-app.com/mcp?company_from=<your_company>&access_token=<your_token>&preset=meta
```

3. Verify by asking the agent to list your Lexiang teams (`team_list_teams`).

Other supported authentication: OAuth 2.0 (interactive clients) and an `Authorization: Bearer lxmcp_xxx` header. The `mcporter` CLI can also call the server from `shell` if the user prefers it (`mcporter call lexiang.space_list_spaces limit=5`).

## Tool discovery (progressive loading)

The server runs in `preset=meta` mode and exposes only four meta tools. Every business tool must be invoked through `call_tool`.

| Tool | Purpose |
| --- | --- |
| `list_tool_categories` | List all tool categories and the tools in each |
| `search_tools` | Search tools by keyword or category |
| `get_tool_schema` | Get the full parameter definition of one tool |
| `call_tool` | Invoke a business tool: `call_tool(tool_name, arguments)` |

Recommended flow: `list_tool_categories` → `search_tools(query="team list")` → `get_tool_schema(tool_name="team_list_teams")` → `call_tool(tool_name="team_list_teams", arguments={"limit": 5})`.

All tools accept an optional `_mcp_fields` string array to select returned fields (nested paths such as `entry.id` are supported). Use it to cut token usage, for example `["id", "root_entry_id", "name"]`.

## Data model

| Concept | Meaning |
| --- | --- |
| Team | Top-level organization unit; owns several spaces |
| Space | Knowledge base container; has a `root_entry_id` as its root node |
| Entry | Content unit in a space: `page`, `folder` or `file`, arranged as a tree via `parent_id` |
| File | Attachment-type entry (PDF, Word, images, ...) |

```
Team → Space → Entry (tree rooted at root_entry_id)
                 ├── page
                 ├── folder
                 └── file
```

Common parameters: `entry_id` (document/page id), `parent_entry_id` (parent for new documents), `block_id` (client-generated temporary id; the server returns the real id).

## URL rules

Domain: `https://*.lexiangla.com` (redirects by `company_from`).

| URL | Extract |
| --- | --- |
| `{domain}/spaces/{space_id}` | `space_id` = path segment after `spaces/` |
| `{domain}/pages/{entry_id}` | `entry_id` = path segment after `pages/` |
| `{domain}/t/{team_id}/spaces` | `team_id` = path segment after `t/` |

Ignore query parameters such as `?company_from=xxx` when extracting ids. Search results link as: `kb_page` → `https://lexiangla.com/pages/<target_id>`; `kb_file` / `kb_video` → `https://lexiangla.com/teams/<team_id>/docs/<target_id>`.

## Clarify ambiguous requests

If a request could target another system ("search documents", "check updates", "create a new document", "write it to Lexiang" with no target), do not guess. Ask which platform (Lexiang or something else), which operation (search/read, create/edit, manage structure) and the scope (team, space, time range). Ask for a space link or exact name before writing.

## Write safety rules

Any create, modify, delete, move or upload must be based on a target the user explicitly provided. Never browse the team or space list and pick a write target yourself, and never choose one because its name "looks right".

Write operations and their tools:

| Operation | Tools |
| --- | --- |
| Create document or folder | `entry_create_entry`, `entry_import_content` |
| Modify content | `entry_import_content_to_entry`, `block_update_block`, `block_update_blocks`, `block_create_block_descendant` |
| Delete | `block_delete_block`, `block_delete_block_children` |
| Move or rename | `block_move_blocks`, `entry_rename_entry` |
| Upload | `file_apply_upload`, `file_commit_upload` |
| Import link | `file_create_hyperlink` |

A write is allowed only when one of these holds:

1. The user gave a URL containing a `space_id`, `entry_id` or `team_id`.
2. The user gave an explicit id (for example "write to the space with id db13925815e1417a").
3. The user gave an exact name and, after you looked it up and echoed it back ("Found space 'Frontend Guidelines' (space_id: xxx, team: yyy). Write here?"), confirmed.

Ambiguous phrases need a follow-up question: "write it to Lexiang", "import into my knowledge base", "go ahead" with several candidate targets, "put it in that team". When the target is unclear, ask for one of: a space link (preferred), a space name (you search and the user confirms), or a `space_id`.

Read-only tools are not restricted: `team_list_teams`, `space_list_spaces`, `space_describe_space`, `entry_list_children`, `block_list_block_children`, `search_kb_search`, `search_kb_embedding_search`, `space_list_recently_spaces`, `entry_list_latest_entries`.

## Tool overview

All parameters must come from `get_tool_schema`; this list only shows what exists.

- Knowledge base management: `entry_create_entry`, `entry_import_content` (creates a NEW document from Markdown/HTML, cannot overwrite), `entry_import_content_to_entry` (into an existing page, overwrite or append), `entry_list_latest_entries`, `entry_rename_entry`.
- Browsing: `entry_list_children`, `entry_describe_entry`, `entry_list_parents` (breadcrumb path).
- Search and discovery: `search_kb_search` (keyword), `search_kb_embedding_search` (semantic; params `query`, optional `space_id`; good when you remember the gist but not the title, then read exactly with `entry_describe_entry`), `team_list_teams`, `space_list_spaces`, `space_describe_space`, `space_list_recently_spaces`.
- Blocks: `block_convert_content_to_blocks`, `block_create_block_descendant`, `block_update_block`, `block_update_blocks`, `block_move_blocks` (target parent cannot be a leaf), `block_delete_block_children`, `block_delete_block`, `block_describe_block`, `block_list_block_children`.
- Attachments: `file_apply_upload` (returns `upload_url` and `session_id`), `file_commit_upload`.
- External import: `file_create_hyperlink` imports a WeChat official-account article (`url`, `space_id`, `parent_entry_id`).

Key return values: `space_describe_space` returns `root_entry_id`, `team_id`, `name`. Team → spaces is a two-step lookup: `team_list_teams`, then `space_list_spaces(team_id)`.

## Workflows

### Write a document to a space the user linked

Only run this when the user supplied the space link or id.

1. Extract `space_id` from the URL.
2. Call `space_describe_space(space_id)` to get `root_entry_id`.
3. Import Markdown as a new document: `entry_import_content(space_id, parent_id=root_entry_id, name, content, content_type="markdown")`. `space_id` and `parent_id` must both be passed; `root_entry_id` means the space root.
4. Build the link from the returned `entry.id`: `https://lexiangla.com/pages/<entry.id>`.

To create an empty page instead: `entry_create_entry(parent_entry_id=root_entry_id, name, entry_type="page")`.

### Structured document with blocks

Create blocks with `block_create_block_descendant(entry_id, descendant=[...], children=[...])`. Each block has a temporary `block_id`, a `block_type` and its content; container blocks list their `children` ids.

```json
{
  "entry_id": "doc123",
  "descendant": [
    {"block_id": "h1", "block_type": "h1", "heading1": {"elements": [{"text_run": {"content": "Project doc"}}]}},
    {"block_id": "tip", "block_type": "callout", "callout": {"color": "#FFF3E0"}, "children": ["tip_p"]},
    {"block_id": "tip_p", "block_type": "p", "text": {"elements": [{"text_run": {"content": "Important note"}}]}},
    {"block_id": "li1", "block_type": "bulleted_list", "bulleted": {"elements": [{"text_run": {"content": "Feature one"}}]}}
  ],
  "children": ["h1", "tip", "li1"]
}
```

### Read and update blocks

- Read: `block_list_block_children(entry_id, with_descendants=true)`.
- Batch update: `block_update_blocks(entry_id, updates={"<actual_block_id>": {"update_text_elements": {"elements": [{"text_run": {"content": "New text"}}]}}})`.

### Upload a file

1. `file_apply_upload(parent_entry_id, name, size)` → `upload_url`, `session_id`. The size must be the exact byte size of the file.
2. Upload the bytes with `shell`: `curl -X PUT "$UPLOAD_URL" --data-binary "@file.md"`.
3. `file_commit_upload(session_id)`.

## Block structure rules

- Leaf blocks (no `children`): headings h1–h5, `code`, `image`, `divider`, `mermaid`, `plantuml`.
- Container blocks (must specify `children`): `callout`, `table`, `table_cell`, `column_list`, `column`, `toggle`.
- Block ids sent by the client are temporary; the server maps them to real ids.

## Key cautions

1. Block ids are temporary and get remapped by the server.
2. Headings, code blocks and images cannot have children.
3. Callouts, tables and column lists must specify children.
4. Upload size must be the exact file size in bytes.

## Bundled resources

References (in `references/`, Chinese-language domain notes): `block-schema.md` (block types), `mcp-examples.md` (complex block examples), `markdown-to-block.md`, `block-update.md`, `content-reorganize.md`, `folder-sync.md`, `markdown-import.md`, `common-errors.md` (troubleshooting), `doc-templates.md`, `theme-config.md`.

Helper scripts (in `scripts/`; see `scripts/README.md`): `sync-folder.ts` (incremental folder sync), `block-helper.ts` (block builders), `mcp-validator.ts` (parameter validation), `upload-files.py`.

Check `references/common-errors.md` first when something fails (Markdown import mode, block id mapping, table cell ordering, versioning).

More: Lexiang https://lexiangla.com, MCP protocol https://modelcontextprotocol.io.
