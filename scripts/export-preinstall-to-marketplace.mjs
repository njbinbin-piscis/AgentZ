#!/usr/bin/env node
/**
 * Export AgentZ bundled/preinstall (post-compat-audit) to theAgentOS official cloud catalog.
 * Usage: node scripts/export-preinstall-to-marketplace.mjs [--out PATH]
 */
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const PREINSTALL = join(ROOT, "bundled", "preinstall");
const EXCLUDE_PATH = join(ROOT, "bundled", "codebuddy", "exclude.json");
const COMPAT_REPORT_PATH = join(ROOT, "bundled", "codebuddy", "compatibility-report.json");
const DEFAULT_OUT = join(
  ROOT,
  "..",
  "theAgentOS",
  "marketplace",
  "seed",
  "official-catalog",
);

const PUBLISHER = "official";
const SEED_VERSION = 2;
const ASSET_VERSION = "1.0.0";
const PLATFORM_COMPAT = { surfaces: ["web", "desktop"] };
const TAGS = ["official", "codebuddy", "openpisci", "agentz"];

function parseArgs() {
  let out = DEFAULT_OUT;
  const args = process.argv.slice(2);
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--out" && args[i + 1]) out = args[++i];
  }
  return { out };
}

function loadExclude() {
  if (!existsSync(EXCLUDE_PATH)) {
    return { skills: [], agents: [], commands: [] };
  }
  return JSON.parse(readFileSync(EXCLUDE_PATH, "utf8"));
}

function loadCompatReport() {
  if (!existsSync(COMPAT_REPORT_PATH)) return null;
  try {
    return JSON.parse(readFileSync(COMPAT_REPORT_PATH, "utf8"));
  } catch {
    return null;
  }
}

