---
name: "tencent-docs"
description: "Create, search, read and edit Tencent Docs (smart docs, Word, Excel, slides, mind maps, flowcharts, smart sheets) through the Tencent Docs MCP server. Use when the user works with Tencent Docs."
description_zh: "通过腾讯文档 MCP 创建、搜索、读取和编辑智能文档、Excel、幻灯片、思维导图、流程图和智能表格"
version: "1.0.0"
source: agentz
---

# Tencent Docs MCP Guide

The Tencent Docs MCP server exposes tools for creating, querying and editing several kinds of online documents.

## Detailed references

For per-tool call examples, parameters, return values, enums and structures, read:
- `references/api_references.md` - complete examples, parameters and return values for all general tools, plus API structures and enums.
- `references/smartsheet_references.md` - SmartSheet reference: field type enums, field value formats, typical workflows and every `smartsheet.*` tool.
- `references/smartcanvas_references.md` - SmartCanvas reference: element types, rich text format enums, typical workflows and every `smartcanvas.*` tool.

These reference files are written in Chinese (the language of the Tencent Docs API).

## Setup

This skill needs the Tencent Docs MCP server to be configured in AgentZ under **Settings -> MCP Servers**.

1. Get a personal token at https://docs.qq.com/open/auth/mcp.html
2. Add an MCP server with URL `https://docs.qq.com/openapi/mcp` and this HTTP header:

```json
{
  "Authorization": "<your token>"
}
```

The header key must be exactly `Authorization`. Other names (`token`, `auth`, `X-Token`, ...) fail authentication.

3. Name the server `tencent-docs`. Once it is enabled, its tools are available to the agent.

If the user has not configured a token, point them to the link above first; otherwise every tool call fails authentication.

## Error codes

| Code | Meaning | Fix |
|------|---------|-----|
| **400006** | Token authentication failed | Check that the header key is `Authorization` and the token is correct. Get a new token at https://docs.qq.com/open/auth/mcp.html |
| **400007** | VIP permission required | The account needs a VIP plan: https://docs.qq.com/vip?immediate_buy=1 |

## Calling the tools

Tool arguments must be a JSON object, never a JSON string such as `"{ ... }"`.

The tool list below is guidance only. The tools actually exposed by the server are authoritative; if a tool is missing or its parameters differ, list the server's tools first and follow what it reports.

| Tool | Purpose |
|------|---------|
| `create_smartcanvas_by_markdown` | Create a smart doc (preferred for general documents) |
| `create_excel_by_markdown` | Create an Excel sheet |
| `create_slide_by_markdown` | Create slides |
| `create_mind_by_markdown` | Create a mind map |
| `create_flowchart_by_mermaid` | Create a flowchart |
| `create_word_by_markdown` | Create a Word document |
| `query_space_node` | Query space nodes |
| `create_space_node` | Create a space node |
| `delete_space_node` | Delete a space node |
| `search_space_file` | Search files in a space |
| `get_content` | Read document content |
| `batch_update_sheet_range` | Batch update a sheet |
| `smartcanvas.*` | Smart doc element operations (pages, text, headings, to-dos); see `references/smartcanvas_references.md` |
| `smartsheet.*` | Smart sheet operations (sheets, views, fields, records); see `references/smartsheet_references.md` |

## Choosing a document type

Preferred: smart doc (SmartCanvas).

- **New documents:** use `create_smartcanvas_by_markdown` first. It lays out content nicely, supports headings, paragraphs, lists, tables, code blocks, quotes and images, and renders the same on every platform.
- **Editing existing smart docs:** use the `smartcanvas.*` tools to add, query, modify and delete content.

Decision tree:

```
What do you need to create?
|
+- General document content (reports, notes, articles)
|   -> create_smartcanvas_by_markdown (preferred)
|
+- Edit or append to an existing smart doc
|   -> smartcanvas.* tools
|
+- Data table (calculation, filtering, statistics)
|   -> create_excel_by_markdown
|
+- Presentation (page-by-page)
|   -> create_slide_by_markdown
|
+- Hierarchical knowledge (outline, knowledge graph)
|   -> create_mind_by_markdown
|
+- Process or architecture diagram (flowchart, sequence)
|   -> create_flowchart_by_mermaid
|
+- Structured data management (views, fields, boards)
|   -> smartsheet.* tools
|
+- Traditional Word export
    -> create_word_by_markdown (only when explicitly requested)
```

## Supported document types

| Type | doc_type | Rating | Notes |
|------|----------|--------|-------|
| Smart doc | smartcanvas | preferred | Good layout, rich components |
| Excel | excel | good | Data tables |
| Slides | slide | good | Presentations |
| Mind map | mind | good | Knowledge graphs |
| Flowchart | flowchart | good | Process diagrams |
| Word | word | fair | Traditional format, plain layout |
| Form | form | fair | Data collection |
| Smart sheet | smartsheet | good | Structured tables with multiple views and field management |
| Whiteboard | board | fair | Online whiteboard |

## Tools

### 1. Creating documents

All `create_*_by_markdown` tools and `create_flowchart_by_mermaid` accept an optional `parent_id` to create the document in a specific folder; without it the document goes to the root.

