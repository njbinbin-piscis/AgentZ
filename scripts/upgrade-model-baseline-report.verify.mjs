import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'agentz-model-report-'));
const input = path.join(dir, 'observations.json');
const spec = {schema_version:1, spec_sha256:'a'.repeat(64), fixture_source_sha256:'b'.repeat(64),
  model:'deepseek-flash', endpoint:'https://api.deepseek.com', thinking:false, repeats:3, tasks:[{id:'one'},{id:'two'}]};
const sample = {task_id:'one',repeat:1,success:true,status:'passed',artifact_verified:true,tool_reads:1,tool_writes:1,
  collector_failed:false,timeout:false,error_codes:[],input_tokens:100,output_tokens:50,unknown_usage_responses:0,
  elapsed_ms:100,logical_calls:3,manual_interventions:0};
const run = {schema_version:1, spec_sha256:spec.spec_sha256, fixture_source_sha256:spec.fixture_source_sha256,
  model:spec.model,endpoint:spec.endpoint,thinking:false,repeats:3,records:[sample],stopped:null,scope:'fixture',unverified:['GUI']};
fs.writeFileSync(`${input}.spec.json`, JSON.stringify(spec));
function report(value) {
  fs.writeFileSync(input, JSON.stringify(value));
  return spawnSync(process.execPath, ['scripts/upgrade-model-baseline-report.mjs', input], {encoding:'utf8', windowsHide:true});
}
let result = report(run);
assert.equal(result.status,0);
const summary = JSON.parse(result.stdout);
assert.equal(summary.passed_samples,1);
assert.equal(summary.unattempted_samples,5);
assert.equal(summary.all_planned_samples_passed,false);
assert.equal(summary.total_billed_cost_usd,null);
assert.equal(report({...run,records:[sample,sample]}).status,1);
assert.equal(report({...run,records:[{...sample,error_codes:['auth_failed']}]}).status,1);
assert.equal(report({...run,fixture_source_sha256:'c'.repeat(64)}).status,1);
result = report({...run,records:[{...sample,input_tokens:null,output_tokens:null,unknown_usage_responses:1}]});
assert.equal(result.status,0);
assert.equal(JSON.parse(result.stdout).unknown_usage_samples,1);
assert.equal(JSON.parse(result.stdout).billing_verified,false);
console.log('Model report verification passed: partial coverage, duplicate, contradictory success, source pin and unknown usage');