function parseSkillFrontmatter(content) {
  const trimmed = content.trimStart();
  if (!trimmed.startsWith("---")) return { name: "", description: "", description_zh: "" };
  const end = trimmed.indexOf("\n---", 3);
  if (end < 0) return { name: "", description: "", description_zh: "" };
  const yaml = trimmed.slice(3, end).trim();
  const meta = {};
  for (const line of yaml.split("\n")) {
    const m = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (!m) continue;
    meta[m[1]] = m[2].trim().replace(/^["']|["']$/g, "");
  }
  return {
    name: meta.name || "",
    description: meta.description || "",
    description_zh: meta.description_zh || "",
  };
}

function safeFilename(assetId) {
  return assetId.replace(/\//g, "__").replace("@", "_at_") + ".json";
}

function slugify(raw) {
  const s = String(raw || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  if (!s || !/^[a-z0-9][a-z0-9-]*$/.test(s)) {
    throw new Error(`invalid slug: ${raw}`);
  }
  return s;
}

function assetId(kind, slug) {
  return `${PUBLISHER}/${kind}/${slug}@${ASSET_VERSION}`;
}

function basePayload(kind, slug, name, description) {
  return {
    spec_version: 1,
    id: assetId(kind, slug),
    kind,
    name: name || slug,
    description: description || "",
    publisher: PUBLISHER,
    category: "official",
    tags: TAGS,
    featured: true,
    is_featured: true,
    platform_compat: PLATFORM_COMPAT,
  };
}

function writeAsset(outDir, payload) {
  const path = join(outDir, safeFilename(payload.id));
  const record = {
    ...payload,
    uploaded_by: "official",
    payload,
  };
  writeFileSync(path, JSON.stringify(record, null, 2), "utf8");
  return path;
}

function exportSkills(outDir, exclude) {
  const src = join(PREINSTALL, "skills");
  let n = 0;
  for (const ent of readdirSync(src, { withFileTypes: true })) {
    if (!ent.isDirectory()) continue;
    const slug = ent.name;
    if (exclude.skills?.includes(slug)) continue;
    const skillMd = join(src, slug, "SKILL.md");
    if (!existsSync(skillMd)) continue;
    const content = readFileSync(skillMd, "utf8");
    const fm = parseSkillFrontmatter(content);
    const desc = fm.description_zh?.trim() || fm.description?.trim() || "";
    const payload = basePayload("skill", slugify(slug), fm.name || slug, desc);
    payload.skill_md = content;
    payload.cloud_only = false;
    writeAsset(outDir, payload);
    n++;
  }
  return n;
}

function exportAgents(outDir, exclude) {
  const src = join(PREINSTALL, "agents");
  let n = 0;
  for (const ent of readdirSync(src, { withFileTypes: true })) {
    if (!ent.isDirectory()) continue;
    const dirName = ent.name;
    const agentJson = join(src, dirName, "agent.json");
    if (!existsSync(agentJson)) continue;
    const manifest = JSON.parse(readFileSync(agentJson, "utf8"));
    const id = manifest.id || dirName;
    if (exclude.agents?.includes(id)) continue;
    const slug = slugify(id);
    const desc =
      manifest.description_zh?.trim() || manifest.description?.trim() || "";
    const payload = basePayload("expert", slug, manifest.name || id, desc);
    payload.icon = manifest.icon || "🤖";
    payload.color = manifest.color || "#7c5cff";
    const prompt = manifest.system_prompt?.trim() || desc || manifest.name || id;
    payload.system_prompt = prompt;
    if (manifest.system_prompt_zh?.trim()) {
      payload.system_prompt_zh = manifest.system_prompt_zh;
    }
    if (manifest.description_zh?.trim()) {
      payload.description_zh = manifest.description_zh;
    }
    if (manifest.tools?.length) payload.allowed_tools = manifest.tools;
    if (manifest.skills?.length) payload.allowed_skills = manifest.skills;
    payload.agent_manifest = manifest;
    payload.cloud_only = false;
    writeAsset(outDir, payload);
    n++;
  }
  return n;
}

function exportCommands(outDir, exclude) {
  const src = join(PREINSTALL, "commands");
  let n = 0;
  for (const ent of readdirSync(src, { withFileTypes: true })) {
    if (!ent.isDirectory()) continue;
    const dirName = ent.name;
    const cmdJson = join(src, dirName, "command.json");
    if (!existsSync(cmdJson)) continue;
    const manifest = JSON.parse(readFileSync(cmdJson, "utf8"));
    const id = manifest.id || dirName;
    if (exclude.commands?.includes(id)) continue;
    const slug = slugify(id);
    const desc =
      manifest.description_zh?.trim() || manifest.description?.trim() || "";
    const payload = basePayload("tool", slug, manifest.name || id, desc);
    payload.cloud_only = true;
    payload.agentz_kind = "slash_command";
    payload.slash_command = manifest;
    payload.tags = [...TAGS, "slash-command", "agentz-only"];
    payload.platform_compat = { surfaces: ["desktop"] };
    writeAsset(outDir, payload);
    n++;
  }
  return n;
}

function main() {
  const { out } = parseArgs();
  const assetsDir = join(out, "assets");
  if (existsSync(assetsDir)) {
    rmSync(assetsDir, { recursive: true, force: true });
  }
  mkdirSync(assetsDir, { recursive: true });
  const exclude = loadExclude();
  const compat = loadCompatReport();

  const skills = exportSkills(assetsDir, exclude);
  const agents = exportAgents(assetsDir, exclude);
  const commands = exportCommands(assetsDir, exclude);

  const manifest = {
    version: SEED_VERSION,
    catalog_id: "official-catalog",
    publisher: PUBLISHER,
    source: "AgentZ bundled/preinstall (post-compat-audit)",
    generated_at: new Date().toISOString(),
    counts: { skills, agents, commands },
    total: skills + agents + commands,
    compatibility: {
      audited: Boolean(compat),
      audit_version: compat?.audit_version ?? null,
      lint_errors: compat?.summary?.errors ?? null,
      lint_warnings: compat?.summary?.warnings ?? null,
      excluded: {
        skills: exclude.skills?.length ?? 0,
        agents: exclude.agents?.length ?? 0,
        commands: exclude.commands?.length ?? 0,
      },
      export_rule:
        "仅导出 bundled/preinstall 中未列入 exclude.json 的 PASS/REWRITE 条目；DELETE 与重复 slash_id 已剔除",
      clients: {
        openpisci_desktop: { skills: true, experts: true, teams: false },
        agentz_desktop: { skills: true, experts: true, slash_commands: true },
        theagentos_web: { skills: true, experts: true, slash_commands: false },
      },
    },
  };
  writeFileSync(join(out, "seed-manifest.json"), JSON.stringify(manifest, null, 2), "utf8");

  console.log(
    `export-official-catalog ok: skills=${skills} agents=${agents} commands=${commands} → ${assetsDir}`,
  );
}

main();
