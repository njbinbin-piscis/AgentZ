#!/usr/bin/env node
/**
 * Import CodeBuddy official marketplace into AgentZ bundled/preinstall layout.
 * Usage: node scripts/import-codebuddy-marketplace.mjs [--repo PATH] [--skip-clone]
 */
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative } from "node:path";
import { execSync } from "node:child_process";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import {
  mapTools,
  normalizeTools,
  rewriteAgentZText,
  rewriteVendorSkillBody,
  sanitizeId,
  isVendorPlugin,
} from "./lib/preinstall-rules.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(ROOT, "bundled", "preinstall");
const EXCLUDE_PATH = join(ROOT, "bundled", "codebuddy", "exclude.json");
const MARKETPLACE_URL = "https://cnb.cool/codebuddy/marketplace.git";
const SKIP_PLUGINS = new Set(["all-agents", "all-commands", "all-hooks", "all-skills"]);

function parseArgs() {
  const args = process.argv.slice(2);
  let repo = join(ROOT, ".cache", "codebuddy-marketplace");
  let skipClone = false;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--repo" && args[i + 1]) repo = args[++i];
    if (args[i] === "--skip-clone") skipClone = true;
  }
  return { repo, skipClone };
}

function sha1(text) {
  return createHash("sha1").update(text).digest("hex").slice(0, 12);
}

function hasCjk(s) {
  return /[\u4e00-\u9fff]/.test(s || "");
}

