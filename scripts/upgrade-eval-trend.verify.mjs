// Verification for scripts/upgrade-eval-trend.mjs. Run: node scripts/upgrade-eval-trend.verify.mjs
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  buildTrend,
  checkGate,
  checkConsistency,
  loadSummaries,
  renderMarkdown,
  validateSummary,
  wilson95,
} from "./upgrade-eval-trend.mjs";

const SPEC = "a".repeat(64);
const SPEC_OTHER = "b".repeat(64);
const FIXTURE = "c".repeat(64);
const FIXTURE_OTHER = "d".repeat(64);

function summary(overrides = {}) {
  return {
    spec_sha256: SPEC,
    fixture_source_sha256: FIXTURE,
    model: "deepseek-flash",
    endpoint: "https://api.deepseek.com",
    planned_samples: 36,
    passed_samples: 33,
    failed_samples: 3,
    all_planned_samples_passed: false,
    ...overrides,
  };
}

const tests = [];
function test(name, fn) {
  tests.push({ name, fn });
}

test("validateSummary rejects missing hashes and bad counts", () => {
  assert.deepEqual(validateSummary(summary(), "ok"), []);
  const problems = validateSummary(summary({ spec_sha256: "", planned_samples: -1 }), "bad");
  assert.ok(problems.some((p) => p.includes("missing spec_sha256")));
  assert.ok(problems.some((p) => p.includes("planned_samples")));
  assert.ok(validateSummary(summary({ fixture_source_sha256: "not-a-hash" }), "hash").some((p) => p.includes("invalid fixture_source_sha256")));
  const inconsistent = validateSummary(summary({ attempted_samples: 35, unattempted_samples: 1, unknown_usage_samples: 36 }), "counts");
  assert.ok(inconsistent.some((p) => p.includes("passed + failed")));
  assert.ok(inconsistent.some((p) => p.includes("unknown usage")));
  assert.ok(validateSummary(summary({ billing_verified: true, total_billed_cost_usd: null }), "billing").some((p) => p.includes("verified billing")));
  assert.ok(validateSummary(summary({ statuses: { passed: 32, failed: 3 } }), "status").some((p) => p.includes("status counts")));
  assert.ok(validateSummary(summary({ by_task: { one: { attempted: 1, passed: 1, failed: 0 } } }), "by_task").some((p) => p.includes("by_task counts")));
  assert.ok(validateSummary(summary({ planned_samples: Number.MAX_SAFE_INTEGER + 1 }), "unsafe").some((p) => p.includes("planned_samples")));
  assert.ok(validateSummary(summary({ manual_interventions_in_samples: 37 }), "intervention").some((p) => p.includes("manual interventions exceed")));
  assert.ok(validateSummary(summary({ host_fault_samples: 37 }), "fault").some((p) => p.includes("host faults exceed")));
  assert.ok(validateSummary(summary({ planned_unique_tasks: 12, repeats: 2 }), "matrix").some((p) => p.includes("task count times repeats")));
  assert.ok(validateSummary(summary({ observed_success_ratio: 1 }), "ratio").some((p) => p.includes("observed success ratio")));
  assert.ok(validateSummary(summary({ by_task: { one: { attempted: 36, passed: 33, failed: 2 } } }), "task failures").some((p) => p.includes("by_task counts")));
});

test("checkConsistency accepts one spec and one fixture hash", () => {
  const result = checkConsistency([
    { source: "a-summary-1.json", summary: summary() },
    { source: "a-summary-2.json", summary: summary({ passed_samples: 36, failed_samples: 0 }) },
  ]);
  assert.equal(result.ok, true);
});

test("checkConsistency refuses mixed spec hashes and names the offenders", () => {
  const result = checkConsistency([
    { source: "old-summary.json", summary: summary() },
    { source: "new-summary.json", summary: summary({ spec_sha256: SPEC_OTHER }) },
  ]);
  assert.equal(result.ok, false);
  assert.match(result.errors[0], /spec_sha256/);
  assert.match(result.errors[0], /old-summary\.json/);
  assert.match(result.errors[0], /new-summary\.json/);
});

test("checkConsistency refuses mixed fixture hashes", () => {
  const result = checkConsistency([
    { source: "a-summary.json", summary: summary() },
    { source: "b-summary.json", summary: summary({ fixture_source_sha256: FIXTURE_OTHER }) },
  ]);
  assert.equal(result.ok, false);
  assert.match(result.errors[0], /fixture_source_sha256/);
});

test("buildTrend groups by model and endpoint, keeps run order, computes ratios", () => {
  const entries = [
    { source: "r1-summary.json", summary: summary({ planned_samples: 4, passed_samples: 2, failed_samples: 2 }) },
    { source: "r2-summary.json", summary: summary({ model: "other", planned_samples: 4, passed_samples: 4, failed_samples: 0, all_planned_samples_passed: true }) },
    { source: "r3-summary.json", summary: summary({ planned_samples: 4, passed_samples: 4, failed_samples: 0, all_planned_samples_passed: true }) },
  ];
  const groups = buildTrend(entries);
  assert.equal(groups.length, 2);
  assert.equal(groups[0].model, "deepseek-flash");
  assert.deepEqual(groups[0].runs.map((r) => r.source), ["r1-summary.json", "r3-summary.json"]);
  assert.equal(groups[0].runs[0].ratio, 0.5);
  assert.equal(groups[0].runs[1].allPassed, true);
  assert.equal(groups[1].model, "other");
});

