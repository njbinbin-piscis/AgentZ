#!/usr/bin/env node
// Local, deterministic research-report contract. Rendering never fetches a source or publishes it.
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const trimmed = (value, max = 8192) => typeof value === "string" && value === value.trim() && value.length > 0 && value.length <= max && !/[\u0000-\u001f]/u.test(value);
const id = (value) => typeof value === "string" && /^[A-Za-z0-9._-]{1,80}$/.test(value);
function safeLocator(value) {
  if (!trimmed(value, 4096)) return false;
  if (/^https:\/\/[^\s]+$/i.test(value)) return true;
  const local = /^(?:workspace|artifact):([A-Za-z0-9._/-]+)$/i.exec(value);
  return Boolean(local && local[1].split("/").every(segment => segment && segment !== "." && segment !== ".."));
}

export function validateReport(report) {
  const errors = [];
  if (!report || typeof report !== "object" || Array.isArray(report)) return ["report must be an object"];
  if (report.schema_version !== 1) errors.push("unsupported report schema_version");
  if (!id(report.task_id)) errors.push("invalid task_id");
  for (const field of ["question", "conclusion"]) if (!trimmed(report[field])) errors.push(`missing or invalid ${field}`);
  if (!Array.isArray(report.sources) || report.sources.length === 0 || report.sources.length > 100) errors.push("sources must contain 1..100 entries");
  if (!Array.isArray(report.findings) || report.findings.length === 0 || report.findings.length > 100) errors.push("findings must contain 1..100 entries");
  if (!Array.isArray(report.limitations) || report.limitations.length === 0 || report.limitations.length > 100 || !report.limitations.every(item => trimmed(item))) errors.push("limitations must contain nonempty entries");
  const sourceIds = new Set();
  for (const source of Array.isArray(report.sources) ? report.sources : []) {
    if (!source || !id(source.id) || sourceIds.has(source.id)) { errors.push("source IDs must be unique and valid"); continue; }
    sourceIds.add(source.id);
    if (!trimmed(source.title, 1024) || !safeLocator(source.locator)) errors.push(`source ${source.id} requires title and supported locator`);
  }
  for (const finding of Array.isArray(report.findings) ? report.findings : []) {
    if (!finding || !trimmed(finding.claim) || !Array.isArray(finding.source_ids) || finding.source_ids.length === 0 || finding.source_ids.some(sourceId => !sourceIds.has(sourceId))) errors.push("every finding requires a claim and known source IDs");
  }
  return errors;
}

function prose(value) {
  return value.replace(/([\\`*_{}\[\]()#+\-.!|])/gu, "\\$1")
    .replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}
export function renderReport(report) {
  const errors = validateReport(report);
  if (errors.length) throw new Error(errors.join("; "));
  const lines = ["# Research report", "", `Task: ${prose(report.task_id)}`, "", "## Question", "", prose(report.question), "", "## Conclusion", "", prose(report.conclusion), "", "## Findings", ""];
  for (const finding of report.findings) lines.push(`- ${prose(finding.claim)} (${finding.source_ids.map(id => `[${id}]`).join(", ")})`);
  lines.push("", "## Sources", "");
  for (const source of report.sources) lines.push(`- [${source.id}] ${prose(source.title)} — ${prose(source.locator)}`);
  lines.push("", "## Limitations", "");
  for (const limitation of report.limitations) lines.push(`- ${prose(limitation)}`);
  return `${lines.join("\n")}\n`;
}

function main(args) {
  if (args.length < 2 || !["validate", "render"].includes(args[0])) { console.error("Usage: node scripts/upgrade-report.mjs validate|render report.json [--out report.md]"); return 2; }
  let report;
  try { report = JSON.parse(readFileSync(args[1], "utf8")); } catch (error) { console.error(error.message); return 1; }
  const errors = validateReport(report);
  if (errors.length) { console.error(errors.join("\n")); return 1; }
  if (args[0] === "validate") { console.log("report valid"); return 0; }
  const outIndex = args.indexOf("--out");
  if (outIndex >= 0 && !args[outIndex + 1]) { console.error("missing --out path"); return 2; }
  const content = renderReport(report);
  if (outIndex >= 0) writeFileSync(args[outIndex + 1], content); else process.stdout.write(content);
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) process.exitCode = main(process.argv.slice(2));
