#!/usr/bin/env node
// Trend table for upgrade evaluation summaries (`*-summary-*.json`).
//
// Refuses to mix runs: every summary must share one spec_sha256 and one
// fixture_source_sha256, otherwise the success ratios are not comparable.
// Rows are grouped by model, endpoint, thinking and measurement scope.
//
// Usage: node scripts/upgrade-eval-trend.mjs [summary.json ...] [--out trend.md] [--gate gate.json]
// With no files, reads docs/upgrade/evidence/*-summary-*.json.

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const REQUIRED_STRING_FIELDS = ["spec_sha256", "fixture_source_sha256", "model", "endpoint"];
const REQUIRED_NUMBER_FIELDS = ["planned_samples", "passed_samples", "failed_samples"];

/** Validate one parsed summary. Returns a list of problems (empty when valid). */
export function validateSummary(summary, source = "summary") {
  const problems = [];
  if (!summary || typeof summary !== "object") return [`${source}: not a JSON object`];
  for (const field of REQUIRED_STRING_FIELDS) {
    if (typeof summary[field] !== "string" || summary[field].trim() === "") {
      problems.push(`${source}: missing ${field}`);
    }
  }
  for (const field of ["spec_sha256", "fixture_source_sha256"]) {
    if (typeof summary[field] === "string" && summary[field].trim() !== "" && !/^[0-9a-f]{64}$/i.test(summary[field])) {
      problems.push(`${source}: invalid ${field}`);
    }
  }
  for (const field of REQUIRED_NUMBER_FIELDS) {
    if (!Number.isSafeInteger(summary[field]) || summary[field] < 0) {
      problems.push(`${source}: ${field} must be a non-negative integer`);
    }
  }
  const counts = ["attempted_samples", "unattempted_samples", "unknown_usage_samples", "manual_interventions_in_samples", "host_fault_samples", "planned_unique_tasks", "repeats"];
  for (const field of counts) if (field in summary && (!Number.isSafeInteger(summary[field]) || summary[field] < 0)) {
    problems.push(`${source}: ${field} must be a non-negative integer`);
  }
  if (REQUIRED_NUMBER_FIELDS.every(field => Number.isSafeInteger(summary[field]) && summary[field] >= 0)) {
    const attempted = summary.attempted_samples ?? summary.passed_samples + summary.failed_samples;
    if (Number.isSafeInteger(summary.planned_unique_tasks) && Number.isSafeInteger(summary.repeats)
      && summary.planned_unique_tasks * summary.repeats !== summary.planned_samples) problems.push(`${source}: task count times repeats must equal planned`);
    if (summary.passed_samples + summary.failed_samples !== attempted) problems.push(`${source}: passed + failed must equal attempted`);
    if (attempted > summary.planned_samples) problems.push(`${source}: attempted exceeds planned`);
    if ("unattempted_samples" in summary && attempted + summary.unattempted_samples !== summary.planned_samples) problems.push(`${source}: attempted + unattempted must equal planned`);
    if (summary.all_planned_samples_passed === true && (summary.passed_samples !== summary.planned_samples || summary.failed_samples !== 0)) problems.push(`${source}: all_planned_samples_passed contradicts counts`);
    if ("observed_success_ratio" in summary && (!Number.isFinite(summary.observed_success_ratio)
      || Math.abs(summary.observed_success_ratio - (summary.planned_samples > 0 ? summary.passed_samples / summary.planned_samples : 0)) > 1e-10)) problems.push(`${source}: observed success ratio contradicts counts`);
    if (Number.isSafeInteger(summary.unknown_usage_samples) && summary.unknown_usage_samples > attempted) problems.push(`${source}: unknown usage exceeds attempted`);
    if (Number.isSafeInteger(summary.manual_interventions_in_samples) && summary.manual_interventions_in_samples > attempted) problems.push(`${source}: manual interventions exceed attempted`);
    if (Number.isSafeInteger(summary.host_fault_samples) && summary.host_fault_samples > attempted) problems.push(`${source}: host faults exceed attempted`);
    if (summary.statuses && typeof summary.statuses === "object" && !Array.isArray(summary.statuses)) {
      const counts = Object.values(summary.statuses);
      if (!counts.every(value => Number.isSafeInteger(value) && value >= 0) || counts.reduce((sum, value) => sum + value, 0) !== attempted) problems.push(`${source}: status counts must equal attempted`);
    }
    if (summary.by_task && typeof summary.by_task === "object" && !Array.isArray(summary.by_task)) {
      const tasks = Object.values(summary.by_task);
      if (!tasks.every(task => task && Number.isSafeInteger(task.attempted) && Number.isSafeInteger(task.passed) && Number.isSafeInteger(task.failed) && task.attempted === task.passed + task.failed)
        || tasks.reduce((sum, task) => sum + task.attempted, 0) !== attempted
        || tasks.reduce((sum, task) => sum + task.passed, 0) !== summary.passed_samples
        || tasks.reduce((sum, task) => sum + task.failed, 0) !== summary.failed_samples) problems.push(`${source}: by_task counts contradict summary`);
    }
  }
  if ("total_billed_cost_usd" in summary && summary.total_billed_cost_usd !== null && (!Number.isFinite(summary.total_billed_cost_usd) || summary.total_billed_cost_usd < 0)) problems.push(`${source}: invalid billed cost`);
  if ("recorded_usage_peak_rate_estimate_usd" in summary && summary.recorded_usage_peak_rate_estimate_usd !== null && (!Number.isFinite(summary.recorded_usage_peak_rate_estimate_usd) || summary.recorded_usage_peak_rate_estimate_usd < 0)) problems.push(`${source}: invalid cost estimate`);
  if (summary.billing_verified === true && (summary.total_billed_cost_usd === null || summary.total_billed_cost_usd === undefined)) problems.push(`${source}: verified billing requires a known billed cost`);
  if ("scope" in summary && (typeof summary.scope !== "string" || !summary.scope.trim())) problems.push(`${source}: invalid measurement scope`);
  if ("thinking" in summary && (typeof summary.thinking !== "string" || !summary.thinking.trim())) problems.push(`${source}: invalid thinking mode`);
  return problems;
}

