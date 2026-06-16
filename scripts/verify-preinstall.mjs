#!/usr/bin/env node
/**
 * CI check: bundled CodeBuddy preinstall tree exists and meets minimum counts.
 * Usage: node scripts/verify-preinstall.mjs [--strict]
 */
import { existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { lintPreinstall } from "./lib/preinstall-rules.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const root = join(ROOT, "bundled", "preinstall");
const manifestPath = join(ROOT, "bundled", "codebuddy", "manifest.json");
const strict = process.argv.includes("--strict");

function countDirs(p) {
  if (!existsSync(p)) return 0;
  return readdirSync(p).filter((name) => statSync(join(p, name)).isDirectory()).length;
}

const skills = countDirs(join(root, "skills"));
const agents = countDirs(join(root, "agents"));
const commands = countDirs(join(root, "commands"));
const templates = countDirs(join(root, "project-templates"));

const mins = { skills: 240, agents: 220, commands: 300, templates: 5 };
let ok = true;

for (const [key, min] of Object.entries(mins)) {
  const n = { skills, agents, commands, templates }[key];
  if (n < min) {
    console.error(`verify-preinstall: ${key} count ${n} < minimum ${min}`);
    ok = false;
  }
}

if (!existsSync(manifestPath)) {
  console.error(`verify-preinstall: missing ${manifestPath}`);
  ok = false;
}

if (strict) {
  const report = lintPreinstall(root);
  if (report.summary.errors > 0) {
    console.error(`verify-preinstall: strict lint errors=${report.summary.errors}`);
    ok = false;
  }
}

if (!ok) process.exit(1);
console.log(
  `verify-preinstall ok: skills=${skills} agents=${agents} commands=${commands} templates=${templates}${strict ? " (strict)" : ""}`,
);
