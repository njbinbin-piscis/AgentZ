# Skill & Tool Orchestration Rules (`plugin-finder`)

When selecting and combining tools, skills, sub-agents, and connectors in this project:

1. **Capability Discovery**:
   - Check `## Available skills` in the system prompt for domain-specific workflows and read matching `SKILL.md` files via `file_read` before starting complex tasks.
   - Use `api_connector` or configured MCP tools when interacting with external services rather than ad-hoc shell curl scripts.
2. **Minimal-Privilege & High-Precision Tool Selection**:
   - Prefer structured IDE tools (`graph_explore`, `symbol_search`, `impact`, `codebase_search`, `file_read`, `file_edit`, `read_lints`) over generic `shell` commands.
   - Offload broad read-only codebase exploration to `delegate` or `call_fish` sub-agents so the main conversation context remains focused.
