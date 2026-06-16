#!/usr/bin/env node
/**
 * Remediate bundled/preinstall for AgentZ compatibility:
 * - normalize tools
 * - rewrite prompts
 * - dedupe slash_id by quality score
 * - apply exclude.json deletions
 *
 * Usage: node scripts/remediate-preinstall.mjs [--write-exclude]
 */
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  BUILTIN_TOOLS,
  HARD_DELETE_COMMAND_SLASH,
  HARD_DELETE_SKILL_IDS,
  lintPreinstall,
  mapTools,
  normalizeTools,
  qualityScore,
  rewriteAgentZText,
  rewriteVendorSkillBody,
  parseSkillFrontmatter,
  isVendorPlugin,
} from "./lib/preinstall-rules.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const PREINSTALL = join(ROOT, "bundled", "preinstall");
const EXCLUDE_PATH = join(ROOT, "bundled", "codebuddy", "exclude.json");
const writeExclude = process.argv.includes("--write-exclude");

function loadExclude() {
  if (!existsSync(EXCLUDE_PATH)) {
    return { skills: [], agents: [], commands: [], reasons: {} };
  }
  return JSON.parse(readFileSync(EXCLUDE_PATH, "utf8"));
}

function saveExclude(exclude) {
  mkdirSync(dirname(EXCLUDE_PATH), { recursive: true });
  writeFileSync(EXCLUDE_PATH, JSON.stringify(exclude, null, 2));
}

function removeDir(p) {
  if (existsSync(p)) rmSync(p, { recursive: true, force: true });
}

function rebuildSkillMd(meta, body) {
  const tools = normalizeTools(Array.isArray(meta.tools) ? meta.tools : mapTools(meta));
  const lines = [
    "---",
    `name: ${JSON.stringify(meta.name || "unnamed")}`,
    `description: ${JSON.stringify(meta.description || "")}`,
  ];
  if (meta.description_zh) lines.push(`description_zh: ${JSON.stringify(meta.description_zh)}`);
  lines.push(`version: ${JSON.stringify(meta.version || "1.0.0")}`);
  if (tools.length) lines.push(`tools: [${tools.join(", ")}]`);
  if (meta.source) lines.push(`source: ${meta.source}`);
  if (meta.source_plugin) lines.push(`source_plugin: ${JSON.stringify(meta.source_plugin)}`);
  lines.push("---", "", body);
  return lines.join("\n");
}

function remediateSkills(exclude) {
  const root = join(PREINSTALL, "skills");
  if (!existsSync(root)) return;
  for (const slug of [...readdirSync(root)]) {
    if (exclude.skills.includes(slug)) {
      removeDir(join(root, slug));
      continue;
    }
    if (HARD_DELETE_SKILL_IDS.has(slug)) {
      exclude.skills.push(slug);
      exclude.reasons[slug] = "platform_hard_dependency";
      removeDir(join(root, slug));
      continue;
    }
    const dir = join(root, slug);
    const mdPath = join(dir, "SKILL.md");
    if (!existsSync(mdPath)) continue;
    const raw = readFileSync(mdPath, "utf8");
    let { meta, body } = parseSkillFrontmatter(raw);
    const plugin = meta.source_plugin || slug;

    if (isVendorPlugin(plugin) || isVendorPlugin(slug)) {
      body = rewriteVendorSkillBody(slug, body, plugin);
    } else {
      body = rewriteAgentZText(body);
      if (/(python3 .*scripts\/|\.py template)/i.test(body)) {
        body =
          "> **AgentZ note:** Bundled scripts are optional reference only. Prefer `shell`, `file_read`, and `file_write`.\n\n" +
          body;
      }
    }

    meta.tools = normalizeTools(Array.isArray(meta.tools) ? meta.tools : mapTools(meta));
    writeFileSync(mdPath, rebuildSkillMd(meta, body));
  }
}

function remediateAgents(exclude) {
  const root = join(PREINSTALL, "agents");
  if (!existsSync(root)) return;
  const personaIndex = new Map();

  for (const id of readdirSync(root)) {
    const p = join(root, id, "agent.json");
    if (!existsSync(p)) continue;
    const agent = JSON.parse(readFileSync(p, "utf8"));

    if (!(agent.system_prompt || "").trim()) {
      exclude.agents.push(id);
      exclude.reasons[id] = "empty_system_prompt";
      removeDir(join(root, id));
      continue;
    }

    agent.tools = normalizeTools(agent.tools || []);
    agent.system_prompt = rewriteAgentZText(agent.system_prompt);
    if (agent.system_prompt_zh) agent.system_prompt_zh = rewriteAgentZText(agent.system_prompt_zh);
    agent.description = rewriteAgentZText(agent.description || "");
    writeFileSync(p, JSON.stringify(agent, null, 2));

    const key = (agent.name || id).toLowerCase().replace(/\s+/g, "-");
    if (!personaIndex.has(key)) personaIndex.set(key, []);
    personaIndex.get(key).push({ id, score: qualityScore(agent) });
  }

  for (const [, list] of personaIndex.entries()) {
    if (list.length <= 1) continue;
    list.sort((a, b) => b.score - a.score);
    for (const loser of list.slice(1)) {
      if (!exclude.agents.includes(loser.id)) {
        exclude.agents.push(loser.id);
        exclude.reasons[loser.id] = "duplicate_persona";
        removeDir(join(root, loser.id));
      }
    }
  }
}