/**
 * Reject a set of summaries that do not share one spec and one fixture hash.
 * Returns { ok, errors } with the distinct hashes listed on failure.
 */
export function checkConsistency(entries) {
  const errors = [];
  const specs = new Map();
  const fixtures = new Map();
  for (const { source, summary } of entries) {
    const spec = summary.spec_sha256;
    const fixture = summary.fixture_source_sha256;
    if (!specs.has(spec)) specs.set(spec, []);
    specs.get(spec).push(source);
    if (!fixtures.has(fixture)) fixtures.set(fixture, []);
    fixtures.get(fixture).push(source);
  }
  if (specs.size > 1) {
    errors.push(describeMix("spec_sha256", specs));
  }
  if (fixtures.size > 1) {
    errors.push(describeMix("fixture_source_sha256", fixtures));
  }
  return { ok: errors.length === 0, errors };
}

function describeMix(field, byHash) {
  const lines = [...byHash.entries()].map(([hash, sources]) => `  ${hash}: ${sources.join(", ")}`);
  return `mixed ${field} across summaries; trends are only comparable within one value:\n${lines.join("\n")}`;
}

/**
 * Group summaries by model and endpoint, keeping run order.
 * Each row carries the aggregate ratio computed from sample counts.
 */
export function buildTrend(entries) {
  const groups = new Map();
  for (const { source, summary } of entries) {
    const scope = summary.scope ?? "unspecified";
    const thinking = summary.thinking ?? "unspecified";
    const key = `${summary.model}\u0000${summary.endpoint}\u0000${thinking}\u0000${scope}`;
    if (!groups.has(key)) {
      groups.set(key, { model: summary.model, endpoint: summary.endpoint, thinking, scope,
        specSha256: summary.spec_sha256, fixtureSha256: summary.fixture_source_sha256, runs: [] });
    }
    const planned = summary.planned_samples;
    const passed = summary.passed_samples;
    groups.get(key).runs.push({
      source,
      planned,
      passed,
      failed: summary.failed_samples,
      missing: summary.unattempted_samples ?? Math.max(0, planned - summary.passed_samples - summary.failed_samples),
      unknownUsage: summary.unknown_usage_samples ?? null,
      interventions: summary.manual_interventions_in_samples ?? null,
      hostFaults: summary.host_fault_samples ?? null,
      billedCost: summary.billing_verified === true ? summary.total_billed_cost_usd : null,
      costEstimate: summary.recorded_usage_peak_rate_estimate_usd ?? null,
      ratio: planned > 0 ? passed / planned : null,
      interval: wilson95(passed, planned),
      allPassed: summary.all_planned_samples_passed === true,
    });
  }
  return [...groups.values()].sort(
    (a, b) => a.model.localeCompare(b.model) || a.endpoint.localeCompare(b.endpoint) || a.thinking.localeCompare(b.thinking) || a.scope.localeCompare(b.scope),
  );
}

