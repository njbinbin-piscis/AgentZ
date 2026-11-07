#!/usr/bin/env node
// Audit the bundled preinstall agents / skills / commands for third-party branding and for
// descriptions that render badly in the AgentZ lists. Read-only; prints a summary and,
// with --list, every offending item. Exit code 1 when anything is found (usable as a gate).
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "bundled", "preinstall");
const list = process.argv.includes("--list");
const only = process.argv.find((a) => a.startsWith("--kind="))?.slice(7);

const BRAND = /code\s*-?\s*buddy|\bcb-|腾讯|tencent|wedata|\bcnb\.cool|\.codebuddy|CODEBUDDY/i;
const MARKUP = /```|<\/?[a-z][a-z0-9]*(\s[^>]*)?>|^\s{0,3}#{1,6}\s|\*\*[^*]+\*\*|^\s*[-*]\s|\n/m;
const PRODUCT_SKILLS = new Set(["cloudbase", "tencent-docs", "tmap-jsapi-gl"]);
// Artifacts of the old blind word-swap and of foreign tool/plugin systems.
const LEGACY = /AgentZ Code|AgentZ\.md|\.AgentZ\b|PLUGIN_ROOT|plugin_root|AgentZ note|\bmcp__|\/project:|CLAUDE\.md|\.claude\/|superpowers:|\bClaude Code\b/;
const CJK = /[\u3400-\u9fff]/;
const LATIN_WORD = /[A-Za-z]{3,}/;

const problems = { brand: [], markup: [], long: [], langMix: [], zhMissing: [], zhPlaceholder: [] };

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8").replace(/^\uFEFF/, ""));
}
function frontmatter(file) {
  const text = readFileSync(file, "utf8").replace(/^\uFEFF/, "");
  const m = /^---\r?\n([\s\S]*?)\r?\n---/.exec(text);
  const fm = {};
  if (!m) return { fm, body: text };
  const lines = m[1].split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const kv = /^([A-Za-z_][\w-]*):\s*(.*)$/.exec(lines[i]);
    if (!kv) continue;
    let v = kv[2].trim();
    if (/^[>|][+-]?$/.test(v)) {
      const buf = [];
      while (i + 1 < lines.length && /^\s+/.test(lines[i + 1])) buf.push(lines[++i].trim());
      v = buf.join(" ");
    }
    if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
      try { v = v.startsWith('"') ? JSON.parse(v) : v.slice(1, -1); } catch { v = v.slice(1, -1); }
    }
    fm[kv[1]] = v;
  }
  return { fm, body: text.slice(m[0].length) };
}

function checkDescriptions(kind, id, description, description_zh) {
  const d = String(description ?? "");
  const z = String(description_zh ?? "");
  if (MARKUP.test(d)) problems.markup.push([kind, id, "description"]);
  if (MARKUP.test(z)) problems.markup.push([kind, id, "description_zh"]);
  if (d.length > 200) problems.long.push([kind, id, `description ${d.length}`]);
  if (z.length > 80) problems.long.push([kind, id, `description_zh ${z.length}`]);
  if (CJK.test(d)) problems.langMix.push([kind, id, "description has CJK (should be English)"]);
  if (z && !CJK.test(z)) problems.langMix.push([kind, id, "description_zh has no CJK (should be Chinese)"]);
  if (!z.trim()) problems.zhMissing.push([kind, id, ""]);
  if (z && d && z.trim() === d.trim()) problems.zhPlaceholder.push([kind, id, "zh == en"]);
  if (/domain-specific expert agents|\?{3,}/i.test(z)) problems.zhPlaceholder.push([kind, id, "placeholder/garbled zh"]);
}

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    const p = path.join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

function brandHits(kind, id, dir) {
  for (const file of walk(dir)) {
    if (statSync(file).size > 2 * 1024 * 1024) continue;
    const buf = readFileSync(file);
    if (buf.includes(0)) continue;
    const text = buf.toString("utf8");
    let m = BRAND.exec(text);
    // Real third-party products that legitimately name their vendor.
    if (m && PRODUCT_SKILLS.has(id) && /tencent|腾讯/i.test(m[0])) m = null;
    if (!m) m = LEGACY.exec(text);
    if (m) problems.brand.push([kind, id, `${path.relative(dir, file)}: ${m[0]}`]);
  }
}

if (!only || only === "skills") {
  for (const id of readdirSync(path.join(root, "skills"))) {
    const dir = path.join(root, "skills", id);
    const file = path.join(dir, "SKILL.md");
    if (!existsSync(file)) { problems.brand.push(["skill", id, "missing SKILL.md"]); continue; }
    const { fm } = frontmatter(file);
    checkDescriptions("skill", id, fm.description, fm.description_zh);
    brandHits("skill", id, dir);
  }
}
if (!only || only === "agents") {
  for (const id of readdirSync(path.join(root, "agents"))) {
    const dir = path.join(root, "agents", id);
    const file = path.join(dir, "agent.json");
    if (!existsSync(file)) continue;
    const j = readJson(file);
    checkDescriptions("agent", id, j.description, j.description_zh);
    brandHits("agent", id, dir);
  }
}
if (!only || only === "commands") {
  for (const id of readdirSync(path.join(root, "commands"))) {
    const dir = path.join(root, "commands", id);
    const file = path.join(dir, "command.json");
    if (!existsSync(file)) continue;
    const j = readJson(file);
    checkDescriptions("command", id, j.description, j.description_zh);
    brandHits("command", id, dir);
  }
}

let total = 0;
for (const [name, items] of Object.entries(problems)) {
  total += items.length;
  const byKind = items.reduce((a, [k]) => ((a[k] = (a[k] ?? 0) + 1), a), {});
  console.log(`${name.padEnd(14)} ${String(items.length).padStart(5)}  ${JSON.stringify(byKind)}`);
  if (list) for (const [k, id, detail] of items) console.log(`   ${k} ${id} ${detail}`);
}
process.exit(total ? 1 : 0);
