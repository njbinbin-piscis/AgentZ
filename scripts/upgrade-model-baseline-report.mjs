// Summarize measured observations; never substitute a process exit for task success.
import fs from 'node:fs';
import path from 'node:path';

const input = path.resolve(process.argv[2] ?? 'docs/upgrade/evidence/AZ-004-live-results-v1.json');
const output = process.argv[3] ? path.resolve(process.argv[3]) : null;
const run = JSON.parse(fs.readFileSync(input, 'utf8'));
const spec = JSON.parse(fs.readFileSync(`${input}.spec.json`, 'utf8'));
function requireFact(condition, message) { if (!condition) throw new Error(message); }
requireFact(run.schema_version === 1 && spec.schema_version === 1, 'Unknown baseline version');
requireFact(run.spec_sha256 === spec.spec_sha256 && run.fixture_source_sha256 === spec.fixture_source_sha256,
  'Execution and frozen fixture fingerprints differ');
requireFact(run.model === spec.model && run.endpoint === spec.endpoint && run.thinking === spec.thinking,
  'Execution configuration differs from frozen baseline');
requireFact(run.repeats === spec.repeats && Array.isArray(run.records), 'Invalid sample population');
const ids = new Set(spec.tasks.map(task => task.id));
const keys = new Set();
let knownInput = 0, knownOutput = 0, unknownUsageSamples = 0;
const statuses = {}, byTask = {};
for (const record of run.records) {
  const key = `${record.task_id}:${record.repeat}`;
  requireFact(ids.has(record.task_id) && Number.isInteger(record.repeat)
    && record.repeat >= 1 && record.repeat <= spec.repeats && !keys.has(key), 'Unknown or duplicate sample');
  keys.add(key);
  requireFact(typeof record.success === 'boolean' && typeof record.status === 'string', 'Missing task result');
  requireFact(!record.success || (record.status === 'passed' && record.artifact_verified === true
    && record.tool_reads > 0 && record.tool_writes > 0 && !record.collector_failed
    && !record.timeout && record.error_codes?.length === 0), 'Contradictory successful task record');
  statuses[record.status] = (statuses[record.status] ?? 0) + 1;
  const task = byTask[record.task_id] ??= {attempted: 0, passed: 0, failed: 0};
  task.attempted++;
  task[record.success ? 'passed' : 'failed']++;
  if (record.input_tokens === null || record.output_tokens === null || record.unknown_usage_responses > 0) {
    unknownUsageSamples++;
  } else {
    requireFact(Number.isSafeInteger(record.input_tokens) && record.input_tokens >= 0
      && Number.isSafeInteger(record.output_tokens) && record.output_tokens >= 0, 'Invalid recorded usage');
    knownInput += record.input_tokens;
    knownOutput += record.output_tokens;
  }
}
const passed = run.records.filter(record => record.success).length;
const latencies = run.records.map(record => record.elapsed_ms).sort((a, b) => a - b);
requireFact(latencies.every(value => Number.isSafeInteger(value) && value >= 0), 'Invalid latency');
const quantile = fraction => latencies.length ? latencies[Math.ceil(latencies.length * fraction) - 1] : null;
const summary = {
  schema_version: 1, scope: run.scope, spec_sha256: run.spec_sha256,
  fixture_source_sha256: run.fixture_source_sha256, model: run.model, endpoint: run.endpoint,
  thinking: run.thinking,
  planned_unique_tasks: ids.size, repeats: spec.repeats, planned_samples: ids.size * spec.repeats,
  attempted_samples: run.records.length, unattempted_samples: ids.size * spec.repeats - run.records.length,
  passed_samples: passed, failed_samples: run.records.length - passed,
  observed_success_ratio: run.records.length ? passed / run.records.length : null,
  all_planned_samples_passed: run.records.length === ids.size * spec.repeats && passed === run.records.length && !run.stopped,
  statuses, by_task: byTask, elapsed_ms: {p50: quantile(0.5), p95: quantile(0.95)},
  logical_calls: run.records.reduce((sum, record) => sum + record.logical_calls, 0),
  recorded_input_tokens_known_samples: knownInput, recorded_output_tokens_known_samples: knownOutput,
  unknown_usage_samples: unknownUsageSamples,
  pricing_source: 'https://api-docs.deepseek.com/quick_start/pricing/', pricing_checked: '2026-10-09',
  // Official peak cache-miss/input + output rates; cache and time can lower the charge.
  recorded_usage_peak_rate_estimate_usd: (knownInput * 0.3 + knownOutput * 1.2) / 1_000_000,
  total_billed_cost_usd: null, billing_verified: false, wire_request_count_verified: false,
  manual_interventions_in_samples: run.records.reduce((sum, record) => sum + record.manual_interventions, 0),
  stopped: run.stopped, unverified: run.unverified,
  limitations: ['Synthetic artifact oracle; substring checks are not full semantic correctness',
    'Fixed order and repeated tasks are not an independent random sample',
    'Latency is kernel task latency, not desktop startup or UI response',
    'Recorded usage estimate is not provider invoice or a verified whole-run fee'],
};
if (output) fs.writeFileSync(output, JSON.stringify(summary, null, 2) + '\n');
console.log(JSON.stringify(summary, null, 2));