/** Wilson 95% interval; null when there is no planned population. */
export function wilson95(passed, planned) {
  if (planned === 0) return null;
  const z = 1.96, p = passed / planned, z2 = z * z, denominator = 1 + z2 / planned;
  const center = (p + z2 / (2 * planned)) / denominator;
  const margin = z * Math.sqrt((p * (1 - p) + z2 / (4 * planned)) / planned) / denominator;
  return [Math.max(0, center - margin), Math.min(1, center + margin)];
}

/** A versioned gate applies only to matching configuration and measurement scope. */
export function checkGate(groups, gate) {
  if (!gate || gate.schema_version !== 1 || typeof gate.model !== "string" || typeof gate.endpoint !== "string" || typeof gate.scope !== "string"
    || !/^[0-9a-f]{64}$/i.test(gate.spec_sha256 ?? "") || !/^[0-9a-f]{64}$/i.test(gate.fixture_source_sha256 ?? "")
    || !Number.isFinite(gate.baseline_success_ratio) || gate.baseline_success_ratio < 0 || gate.baseline_success_ratio > 1
    || !Number.isFinite(gate.allowed_drop) || gate.allowed_drop < 0 || gate.allowed_drop > 1
    || !Number.isSafeInteger(gate.max_missing_samples) || gate.max_missing_samples < 0
    || (gate.min_runs !== undefined && (!Number.isSafeInteger(gate.min_runs) || gate.min_runs < 1))
    || (gate.max_host_fault_samples !== undefined && (!Number.isSafeInteger(gate.max_host_fault_samples) || gate.max_host_fault_samples < 0))
    || (gate.max_manual_interventions_in_samples !== undefined && (!Number.isSafeInteger(gate.max_manual_interventions_in_samples) || gate.max_manual_interventions_in_samples < 0))
    || (gate.max_unknown_usage_samples !== undefined && (!Number.isSafeInteger(gate.max_unknown_usage_samples) || gate.max_unknown_usage_samples < 0))
    || (gate.max_billed_cost_usd !== undefined && (!Number.isFinite(gate.max_billed_cost_usd) || gate.max_billed_cost_usd < 0))) return ["invalid evaluation gate"];
  const matches = groups.filter(group => group.model === gate.model && group.endpoint === gate.endpoint && group.scope === gate.scope && group.thinking === (gate.thinking ?? "unspecified"));
  if (matches.length !== 1 || matches[0].runs.length === 0) return ["evaluation gate has no matching run"];
  if (matches[0].specSha256 !== gate.spec_sha256 || matches[0].fixtureSha256 !== gate.fixture_source_sha256) return ["evaluation gate baseline fingerprints differ from run"];
  const errors = [];
  if (gate.min_runs !== undefined && matches[0].runs.length < gate.min_runs) errors.push("evaluation gate has fewer runs than required");
  for (const run of matches[0].runs) {
    if (run.ratio === null || run.ratio < gate.baseline_success_ratio - gate.allowed_drop) errors.push(`${run.source}: success ratio below gate`);
    if (run.missing > gate.max_missing_samples) errors.push(`${run.source}: missing samples exceed gate`);
    if (gate.max_host_fault_samples !== undefined && (run.hostFaults === null || run.hostFaults > gate.max_host_fault_samples)) errors.push(`${run.source}: host faults unknown or exceed gate`);
    if (gate.max_manual_interventions_in_samples !== undefined && (run.interventions === null || run.interventions > gate.max_manual_interventions_in_samples)) errors.push(`${run.source}: interventions unknown or exceed gate`);
    if (gate.max_unknown_usage_samples !== undefined && (run.unknownUsage === null || run.unknownUsage > gate.max_unknown_usage_samples)) errors.push(`${run.source}: usage unknown or exceeds gate`);
    if (gate.max_billed_cost_usd !== undefined && (run.billedCost === null || run.billedCost > gate.max_billed_cost_usd)) errors.push(`${run.source}: billed cost unknown or exceeds gate`);
  }
  return errors;
}