- **`create_smartcanvas_by_markdown`** (preferred): creates a smart doc from Markdown. Use for documents, reports, notes, articles, meeting minutes, plans, technical docs, tutorials - anything that should look good.
- **`create_excel_by_markdown`**: creates an Excel sheet from a Markdown table. Use for data reports and statistics needing formulas or filtering.
- **`create_slide_by_markdown`**: creates slides from Markdown with a fixed hierarchy: `#` title -> `##` section -> `###` page -> `-` paragraph -> indented sub-items as body text. Use for presentations, project reports and training material.
- **`create_mind_by_markdown`**: creates a mind map; heading levels and nested lists express the structure.
- **`create_flowchart_by_mermaid`**: creates a flowchart from Mermaid syntax. The Mermaid content must be entirely in English. Use for flowcharts, sequence diagrams and architecture diagrams.
- **`create_word_by_markdown`**: creates a Word document. Use only when the user explicitly asks for Word; otherwise use smart docs.

### 2. Space management

- **`query_space_node`**: queries the node tree (folders and documents). Paginated, 20 items per page.
- **`create_space_node`**: creates a node: folder (`wiki_folder`), online doc (`wiki_tdoc`) or link (`link`).
- **`search_space_file`**: searches documents by keyword in title and content. Paginated, 40 per page. It only finds document nodes (word, excel, slide, ...), not folders; to find a folder, walk the tree with `query_space_node`.
- **`delete_space_node`**: deletes a node. `remove_type=current` (default) deletes only that node and re-attaches its children to the parent; `remove_type=all` deletes the node and all descendants recursively - use with care.

### 3. Document operations

- **`get_content`**: returns the full text of a document given its `file_id`.
- **`batch_update_sheet_range`**: batch updates cells in an Excel sheet; data is appended from the end of the sheet and does not overwrite existing content.

### 4. Smart doc (SmartCanvas) element tools

Seven `smartcanvas.*` tools add, query, modify and delete pages, text, headings and to-do items:

- `smartcanvas.create_smartcanvas_element`: add a Page, Text, Heading or Task. `Text`, `Task` and `Heading` must be attached under a `Page` parent (`parent_id` required). A `Page` may omit the parent and goes to the root. A parent cannot be a `Heading`.
- `smartcanvas.get_element_info`: batch query element details (content, type, parent/child relations).
- `smartcanvas.get_page_info`: list all elements inside a page, paginated with the `cursor` parameter; `is_over=true` means everything has been fetched.
- `smartcanvas.get_top_level_pages`: list the top-level pages (direct children of the root) to learn the document structure.
- `smartcanvas.update_element`: batch modify text, format, heading level, page title and other properties.
- `smartcanvas.delete_element`: batch delete elements. Deleting a Page also deletes all of its children.
- `smartcanvas.append_insert_smartcanvas_by_markdown`: append Markdown content to the end of an existing smart doc.

Full descriptions, element type definitions, heading level and color enums, rich text format and sample workflows are in `references/smartcanvas_references.md`.

### 5. Smart sheet (SmartSheet) tools

Twelve `smartsheet.*` tools add, query, modify and delete sheets, views, fields and records. Every one needs `file_id` and `sheet_id`; call `smartsheet.list_tables` first to get the `sheet_id`.

Full descriptions, field definitions, field type enums, value formats and sample workflows are in `references/smartsheet_references.md`.

## Common workflows

### Create a general document (recommended)

1. Call `create_smartcanvas_by_markdown`.
2. Read `file_id` and `url` from the result.

### Edit an existing smart doc

1. Call `smartcanvas.get_top_level_pages` to learn the page structure.
2. Use the `smartcanvas.*` tools as needed:
   - Append content: `smartcanvas.append_insert_smartcanvas_by_markdown`
   - Add elements: `smartcanvas.create_smartcanvas_element`
   - Query elements: `smartcanvas.get_element_info` / `smartcanvas.get_page_info`
   - Modify elements: `smartcanvas.update_element`
   - Delete elements: `smartcanvas.delete_element`

### Put a document in a specific folder

1. Call `query_space_node` to find the target folder.
2. Call `create_space_node` to create the document node there (prefer doc_type `smartcanvas`).

### Search and read a document

1. Call `search_space_file`.
2. Take the `node_id` from the result (it is also the `file_id`).
3. Call `get_content`.

### Build a task tracker from scratch (smart sheet)

1. `smartsheet.list_tables` to get the `sheet_id`.
2. `smartsheet.add_fields` to add columns (task name, priority, due date, ...).
3. `smartsheet.add_records` to write data in batch.
4. Optionally `smartsheet.add_view` with `view_type=2` for a kanban view.

### Query and update records

1. `smartsheet.list_tables` to get the sheet.
2. `smartsheet.list_records` to get `record_id` values.
3. `smartsheet.update_records` with the `record_id` and new field values.

More smart sheet workflows are in `references/smartsheet_references.md`.

## Notes

- Default to smart docs: unless the user asks for another format, create new documents with `create_smartcanvas_by_markdown` and edit existing smart docs with `smartcanvas.*`.
- `delete_space_node` with `remove_type=all` is recursive; confirm with the user first.
- Markdown content is UTF-8; special characters need no escaping.
- Slides must follow the hierarchy above, with 2-4 paragraph headings per page.
- Paginated queries return 20-40 records per page; use `has_next` to check for more.
- `node_id` is also the document's `file_id`.
- Mermaid content for `create_flowchart_by_mermaid` must be entirely English.
- Smart doc elements: `Text`, `Heading` and `Task` must sit under a `Page`, and `parent_id` must be a Page element id. Call `smartcanvas.get_top_level_pages` first.
- Smart sheet: `update_fields` cannot change `field_type`, but the original value must still be passed.
- Record field values have different formats per field type; see `references/smartsheet_references.md`.
