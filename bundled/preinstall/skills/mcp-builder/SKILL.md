---
name: "mcp-builder"
description: "Build high-quality MCP servers in Python (FastMCP) or TypeScript that let agents use external APIs through well-designed tools. Use when creating an MCP server and registering it in AgentZ."
description_zh: "用 Python 或 TypeScript 构建高质量 MCP 服务器，并在 AgentZ 设置中注册使用。"
version: "1.0.0"
source: agentz
---

# MCP Server Development Guide

## Overview

Create MCP (Model Context Protocol) servers that enable LLMs to interact with external services through well-designed tools. The quality of an MCP server is measured by how well it enables LLMs to accomplish real-world tasks.

---

# Process

## High-Level Workflow

Creating a high-quality MCP server involves four main phases:

### Phase 1: Deep Research and Planning

#### 1.1 Understand Modern MCP Design

**API Coverage vs. Workflow Tools:**
Balance comprehensive API endpoint coverage with specialized workflow tools. Workflow tools can be more convenient for specific tasks, while comprehensive coverage gives agents flexibility to compose operations. Performance varies by client—some clients benefit from code execution that combines basic tools, while others work better with higher-level workflows. When uncertain, prioritize comprehensive API coverage.

**Tool Naming and Discoverability:**
Clear, descriptive tool names help agents find the right tools quickly. Use consistent prefixes (e.g., `github_create_issue`, `github_list_repos`) and action-oriented naming.

**Context Management:**
Agents benefit from concise tool descriptions and the ability to filter/paginate results. Design tools that return focused, relevant data. Some clients support code execution which can help agents filter and process data efficiently.

**Actionable Error Messages:**
Error messages should guide agents toward solutions with specific suggestions and next steps.

#### 1.2 Study MCP Protocol Documentation

**Navigate the MCP specification:**

Start with the sitemap to find relevant pages: `https://modelcontextprotocol.io/sitemap.xml`

Then fetch specific pages with `.md` suffix for markdown format (e.g., `https://modelcontextprotocol.io/specification/draft.md`).

Key pages to review:
- Specification overview and architecture
- Transport mechanisms (streamable HTTP, stdio)
- Tool, resource, and prompt definitions

#### 1.3 Study Framework Documentation

**Recommended stack:**
- **Language**: TypeScript (high-quality SDK support and good compatibility in many execution environments e.g. MCPB. Plus AI models are good at generating TypeScript code, benefiting from its broad usage, static typing and good linting tools)
- **Transport**: Streamable HTTP for remote servers, using stateless JSON (simpler to scale and maintain, as opposed to stateful sessions and streaming responses). stdio for local servers.

**Load framework documentation:**

- **MCP Best Practices**: [View Best Practices](./reference/mcp_best_practices.md) - Core guidelines

**For TypeScript (recommended):**
- **TypeScript SDK**: Use `web_fetch` to load `https://raw.githubusercontent.com/modelcontextprotocol/typescript-sdk/main/README.md`
- [TypeScript Guide](./reference/node_mcp_server.md) - TypeScript patterns and examples

**For Python:**
- **Python SDK**: Use `web_fetch` to load `https://raw.githubusercontent.com/modelcontextprotocol/python-sdk/main/README.md`
- [Python Guide](./reference/python_mcp_server.md) - Python patterns and examples

#### 1.4 Plan Your Implementation

**Understand the API:**
Review the service's API documentation to identify key endpoints, authentication requirements, and data models. Use `web_search` and `web_fetch` as needed.

**Tool Selection:**
Prioritize comprehensive API coverage. List endpoints to implement, starting with the most common operations.

---

### Phase 2: Implementation

#### 2.1 Set Up Project Structure

See language-specific guides for project setup:
- [TypeScript Guide](./reference/node_mcp_server.md) - Project structure, package.json, tsconfig.json
- [Python Guide](./reference/python_mcp_server.md) - Module organization, dependencies

#### 2.2 Implement Core Infrastructure

Create shared utilities:
- API client with authentication
- Error handling helpers
- Response formatting (JSON/Markdown)
- Pagination support

#### 2.3 Implement Tools

For each tool:

**Input Schema:**
- Use Zod (TypeScript) or Pydantic (Python)
- Include constraints and clear descriptions
- Add examples in field descriptions

**Output Schema:**
- Define `outputSchema` where possible for structured data
- Use `structuredContent` in tool responses (TypeScript SDK feature)
- Helps clients understand and process tool outputs

**Tool Description:**
- Concise summary of functionality
- Parameter descriptions
- Return type schema

**Implementation:**
- Async/await for I/O operations
- Proper error handling with actionable messages
- Support pagination where applicable
- Return both text content and structured data when using modern SDKs

**Annotations:**
- `readOnlyHint`: true/false
- `destructiveHint`: true/false
- `idempotentHint`: true/false
- `openWorldHint`: true/false

---

### Phase 3: Review and Test

#### 3.1 Code Quality

Review for:
- No duplicated code (DRY principle)
- Consistent error handling
- Full type coverage
- Clear tool descriptions

#### 3.2 Build and Test

**TypeScript:**
- Run `npm run build` to verify compilation
- Test with MCP Inspector (`npx @modelcontextprotocol/inspector`, run through `shell`)

**Python:**
- Verify syntax: `python -m py_compile your_server.py`
- Test with MCP Inspector

See language-specific guides for detailed testing approaches and quality checklists.

---

### Phase 4: Register and Evaluate

#### 4.1 Register the server in AgentZ

Add the server in **Settings, MCP Servers**: use `stdio` with `command` and `args` for a local server (put keys in `env`), or `http` with `url` and `headers` for a remote one. Enable it and confirm it connects. To give it only to a specific agent, keep it disabled globally and list its name in that agent's `mcp_servers`. The mcp-integration skill covers the details.

#### 4.2 Write evaluation questions

Test whether an agent can answer realistic questions with your server. Write about 10 questions and solve each yourself first.

Each question must be:
- **Independent**: not dependent on other questions
- **Read-only**: answerable with non-destructive tool calls
- **Complex**: needs several tool calls and real exploration
- **Realistic**: something a person would actually ask
- **Verifiable**: one clear answer that can be checked by string comparison
- **Stable**: the answer will not change over time

Keep them as simple question and answer pairs in a Markdown file next to the server. Run them in a fresh chat with the server enabled, compare answers, and look at which tool calls failed or were wasted. Fix unclear tool names, descriptions, missing filters and unhelpful error messages, then rerun.

---

# Reference Files

Load these resources as needed during development:

- [MCP Best Practices](./reference/mcp_best_practices.md): naming, response formats, pagination, transport selection, security and error handling.
- [Python Implementation Guide](./reference/python_mcp_server.md): FastMCP initialization, Pydantic models, `@mcp.tool` registration, working examples and a quality checklist.
- [TypeScript Implementation Guide](./reference/node_mcp_server.md): project structure, Zod schemas, `server.registerTool`, working examples and a quality checklist.
- MCP protocol: start with `https://modelcontextprotocol.io/sitemap.xml` and fetch pages with a `.md` suffix using `web_fetch`.
- SDK READMEs: `https://raw.githubusercontent.com/modelcontextprotocol/python-sdk/main/README.md` and `https://raw.githubusercontent.com/modelcontextprotocol/typescript-sdk/main/README.md`.
