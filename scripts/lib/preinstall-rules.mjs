/**
 * Shared AgentZ preinstall compatibility rules (skills / agents / slash commands).
 */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

export const BUILTIN_TOOLS = new Set([
  "file_read",
  "file_write",
  "file_edit",
  "file_diff",
  "file_list",
  "file_search",
  "code_run",
  "shell",
  "process_control",
  "web_search",
  "web_fetch",
  "email",
  "ssh",
  "memory_store",
  "skill_manage",
  "recall_tool_result",
  "vision_context",
  "pdf",
  "plan_todo",
  "plan_write",
  "plan_mode_ui",
  "pool_org",
  "pool_chat",
  "lsp",
  "read_lints",
  "codebase_search",
  "browser",
  "terminal_read",
  "delegate",
  "chat_ui",
  "chat_ui_patch",
  "chat_ui_listen",
  "api_connector",
]);

export const TOOL_MAP = {
  Read: "file_read",
  Write: "file_write",
  Edit: "file_edit",
  Bash: "shell",
  Grep: "codebase_search",
  Glob: "file_list",
  WebFetch: "web_fetch",
  WebSearch: "web_search",
  Task: "delegate",
  Browser: "browser",
  LSP: "lsp",
};

const TOKEN_ALIASES = [
  [/^(read|read_file|_read_|__read_)$/i, "file_read"],
  [/^(write|write_to_file|_write_)$/i, "file_write"],
  [/^(edit|replace_in_file|multiedit|_edit_)$/i, "file_edit"],
  [/^(bash|execute_command|bash_git|bash_gh|bash_npm|_bash_|bash__plugin___)$/i, "shell"],
  [/^(grep|search|search_file|search_content|_grep_)$/i, "codebase_search"],
  [/^(glob|list_dir|ls|_glob_)$/i, "file_list"],
  [/^(webfetch|_webfetch__)$/i, "web_fetch"],
  [/^(websearch|_websearch_)$/i, "web_search"],
  [/^(askuserquestion|_askuserquestion_)$/i, "chat_ui"],
  [/^(todowrite|todo_write)$/i, "plan_todo"],
  [/^(delete_file)$/i, "file_write"],
  [/^(preview_url)$/i, "browser"],
  [/^(notebookread|bashoutput|killshell)$/i, "shell"],
  [/^(automation_update)$/i, "shell"],
];

const DROP_TOKENS = new Set(["use_skill", "skill", "_skill__"]);
const VENDOR_PLUGIN_PATTERNS =
  /^(wedata|testbuddy|tcase|yottadb|alb-|apple-|blogwatcher|map-|amap|gaode)/i;
const GENERAL_DEV_PLUGINS =
  /^(superpowers|commands-|agents-|plugin-|api-|testing|test-|dev-|git-|workflow)/i;

const PLATFORM_BODY_RE =
  /mcp__|MCPServer|OpenAI Agents SDK|CODEBUDDY_PLUGIN|claude-in-chrome|CodeBuddy CLI/i;

/** Hard-delete: cannot be adapted to AgentZ without proprietary runtime. */
export const HARD_DELETE_COMMAND_SLASH = new Set([
  "search-poi",
  "route-plan",
  "set-key",
  "view-models",
  "update-task",
  "update-tasks-from-id",
  "update-single-task",
  "tm-main",
  "smart-workflow",
]);

export const HARD_DELETE_SKILL_IDS = new Set([
  "blogwatcher",
  "apple-notes",
  "apple-reminders",
]);

export function sanitizeId(raw, prefix = "") {
  let id = String(raw || "unnamed")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._:-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^[-.:]+|[-.:]+$/g, "");
  if (!id) id = "unnamed";
  if (prefix && !id.startsWith(prefix)) id = `${prefix}${id}`;
  return id.slice(0, 80);
}