function parseFrontmatter(content) {
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

function loadExclude() {
  if (!existsSync(EXCLUDE_PATH)) {
    return { skills: [], agents: [], commands: [], reasons: {} };
  }
  return JSON.parse(readFileSync(EXCLUDE_PATH, "utf8"));
}

function isExcluded(exclude, kind, id) {
  return (exclude[kind] || []).includes(id);
}

function buildSkillMd(meta, body, pluginName, slug) {
  const name = meta.name || slug;
  const desc = meta.description || "";
  const descZh = meta.description_zh || (hasCjk(desc) ? desc : "");
  const tools = normalizeTools(mapTools(meta));
  const lines = ["---", `name: ${JSON.stringify(name)}`, `description: ${JSON.stringify(desc)}`];
  if (descZh) lines.push(`description_zh: ${JSON.stringify(descZh)}`);
  lines.push(`version: ${JSON.stringify(meta.version || "1.0.0")}`);
  if (tools.length) lines.push(`tools: [${tools.join(", ")}]`);
  lines.push("source: codebuddy");
  lines.push(`source_plugin: ${JSON.stringify(pluginName)}`);
  lines.push("---", "", body);
  return lines.join("\n");
}

function copyTree(src, dest, filter) {
  if (!existsSync(src)) return;
  mkdirSync(dest, { recursive: true });
  for (const ent of readdirSync(src, { withFileTypes: true })) {
    const s = join(src, ent.name);
    const d = join(dest, ent.name);
    if (filter && !filter(s, ent)) continue;
    if (ent.isDirectory()) copyTree(s, d, filter);
    else cpSync(s, d);
  }
}

function uniqueId(base, used) {
  let id = base;
  let n = 2;
  while (used.has(id)) {
    id = `${base}-${n++}`;
  }
  used.add(id);
  return id;
}

function ensureRepo(repo) {
  if (existsSync(join(repo, ".git"))) {
    execSync("git pull --ff-only", { cwd: repo, stdio: "inherit" });
  } else {
    mkdirSync(dirname(repo), { recursive: true });
    execSync(`git clone --depth 1 ${MARKETPLACE_URL} ${repo}`, { stdio: "inherit" });
  }
}

function commandIdFromPath(relPath) {
  return relPath.replace(/\\/g, "/").replace(/\.md$/i, "").replace(/\//g, ":");
}

function rewriteSkillBody(slug, body, plugin) {
  if (isVendorPlugin(plugin) || isVendorPlugin(slug)) {
    return rewriteVendorSkillBody(slug, body, plugin);
  }
  return rewriteAgentZText(body);
}

function main() {
  const { repo, skipClone } = parseArgs();
  const exclude = loadExclude();
  if (!skipClone) {
    console.log("Cloning/updating marketplace…");
    ensureRepo(repo);
  }

  const marketplace = JSON.parse(
    readFileSync(join(repo, ".codebuddy-plugin", "marketplace.json"), "utf8"),
  );
  const pluginDesc = new Map(
    (marketplace.plugins || []).map((p) => [p.name, p.description || ""]),
  );

  rmSync(OUT, { recursive: true, force: true });
  const skillsOut = join(OUT, "skills");
  const agentsOut = join(OUT, "agents");
  const commandsOut = join(OUT, "commands");
  const templatesOut = join(OUT, "project-templates");
  mkdirSync(skillsOut, { recursive: true });
  mkdirSync(agentsOut, { recursive: true });
  mkdirSync(commandsOut, { recursive: true });
  mkdirSync(templatesOut, { recursive: true });

  const manifest = {
    version: 2,
    audit_version: 1,
    source: MARKETPLACE_URL,
    imported_at: new Date().toISOString(),
    skills: [],
    agents: [],
    commands: [],
    templates: [],
    excluded: exclude,
    notices: [],
    compatibility: { pass: 0, rewrite: 0, delete: 0 },
  };

  const usedSkill = new Set();
  const usedAgent = new Set();
  const usedCmd = new Set();
  const usedSlash = new Set();

  for (const entry of marketplace.plugins || []) {
    if (SKIP_PLUGINS.has(entry.name)) continue;
    const srcRel = typeof entry.source === "string" ? entry.source : null;
    if (!srcRel) continue;
    const pluginDir = join(repo, srcRel.replace(/^\.\//, ""));
    if (!existsSync(pluginDir)) continue;

    const licensePath = ["LICENSE", "LICENSE.txt", "LICENSE.md"]
      .map((f) => join(pluginDir, f))
      .find((p) => existsSync(p));
    if (licensePath) {
      manifest.notices.push({
        plugin: entry.name,
        license_file: relative(repo, licensePath),
        description: entry.description || "",
      });
    }

    const skillsDir = join(pluginDir, "skills");
    if (existsSync(skillsDir)) {
      for (const ent of readdirSync(skillsDir, { withFileTypes: true })) {
        if (!ent.isDirectory()) continue;
        const skillMd = join(skillsDir, ent.name, "SKILL.md");
        if (!existsSync(skillMd)) continue;
        const slug = uniqueId(sanitizeId(ent.name), usedSkill);
        if (isExcluded(exclude, "skills", slug)) {
          manifest.compatibility.delete++;
          continue;
        }
        const dest = join(skillsOut, slug);
        mkdirSync(dest, { recursive: true });
        const raw = readFileSync(skillMd, "utf8");
        const { meta, body } = parseFrontmatter(raw);
        if (!meta.description_zh && hasCjk(pluginDesc.get(entry.name) || "")) {
          meta.description_zh = pluginDesc.get(entry.name);
        }
        const rewritten = rewriteSkillBody(slug, body, entry.name);
        writeFileSync(join(dest, "SKILL.md"), buildSkillMd(meta, rewritten, entry.name, slug));
        copyTree(join(skillsDir, ent.name), dest, (p) => !p.endsWith("SKILL.md"));
        manifest.skills.push({ id: slug, plugin: entry.name, sha: sha1(raw) });
        manifest.compatibility.rewrite++;
      }
    }

    const rootSkill = join(pluginDir, "SKILL.md");
    if (existsSync(rootSkill) && basename(pluginDir) === entry.name.replace(/^.*\//, "")) {
      const slug = uniqueId(sanitizeId(entry.name), usedSkill);
      if (!isExcluded(exclude, "skills", slug) && !existsSync(join(skillsOut, slug))) {
        const dest = join(skillsOut, slug);
        mkdirSync(dest, { recursive: true });
        const raw = readFileSync(rootSkill, "utf8");
        const { meta, body } = parseFrontmatter(raw);
        const rewritten = rewriteSkillBody(slug, body, entry.name);
        writeFileSync(join(dest, "SKILL.md"), buildSkillMd(meta, rewritten, entry.name, slug));
        copyTree(pluginDir, dest, (p) => {
          const rel = relative(pluginDir, p);
          return rel !== "SKILL.md" && !rel.startsWith("commands/") && !rel.startsWith("agents/");
        });
        manifest.skills.push({ id: slug, plugin: entry.name, sha: sha1(raw) });
        manifest.compatibility.rewrite++;
      }
    }

    const agentsDir = join(pluginDir, "agents");
    if (existsSync(agentsDir)) {
      for (const f of readdirSync(agentsDir, { withFileTypes: true })) {
        if (!f.isFile() || !f.name.endsWith(".md")) continue;
        const raw = readFileSync(join(agentsDir, f.name), "utf8");
        const { meta, body } = parseFrontmatter(raw);
        if (!(body || "").trim()) continue;
        const baseId = sanitizeId(meta.name || f.name.replace(/\.md$/, ""), "cb-");
        const id = uniqueId(baseId, usedAgent);
        if (isExcluded(exclude, "agents", id)) {
          manifest.compatibility.delete++;
          continue;
        }
        const agentDir = join(agentsOut, id);
        mkdirSync(agentDir, { recursive: true });
        const desc = meta.description || entry.description || "";
        const agent = {
          id,
          name: meta.name || id,
          role: meta.role || "",
          icon: meta.icon || "🤖",
          color: meta.color || "#7c6af7",
          description: rewriteAgentZText(desc),
          description_zh:
            meta.description_zh || (hasCjk(desc) ? desc : pluginDesc.get(entry.name) || ""),
          system_prompt: rewriteAgentZText(body),
          system_prompt_zh: rewriteAgentZText(meta.system_prompt_zh || ""),
          skills: [],
          tools: normalizeTools(mapTools(meta)),
          mcp_servers: [],
          connectors: [],
          llm_provider_id: null,
          max_iterations: 0,
          task_timeout_secs: 0,
          koi_id: null,
          source: "codebuddy",
          source_plugin: entry.name,
        };
        writeFileSync(join(agentDir, "agent.json"), JSON.stringify(agent, null, 2));
        manifest.agents.push({ id, plugin: entry.name, file: f.name });
        manifest.compatibility.rewrite++;
      }
    }

    const commandsDir = join(pluginDir, "commands");
    if (existsSync(commandsDir)) {
      const walk = (dir, prefix = "") => {
        for (const ent of readdirSync(dir, { withFileTypes: true })) {
          const p = join(dir, ent.name);
          if (ent.isDirectory()) walk(p, prefix ? `${prefix}/${ent.name}` : ent.name);
          else if (ent.name.endsWith(".md")) {
            const rel = prefix ? `${prefix}/${ent.name}` : ent.name;
            const slashId = commandIdFromPath(rel);
            if (usedSlash.has(slashId)) {
              manifest.compatibility.delete++;
              continue;
            }
            const cmdId = uniqueId(sanitizeId(slashId, "cb-"), usedCmd);
            if (isExcluded(exclude, "commands", cmdId)) {
              manifest.compatibility.delete++;
              continue;
            }
            usedSlash.add(slashId);
            const raw = readFileSync(p, "utf8");
            const { meta, body } = parseFrontmatter(raw);
            const desc = meta.description || "";
            const cmdDir = join(commandsOut, cmdId);
            mkdirSync(cmdDir, { recursive: true });
            const cmd = {
              id: cmdId,
              slash_id: slashId,
              name: meta.name || cmdId,
              description: rewriteAgentZText(desc),
              description_zh: meta.description_zh || (hasCjk(desc) ? desc : ""),
              argument_hint: meta["argument-hint"] || meta.argument_hint || "",
              tools: normalizeTools(mapTools(meta)),
              prompt: rewriteAgentZText(body),
              prompt_zh: rewriteAgentZText(meta.prompt_zh || ""),
              source: "codebuddy",
              source_plugin: entry.name,
            };
            writeFileSync(join(cmdDir, "command.json"), JSON.stringify(cmd, null, 2));
            manifest.commands.push({ id: cmdId, slash_id: cmd.slash_id, plugin: entry.name });
            manifest.compatibility.rewrite++;
          }
        }
      };
      walk(commandsDir);
    }

    const rulesDir = join(pluginDir, "rules");
    const hooksFile = [join(pluginDir, "hooks", "hooks.json"), join(pluginDir, "hooks.json")].find(
      (p) => existsSync(p),
    );
    if (existsSync(rulesDir) || hooksFile) {
      const tplId = sanitizeId(`${entry.name}-template`, "tpl-");
      const tplRoot = join(templatesOut, tplId);
      mkdirSync(join(tplRoot, ".agentz", "rules"), { recursive: true });
      const tplMeta = {
        id: tplId,
        name: entry.name,
        name_zh: hasCjk(entry.description) ? entry.name : entry.name,
        description: entry.description || entry.name,
        description_zh: hasCjk(entry.description || "") ? entry.description : "",
        source_plugin: entry.name,
      };
      writeFileSync(join(tplRoot, "template.json"), JSON.stringify(tplMeta, null, 2));
      if (existsSync(rulesDir)) {
        copyTree(rulesDir, join(tplRoot, ".agentz", "rules"));
      }
      if (hooksFile) {
        let hooksRaw = readFileSync(hooksFile, "utf8");
        hooksRaw = hooksRaw.replace(/\$\{CODEBUDDY_PLUGIN_ROOT\}/g, "${AGENTZ_BUNDLE_ROOT}");
        writeFileSync(join(tplRoot, ".agentz", "hooks.json"), hooksRaw);
      }
      manifest.templates.push({ id: tplId, plugin: entry.name });
    }
  }

  manifest.compatibility.pass =
    manifest.skills.length + manifest.agents.length + manifest.commands.length;

  mkdirSync(join(ROOT, "bundled", "codebuddy"), { recursive: true });
  writeFileSync(join(ROOT, "bundled", "codebuddy", "manifest.json"), JSON.stringify(manifest, null, 2));

  const noticesPath = join(ROOT, "THIRD_PARTY_NOTICES.md");
  const noticeLines = [
    "# Third-Party Notices",
    "",
    "AgentZ preinstall resources include materials adapted from the CodeBuddy Official Plugin Marketplace.",
    "",
    `- Source: ${MARKETPLACE_URL}`,
    `- Imported: ${manifest.imported_at}`,
    `- Skills: ${manifest.skills.length}`,
    `- Agents: ${manifest.agents.length}`,
    `- Commands: ${manifest.commands.length}`,
    `- Project templates: ${manifest.templates.length}`,
    `- Excluded (exclude.json): ${exclude.skills.length + exclude.agents.length + exclude.commands.length}`,
    "",
    "Run `npm run lint:preinstall -- --strict` for compatibility audit.",
    "",
    "## Plugins",
    "",
  ];
  for (const n of manifest.notices.slice(0, 200)) {
    noticeLines.push(`- **${n.plugin}**: ${n.description.slice(0, 120)} (${n.license_file})`);
  }
  writeFileSync(noticesPath, noticeLines.join("\n"));

  console.log(
    `Done: ${manifest.skills.length} skills, ${manifest.agents.length} agents, ${manifest.commands.length} commands, ${manifest.templates.length} templates`,
  );
}

main();