function percent(ratio) {
  return ratio === null ? "n/a" : `${(ratio * 100).toFixed(1)}%`;
}

/** Markdown trend tables, one section per model and endpoint. */
export function renderMarkdown(groups, { specSha256, fixtureSha256 } = {}) {
  const lines = ["# Upgrade evaluation trend", ""];
  if (specSha256 || fixtureSha256) {
    lines.push(`- spec_sha256: \`${specSha256 ?? "unknown"}\``);
    lines.push(`- fixture_source_sha256: \`${fixtureSha256 ?? "unknown"}\``);
    lines.push("");
  }
  if (groups.length === 0) {
    lines.push("No summaries found.");
    return `${lines.join("\n")}\n`;
  }
  for (const group of groups) {
    lines.push(`## ${group.model} @ ${group.endpoint} (${group.thinking}; ${group.scope})`, "");
    lines.push("| Run | Passed / planned | Success | Wilson 95% | Missing | Usage unknown | Interventions | Host faults | Billed USD | Estimate USD | All passed |");
    lines.push("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for (const run of group.runs) {
      lines.push(
        `| ${path.basename(run.source)} | ${run.passed} / ${run.planned} | ${percent(run.ratio)} | ${run.interval ? `${percent(run.interval[0])}–${percent(run.interval[1])}` : "n/a"} | ${run.missing} | ${run.unknownUsage ?? "unknown"} | ${run.interventions ?? "unknown"} | ${run.hostFaults ?? "unknown"} | ${run.billedCost ?? "unknown"} | ${run.costEstimate ?? "unknown"} | ${run.allPassed ? "yes" : "no"} |`,
      );
    }
    lines.push("");
  }
  return `${lines.join("\n")}\n`;
}

function defaultInputs(repoRoot) {
  const dir = path.join(repoRoot, "docs", "upgrade", "evidence");
  return readdirSync(dir)
    .filter((name) => /-summary-.*\.json$/.test(name))
    .sort()
    .map((name) => path.join(dir, name));
}

/** Load and validate files. Returns { entries, errors }. */
export function loadSummaries(files) {
  const entries = [];
  const errors = [];
  for (const file of files) {
    let summary;
    try {
      summary = JSON.parse(readFileSync(file, "utf8"));
    } catch (error) {
      errors.push(`${file}: ${error.message}`);
      continue;
    }
    const problems = validateSummary(summary, file);
    if (problems.length > 0) {
      errors.push(...problems);
      continue;
    }
    entries.push({ source: file, summary });
  }
  return { entries, errors };
}

function main(argv) {
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, "..");
  const outIndex = argv.indexOf("--out");
  const outFile = outIndex >= 0 ? argv[outIndex + 1] : null;
  const gateIndex = argv.indexOf("--gate");
  const gateFile = gateIndex >= 0 ? argv[gateIndex + 1] : null;
  if ((outIndex >= 0 && !outFile) || (gateIndex >= 0 && !gateFile)) { console.error("missing --out or --gate argument"); return 1; }
  const positional = argv.filter((arg, i) => arg !== "--out" && arg !== "--gate" && (outIndex < 0 || i !== outIndex + 1) && (gateIndex < 0 || i !== gateIndex + 1));
  const files = positional.length > 0 ? positional : defaultInputs(repoRoot);

  const { entries, errors } = loadSummaries(files);
  if (errors.length > 0) {
    console.error(errors.join("\n"));
    return 1;
  }
  const consistency = checkConsistency(entries);
  if (!consistency.ok) {
    console.error(consistency.errors.join("\n"));
    return 1;
  }
  const spec = entries[0]?.summary.spec_sha256;
  const fixture = entries[0]?.summary.fixture_source_sha256;
  const groups = buildTrend(entries);
  if (gateFile) {
    let gate;
    try { gate = JSON.parse(readFileSync(gateFile, "utf8")); } catch (error) { console.error(`${gateFile}: ${error.message}`); return 1; }
    const failures = checkGate(groups, gate);
    if (failures.length) { console.error(failures.join("\n")); return 1; }
  }
  const markdown = renderMarkdown(groups, { specSha256: spec, fixtureSha256: fixture });
  if (outFile) {
    writeFileSync(outFile, markdown);
  } else {
    process.stdout.write(markdown);
  }
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  process.exitCode = main(process.argv.slice(2));
}
