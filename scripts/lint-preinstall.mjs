#!/usr/bin/env node
/**
 * Lint bundled/preinstall for AgentZ compatibility.
 * Usage: node scripts/lint-preinstall.mjs [--strict] [--root PATH]
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { lintPreinstall } from "./lib/preinstall-rules.mjs";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const strict = args.includes("--strict");
const rootArg = args.indexOf("--root");
const preinstallRoot =
  rootArg >= 0 && args[rootArg + 1] ? args[rootArg + 1] : join(ROOT, "bundled", "preinstall");

const report = lintPreinstall(preinstallRoot);
const outDir = join(ROOT, "bundled", "codebuddy");
mkdirSync(outDir, { recursive: true });

const payload = {
  audit_version: 1,
  generated_at: new Date().toISOString(),
  root: preinstallRoot,
  summary: report.summary,
  scores: report.scores,
  findings: report.findings,
};

writeFileSync(join(outDir, "compatibility-report.json"), JSON.stringify(payload, null, 2));

const lines = [
  "# CodeBuddy Preinstall Compatibility Report",
  "",
  `Generated: ${payload.generated_at}`,
  "",
  "## Summary",
  "",
  `| Metric | Count |`,
  `|--------|------:|`,
  `| Skills | ${report.summary.skills} |`,
  `| Agents | ${report.summary.agents} |`,
  `| Commands | ${report.summary.commands} |`,
  `| Errors | ${report.summary.errors} |`,
  `| Warnings | ${report.summary.warnings} |`,
  `| Info | ${report.summary.info} |`,
  "",
  "## Errors",
  "",
];
for (const f of report.findings.filter((x) => x.severity === "error").slice(0, 200)) {
  lines.push(`- **${f.rule}** [${f.kind}] \`${f.id}\`: ${f.message}`);
}
if (report.summary.errors > 200) lines.push(`- … and ${report.summary.errors - 200} more`);
lines.push("", "## Warnings (first 50)", "");
for (const f of report.findings.filter((x) => x.severity === "warn").slice(0, 50)) {
  lines.push(`- **${f.rule}** [${f.kind}] \`${f.id}\`: ${f.message}`);
}
writeFileSync(join(outDir, "compatibility-report.md"), lines.join("\n"));

console.log(
  `lint-preinstall: errors=${report.summary.errors} warnings=${report.summary.warnings} info=${report.summary.info}`,
);
console.log(`Report: bundled/codebuddy/compatibility-report.json`);

if (strict && report.summary.errors > 0) {
  console.error("lint-preinstall: --strict failed");
  process.exit(1);
}