function remediateCommands(exclude) {
  const root = join(PREINSTALL, "commands");
  if (!existsSync(root)) return;

  const slashGroups = new Map();
  for (const id of readdirSync(root)) {
    const p = join(root, id, "command.json");
    if (!existsSync(p)) continue;
    const cmd = JSON.parse(readFileSync(p, "utf8"));
    const slashId = cmd.slash_id || cmd.id;

    if (HARD_DELETE_COMMAND_SLASH.has(slashId)) {
      exclude.commands.push(id);
      exclude.reasons[id] = "hard_delete_platform_command";
      removeDir(join(root, id));
      continue;
    }

    cmd.tools = normalizeTools(cmd.tools || []);
    cmd.prompt = rewriteAgentZText(cmd.prompt || "");
    if (cmd.prompt_zh) cmd.prompt_zh = rewriteAgentZText(cmd.prompt_zh);
    writeFileSync(p, JSON.stringify(cmd, null, 2));

    if (!slashGroups.has(slashId)) slashGroups.set(slashId, []);
    slashGroups.get(slashId).push({ id, cmd, score: qualityScore(cmd) });
  }

  for (const [slashId, list] of slashGroups.entries()) {
    if (list.length <= 1) continue;
    list.sort((a, b) => b.score - a.score);
    for (const loser of list.slice(1)) {
      if (!exclude.commands.includes(loser.id)) {
        exclude.commands.push(loser.id);
        exclude.reasons[loser.id] = `slash_id_duplicate:${slashId}`;
        removeDir(join(root, loser.id));
      }
    }
  }
}

function applyExcludeList(exclude) {
  for (const slug of exclude.skills) removeDir(join(PREINSTALL, "skills", slug));
  for (const id of exclude.agents) removeDir(join(PREINSTALL, "agents", id));
  for (const id of exclude.commands) removeDir(join(PREINSTALL, "commands", id));
}

function syncManifest(exclude) {
  const manifestPath = join(ROOT, "bundled", "codebuddy", "manifest.json");
  if (!existsSync(manifestPath)) return;
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const skills = countDirs(join(PREINSTALL, "skills"));
  const agents = countDirs(join(PREINSTALL, "agents"));
  const commands = countDirs(join(PREINSTALL, "commands"));
  manifest.excluded = exclude;
  manifest.compatibility = {
    pass: skills + agents + commands,
    rewrite: manifest.compatibility?.rewrite ?? 0,
    delete: exclude.skills.length + exclude.agents.length + exclude.commands.length,
  };
  manifest.audit_version = 1;
  manifest.counts = { skills, agents, commands };
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
}

function countDirs(p) {
  if (!existsSync(p)) return 0;
  return readdirSync(p).filter((name) => statSync(join(p, name)).isDirectory()).length;
}

function writeBatchReports(exclude) {
  const reportsDir = join(ROOT, "bundled", "codebuddy", "reports");
  mkdirSync(reportsDir, { recursive: true });
  const byPlugin = new Map();

  const note = (plugin, kind, id, action) => {
    if (!byPlugin.has(plugin)) byPlugin.set(plugin, []);
    byPlugin.get(plugin).push({ kind, id, action });
  };

  for (const id of exclude.skills) {
    note("excluded", "skill", id, exclude.reasons[id] || "delete");
  }
  for (const id of exclude.agents) {
    note("excluded", "agent", id, exclude.reasons[id] || "delete");
  }
  for (const id of exclude.commands) {
    note("excluded", "command", id, exclude.reasons[id] || "delete");
  }

  for (const [plugin, items] of byPlugin.entries()) {
    const lines = [`# ${plugin}`, "", "| Kind | ID | Action |", "|------|-----|--------|"];
    for (const it of items) {
      lines.push(`| ${it.kind} | ${it.id} | ${it.action} |`);
    }
    writeFileSync(join(reportsDir, `${plugin.replace(/[^a-z0-9.-]+/gi, "-")}.md`), lines.join("\n"));
  }

  const summary = [
    "# Preinstall audit summary",
    "",
    `- Skills in bundle: ${countDirs(join(PREINSTALL, "skills"))}`,
    `- Agents in bundle: ${countDirs(join(PREINSTALL, "agents"))}`,
    `- Commands in bundle: ${countDirs(join(PREINSTALL, "commands"))}`,
    `- Excluded skills: ${exclude.skills.length}`,
    `- Excluded agents: ${exclude.agents.length}`,
    `- Excluded commands: ${exclude.commands.length}`,
  ];
  writeFileSync(join(reportsDir, "SUMMARY.md"), summary.join("\n"));
}

function main() {
  const exclude = loadExclude();
  exclude.skills = [...new Set(exclude.skills)];
  exclude.agents = [...new Set(exclude.agents)];
  exclude.commands = [...new Set(exclude.commands)];
  exclude.reasons = exclude.reasons || {};

  applyExcludeList(exclude);
  remediateSkills(exclude);
  remediateAgents(exclude);
  remediateCommands(exclude);

  if (writeExclude) saveExclude(exclude);

  const report = lintPreinstall(PREINSTALL);
  console.log(
    `remediate-preinstall done: skills=${report.summary.skills} agents=${report.summary.agents} commands=${report.summary.commands}`,
  );
  console.log(
    `lint after remediate: errors=${report.summary.errors} warnings=${report.summary.warnings}`,
  );
  console.log(
    `exclude totals: skills=${exclude.skills.length} agents=${exclude.agents.length} commands=${exclude.commands.length}`,
  );

  saveExclude(exclude);
  syncManifest(exclude);
  writeBatchReports(exclude);
}

main();