export function mapTools(meta) {
  const raw =
    meta["allowed-tools"] || meta.tools || meta["allowed_tools"] || "";
  const parts = String(raw)
    .split(/[,|\s]+/)
    .map((s) => s.trim())
    .filter(Boolean);
  const out = [];
  for (const p of parts) {
    const mapped = TOOL_MAP[p] || p;
    const norm = normalizeToolToken(mapped);
    for (const t of norm) {
      if (!out.includes(t)) out.push(t);
    }
  }
  return out;
}

export function normalizeToolToken(token) {
  const raw = String(token || "").trim();
  if (!raw || /^_+$/.test(raw) || raw === "__") return [];

  const lower = raw.toLowerCase();
  if (DROP_TOKENS.has(lower)) return [];
  if (lower.startsWith("mcp__")) return [];
  if (BUILTIN_TOOLS.has(lower)) return [lower];

  if (TOOL_MAP[raw]) return [TOOL_MAP[raw]];

  for (const [re, mapped] of TOKEN_ALIASES) {
    if (re.test(raw) || re.test(lower)) return [mapped];
  }

  const underscored = lower.replace(/[^a-z0-9_]/g, "_").replace(/_+/g, "_");
  if (BUILTIN_TOOLS.has(underscored)) return [underscored];

  return [];
}

export function normalizeTools(tools) {
  const out = [];
  for (const t of tools || []) {
    for (const n of normalizeToolToken(t)) {
      if (!out.includes(n)) out.push(n);
    }
  }
  return out;
}

export function hasPlatformRefs(text) {
  return PLATFORM_BODY_RE.test(text || "");
}

export function isVendorPlugin(name) {
  return VENDOR_PLUGIN_PATTERNS.test(name || "");
}

export function isGeneralDevPlugin(name) {
  return GENERAL_DEV_PLUGINS.test(name || "");
}

export function qualityScore(entry) {
  const tools = normalizeTools(entry.tools || []);
  const validTools = tools.length > 0 && tools.every((t) => BUILTIN_TOOLS.has(t));
  const desc = String(entry.description || "").trim();
  const prompt = String(
    entry.prompt || entry.system_prompt || entry.body || "",
  ).trim();
  const plen = prompt.length;
  const promptScore = plen >= 500 && plen <= 8000 ? 15 : 5;
  const platformFree = !hasPlatformRefs(prompt) ? 20 : 0;
  const pluginBonus =
    isGeneralDevPlugin(entry.source_plugin || entry.plugin || "") ? 10 : 0;

  return (
    (validTools ? 20 : tools.length === 0 ? 10 : 0) +
    (desc ? 10 : 0) +
    promptScore +
    platformFree +
    pluginBonus
  );
}

export function rewriteAgentZText(text) {
  if (text == null) return "";
  let out = String(text);
  out = out.replace(/# Claude Command:/gi, "# AgentZ slash command:");
  out = out.replace(/CodeBuddy CLI/gi, "AgentZ");
  out = out.replace(/CodeBuddy/gi, "AgentZ");
  out = out.replace(/Claude Code/gi, "AgentZ");
  out = out.replace(/\$\{CODEBUDDY_PLUGIN_ROOT\}/g, "${AGENTZ_BUNDLE_ROOT}");
  out = out.replace(/OpenAI Agents SDK/gi, "AgentZ agent runtime");
  out = out.replace(/agents\.mcp/gi, "AgentZ MCP settings");
  out = out.replace(/MCPServerStreamableHttpParams/gi, "MCP server configuration");
  out = out.replace(/TodoWrite/g, "plan_todo tool");
  out = out.replace(/AskUserQuestion/g, "chat_ui tool");
  return out;
}

export function rewriteVendorSkillBody(slug, body, plugin) {
  let out = rewriteAgentZText(body);
  const header =
    "> **AgentZ note:** Configure external services under Settings → MCP Servers / Connectors. " +
    "Use `api_connector`, `shell`, `file_read`, and `codebase_search` instead of vendor-specific MCP tool names.\n\n";
  if (!out.includes("AgentZ note")) out = header + out;
  out = out.replace(
    /https:\/\/mcp-[a-z0-9.-]+\.[^\s)]+/gi,
    "(configure your MCP endpoint in AgentZ Settings)",
  );
  out = out.replace(/SECRET_ID|SECRET_KEY|AK\/SK/gi, "credentials in AgentZ connector settings");
  if (/wedata|tencent/i.test(slug + plugin)) {
    out = out.replace(/from agents\.mcp import[^\n]+/g, "# Use AgentZ MCP server configuration instead");
  }
  return out;
}