test("renderMarkdown prints one table per group with the shared hashes", () => {
  const groups = buildTrend([{ source: "x/r1-summary.json", summary: summary() }]);
  const md = renderMarkdown(groups, { specSha256: SPEC, fixtureSha256: FIXTURE });
  assert.match(md, /## deepseek-flash @ https:\/\/api\.deepseek\.com \(unspecified; unspecified\)/);
  assert.match(md, /\| r1-summary\.json \| 33 \/ 36 \| 91\.7% \| \d+\.\d%–\d+\.\d% \| 0 \| unknown \| unknown \| unknown \| unknown \| unknown \| no \|/);
  assert.match(md, new RegExp(`spec_sha256: \`${SPEC}\``));
});

test("Wilson interval handles empty and full populations", () => {
  assert.equal(wilson95(0, 0), null);
  const interval = wilson95(36, 36);
  assert.ok(interval[0] < 1 && interval[0] > 0.8);
  assert.equal(interval[1], 1);
});

test("gate rejects regression, missing samples and unknown host faults without mixing scope", () => {
  const gate = { schema_version: 1, model: "deepseek-flash", endpoint: "https://api.deepseek.com", scope: "desktop", spec_sha256: SPEC, fixture_source_sha256: FIXTURE, baseline_success_ratio: 0.9, allowed_drop: 0.05, max_missing_samples: 0, max_host_fault_samples: 0 };
  const groups = buildTrend([{ source: "desktop.json", summary: summary({ scope: "desktop", planned_samples: 10, attempted_samples: 9, unattempted_samples: 1, passed_samples: 8, failed_samples: 1 }) }]);
  const failures = checkGate(groups, gate);
  assert.ok(failures.some(failure => failure.includes("success ratio")));
  assert.ok(failures.some(failure => failure.includes("missing samples")));
  assert.ok(failures.some(failure => failure.includes("host faults unknown")));
  assert.deepEqual(checkGate(buildTrend([{ source: "kernel.json", summary: summary({ scope: "kernel" }) }]), gate), ["evaluation gate has no matching run"]);
  assert.deepEqual(checkGate(groups, { ...gate, spec_sha256: SPEC_OTHER }), ["evaluation gate baseline fingerprints differ from run"]);
});

test("optional intervention, usage and billed-cost gates fail closed on unknowns", () => {
  const gate = { schema_version: 1, model: "deepseek-flash", endpoint: "https://api.deepseek.com", scope: "kernel", spec_sha256: SPEC, fixture_source_sha256: FIXTURE, baseline_success_ratio: 0.9, allowed_drop: 0.1, max_missing_samples: 0, max_manual_interventions_in_samples: 0, max_unknown_usage_samples: 0, max_billed_cost_usd: 1 };
  const unknown = buildTrend([{ source: "unknown.json", summary: summary({ scope: "kernel" }) }]);
  const failures = checkGate(unknown, gate);
  assert.ok(failures.some(failure => failure.includes("interventions unknown")));
  assert.ok(failures.some(failure => failure.includes("usage unknown")));
  assert.ok(failures.some(failure => failure.includes("billed cost unknown")));
  const known = buildTrend([{ source: "known.json", summary: summary({ scope: "kernel", manual_interventions_in_samples: 0, unknown_usage_samples: 0, billing_verified: true, total_billed_cost_usd: 0.5 }) }]);
  assert.deepEqual(checkGate(known, gate), []);
  assert.deepEqual(checkGate(known, { ...gate, max_billed_cost_usd: -1 }), ["invalid evaluation gate"]);
});

test("gate requires the configured number of comparable runs", () => {
  const gate = { schema_version: 1, model: "deepseek-flash", endpoint: "https://api.deepseek.com", scope: "kernel", spec_sha256: SPEC, fixture_source_sha256: FIXTURE, baseline_success_ratio: 0.9, allowed_drop: 0.1, max_missing_samples: 0, min_runs: 2 };
  const one = buildTrend([{ source: "one.json", summary: summary({ scope: "kernel" }) }]);
  assert.ok(checkGate(one, gate).some(error => error.includes("fewer runs")));
  const two = buildTrend([{ source: "one.json", summary: summary({ scope: "kernel" }) }, { source: "two.json", summary: summary({ scope: "kernel" }) }]);
  assert.deepEqual(checkGate(two, gate), []);
  assert.deepEqual(checkGate(two, { ...gate, min_runs: 0 }), ["invalid evaluation gate"]);
});

test("loadSummaries reports unreadable and invalid files without throwing", () => {
  const dir = mkdtempSync(path.join(tmpdir(), "eval-trend-"));
  try {
    const good = path.join(dir, "good-summary-1.json");
    const bad = path.join(dir, "bad-summary-2.json");
    const broken = path.join(dir, "broken-summary-3.json");
    writeFileSync(good, JSON.stringify(summary()));
    writeFileSync(bad, JSON.stringify({ model: "x" }));
    writeFileSync(broken, "{not json");
    const { entries, errors } = loadSummaries([good, bad, broken]);
    assert.equal(entries.length, 1);
    assert.ok(errors.some((e) => e.startsWith(bad)), "invalid summary is reported");
    assert.ok(errors.some((e) => e.startsWith(broken)), "unparseable file is reported");
    assert.ok(!errors.some((e) => e.startsWith(good)), "valid summary is not reported");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

let failed = 0;
for (const { name, fn } of tests) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    failed += 1;
    console.error(`not ok - ${name}\n${error.message}`);
  }
}
if (failed > 0) {
  process.exitCode = 1;
} else {
  console.log(`all ${tests.length} checks passed`);
}