export function parseSkillFrontmatter(content) {
  const trimmed = content.trimStart();
  if (!trimmed.startsWith("---")) {
    return { meta: {}, body: content.trim() };
  }
  const end = trimmed.indexOf("\n---", 3);
  if (end < 0) return { meta: {}, body: content.trim() };
  const yaml = trimmed.slice(3, end).trim();
  const body = trimmed.slice(end + 4).trim();
  const meta = {};
  let currentList = null;
  for (const line of yaml.split("\n")) {
    const listMatch = line.match(/^\s*-\s+(.+)$/);
    if (listMatch && currentList) {
      currentList.push(listMatch[1].trim().replace(/^["']|["']$/g, ""));
      continue;
    }
    const kv = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (!kv) continue;
    const val = kv[2].trim();
    if (val === "") {
      currentList = [];
      meta[kv[1]] = currentList;
    } else {
      currentList = null;
      meta[kv[1]] = val.replace(/^["']|["']$/g, "");
    }
  }
  return { meta, body };
}

export function skillHasScripts(dir) {
  if (!existsSync(dir)) return false;
  const walk = (d) => {
    for (const ent of readdirSync(d, { withFileTypes: true })) {
      const p = join(d, ent.name);
      if (ent.isDirectory()) {
        if (walk(p)) return true;
      } else if (/\.(py|sh|js|ts)$/i.test(ent.name) && ent.name !== "SKILL.md") {
        return true;
      }
    }
    return false;
  };
  return walk(dir);
}

export function lintPreinstall(root) {
  const findings = [];
  const scores = {};
  const slashIndex = new Map();

  const add = (rule, severity, kind, id, message, extra = {}) => {
    findings.push({ rule, severity, kind, id, message, ...extra });
  };

  const skillsRoot = join(root, "skills");
  if (existsSync(skillsRoot)) {
    for (const slug of readdirSync(skillsRoot)) {
      const dir = join(skillsRoot, slug);
      if (!statSync(dir).isDirectory()) continue;
      const mdPath = join(dir, "SKILL.md");
      if (!existsSync(mdPath)) {
        add("SCHEMA", "error", "skill", slug, "missing SKILL.md");
        continue;
      }
      const raw = readFileSync(mdPath, "utf8");
      const { meta, body } = parseSkillFrontmatter(raw);
      if (!meta.name) add("SCHEMA", "error", "skill", slug, "missing name in frontmatter");
      const tools = normalizeTools(Array.isArray(meta.tools) ? meta.tools : mapTools(meta));
      for (const t of tools) {
        if (!BUILTIN_TOOLS.has(t)) add("TOOL_UNKNOWN", "error", "skill", slug, `unknown tool: ${t}`);
      }
      if (hasPlatformRefs(body)) add("PROMPT_PLATFORM", "warn", "skill", slug, "platform-specific references in body");
      if (isVendorPlugin(meta.source_plugin || slug))
        add("VENDOR_PLUGIN", "warn", "skill", slug, `vendor plugin: ${meta.source_plugin || slug}`);
      if (skillHasScripts(dir)) add("SKILL_SCRIPTS", "info", "skill", slug, "bundled scripts present");
      if (body.length > 32000) add("PROMPT_HUGE", "warn", "skill", slug, `body ${body.length} chars`);
      scores[`skill:${slug}`] = qualityScore({
        tools,
        description: meta.description,
        body,
        source_plugin: meta.source_plugin,
      });
    }
  }

  const agentsRoot = join(root, "agents");
  if (existsSync(agentsRoot)) {
    for (const id of readdirSync(agentsRoot)) {
      const p = join(agentsRoot, id, "agent.json");
      if (!existsSync(p)) {
        add("SCHEMA", "error", "agent", id, "missing agent.json");
        continue;
      }
      let agent;
      try {
        agent = JSON.parse(readFileSync(p, "utf8"));
      } catch (e) {
        add("SCHEMA", "error", "agent", id, `invalid JSON: ${e.message}`);
        continue;
      }
      if (!agent.id) add("SCHEMA", "error", "agent", id, "missing id");
      if (!(agent.system_prompt || "").trim())
        add("AGENT_EMPTY_PROMPT", "error", "agent", id, "empty system_prompt");
      const tools = normalizeTools(agent.tools || []);
      for (const t of agent.tools || []) {
        if (!normalizeToolToken(t).length && !BUILTIN_TOOLS.has(String(t).toLowerCase())) {
          add("TOOL_UNKNOWN", "error", "agent", id, `unknown tool: ${t}`);
        }
      }
      if (hasPlatformRefs(agent.system_prompt))
        add("PROMPT_PLATFORM", "warn", "agent", id, "platform refs in system_prompt");
      scores[`agent:${id}`] = qualityScore({
        tools,
        description: agent.description,
        system_prompt: agent.system_prompt,
        source_plugin: agent.source_plugin,
      });
    }
  }

  const commandsRoot = join(root, "commands");
  if (existsSync(commandsRoot)) {
    for (const id of readdirSync(commandsRoot)) {
      const p = join(commandsRoot, id, "command.json");
      if (!existsSync(p)) {
        add("SCHEMA", "error", "command", id, "missing command.json");
        continue;
      }
      let cmd;
      try {
        cmd = JSON.parse(readFileSync(p, "utf8"));
      } catch (e) {
        add("SCHEMA", "error", "command", id, `invalid JSON: ${e.message}`);
        continue;
      }
      const slashId = cmd.slash_id || cmd.id;
      if (!slashIndex.has(slashId)) slashIndex.set(slashId, []);
      slashIndex.get(slashId).push(id);

      for (const t of cmd.tools || []) {
        if (!normalizeToolToken(t).length && !BUILTIN_TOOLS.has(String(t).toLowerCase())) {
          add("TOOL_UNKNOWN", "error", "command", id, `unknown tool: ${t}`);
        }
      }
      if (hasPlatformRefs(cmd.prompt))
        add("PROMPT_PLATFORM", "warn", "command", id, "platform refs in prompt");
      if ((cmd.prompt || "").length > 32000)
        add("PROMPT_HUGE", "warn", "command", id, `prompt ${cmd.prompt.length} chars`);
      scores[`command:${id}`] = qualityScore({
        tools: normalizeTools(cmd.tools || []),
        description: cmd.description,
        prompt: cmd.prompt,
        source_plugin: cmd.source_plugin,
      });
    }
  }

  for (const [slashId, ids] of slashIndex.entries()) {
    if (ids.length > 1) {
      for (const id of ids) {
        add("SLASH_DUP", "error", "command", id, `duplicate slash_id: ${slashId}`, { slash_id: slashId });
      }
    }
  }

  const summary = {
    errors: findings.filter((f) => f.severity === "error").length,
    warnings: findings.filter((f) => f.severity === "warn").length,
    info: findings.filter((f) => f.severity === "info").length,
    skills: existsSync(skillsRoot) ? readdirSync(skillsRoot).length : 0,
    agents: existsSync(agentsRoot) ? readdirSync(agentsRoot).length : 0,
    commands: existsSync(commandsRoot) ? readdirSync(commandsRoot).length : 0,
  };

  return { findings, scores, summary, slashIndex };
}
