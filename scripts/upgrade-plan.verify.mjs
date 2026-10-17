// Verify ledger guardrails only in an isolated fixture; canonical statuses stay untouched.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import crypto from 'node:crypto';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const original = path.join(root, 'docs/upgrade/plan.json');
const before = fs.readFileSync(original, 'utf8');
const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'agentz-plan-check-'));
const file = path.join(fixture, 'plan.json');
const seed = JSON.parse(before);
for (const task of seed.tasks) {
  Object.assign(task, {status:'pending',owner:null,reviewer:null,evidence:[],blocker:null,
    started_at:null,completed_at:null,completion_summary:null,review_notes:null,actual_days:null,history:[]});
  delete task.code_dependencies;
  delete task.implementation_review;
}
for (const milestone of seed.milestones) {
  Object.assign(milestone, {status:'pending',decision_owner:null,evidence:[],decision_note:null,history:[]});
}
fs.writeFileSync(file, JSON.stringify(seed, null, 2));
function run(expected, ...args) {
  const result = spawnSync(process.execPath, [path.join(root, 'scripts/upgrade-plan.mjs'), ...args, '--file', file], {encoding:'utf8'});
  assert.equal(result.status, expected, `${args.join(' ')}: ${result.stdout}${result.stderr}`);
  return result;
}
run(0, 'validate');
const unchanged = fs.readFileSync(file, 'utf8');
run(1, 'update', 'AZ-007', '--owner', 'fixture-owner', '--status', 'doing', '--note', 'fixture unmet dependencies');
assert.equal(fs.readFileSync(file, 'utf8'), unchanged, 'Rejected updates must not persist');
run(1, 'update', 'AZ-007', '--owner', 'fixture-owner', '--status', 'preparing', '--note', 'candidate must respect task dependencies');
assert.equal(fs.readFileSync(file, 'utf8'), unchanged, 'Candidate rejection must not persist');
run(1, 'update', 'AZ-001', '--owner', 'fixture-owner', '--status', 'done', '--note', 'fixture illegal transition');
run(0, 'update', 'AZ-001', '--owner', 'fixture-owner', '--status', 'doing', '--note', 'fixture start');
const initialSource=path.join(fixture,'contract.txt'),initialReview=path.join(fixture,'contract-review.md'),initialManifest=path.join(fixture,'contract-manifest.json');
fs.writeFileSync(initialSource,'fixture contract'); fs.writeFileSync(initialReview,'fixture source review');
fs.writeFileSync(initialManifest,JSON.stringify({files:[{path:initialSource,sha256:crypto.createHash('sha256').update(fs.readFileSync(initialSource)).digest('hex')}]}));
// Independent implementation records are accepted only for candidate/review tasks.
const initialPlan=JSON.parse(fs.readFileSync(file,'utf8'));initialPlan.tasks.find(t=>t.id==='AZ-001').status='preparing';
fs.writeFileSync(file,JSON.stringify(initialPlan));
run(0,'record-implementation-review','AZ-001','--reviewer','fixture-reviewer','--independent-review','true','--scope','contract','--verification','fixture only','--evidence',initialReview,'--source-manifest',initialManifest,'--note','fixture contract review');
run(0, 'update', 'AZ-001', '--status', 'review', '--note', 'fixture review');
const reviewing = fs.readFileSync(file, 'utf8');
run(1, 'update', 'AZ-001', '--status', 'done', '--note', 'fixture missing evidence');
assert.equal(fs.readFileSync(file, 'utf8'), reviewing);
run(0, 'update', 'AZ-001', '--status', 'done', '--reviewer', 'fixture-reviewer', '--evidence', 'fixture-only', '--summary', 'fixture completion', '--review', 'fixture retrospective', '--actual-days', '1', '--gui','not_required', '--note', 'fixture accepted');
run(1, 'gate', 'M0', '--owner', 'fixture-owner', '--evidence', 'fixture-only', '--note', 'fixture incomplete gate');
run(1, 'update', 'AZ-001', '--status', 'doing', '--note', 'fixture immutable done');
const addition = {...seed.tasks.find(task=>task.id==='AZ-012'), id:'AZ-999', title:'fixture follow-up', dependencies:['AZ-001']};
const additionFile=path.join(fixture,'addition.json'); fs.writeFileSync(additionFile,JSON.stringify(addition));
run(0,'add','--task-json',additionFile,'--note','fixture new finding');
run(0,'update','AZ-999','--owner','fixture-owner','--status','preparing','--note','candidate can await gate with completed task dependencies');
const candidate=fs.readFileSync(file,'utf8');
run(1,'update','AZ-999','--status','review','--note','candidate cannot bypass gate');
assert.equal(fs.readFileSync(file,'utf8'),candidate,'Candidate cannot bypass acceptance gate');
const added=fs.readFileSync(file,'utf8');
run(1,'add','--task-json',additionFile,'--note','fixture duplicate');
assert.equal(fs.readFileSync(file,'utf8'),added,'Duplicate additions must not persist');
addition.id='AZ-998'; addition.dependencies=['missing']; fs.writeFileSync(additionFile,JSON.stringify(addition));
run(1,'add','--task-json',additionFile,'--note','fixture unknown dependency');
assert.equal(fs.readFileSync(file,'utf8'),added,'Invalid additions must not persist');

// ADR-006: source-pinned independent implementation reviews allow candidate
// code consumers only; original acceptance dependencies/Gates remain unchanged.
const sources = path.join(fixture, 'implementation.rs');
const evidence = path.join(fixture, 'implementation-evidence.md');
const manifestFile = path.join(fixture, 'implementation-manifest.json');
fs.writeFileSync(sources, 'fixture implementation v1');
fs.writeFileSync(evidence, 'fixture independent review: unit tests passed; desktop acceptance not executed');
const manifest = () => fs.writeFileSync(manifestFile, JSON.stringify({files:[{
  path:sources,sha256:crypto.createHash('sha256').update(fs.readFileSync(sources)).digest('hex'),
}]}));
manifest();
const consumer = {...addition, id:'AZ-998', title:'fixture candidate consumer', dependencies:['AZ-999']};
fs.writeFileSync(additionFile,JSON.stringify(consumer));
run(0,'add','--task-json',additionFile,'--note','fixture code consumer');
const beforeUnreviewed = fs.readFileSync(file,'utf8');
run(0,'update','AZ-998','--owner','fixture-consumer','--status','preparing','--code-dependencies','AZ-999','--note','unreviewed candidate warning');
const afterUnreviewed = fs.readFileSync(file,'utf8');
const reviewArgs = ['record-implementation-review','AZ-999','--reviewer','fixture-main-reviewer','--scope','frozen fixture interface only',
  '--verification','fixture tests passed; GUI deferred','--evidence',evidence,'--source-manifest',manifestFile,'--note','fixture independent implementation acceptance'];
run(1,...reviewArgs);
assert.equal(fs.readFileSync(file,'utf8'),afterUnreviewed,'Missing explicit independent review must not persist');
run(1,...reviewArgs,'--independent-review','true','--reviewer','fixture-owner');
assert.equal(fs.readFileSync(file,'utf8'),afterUnreviewed,'Implementation owner cannot self-approve');
fs.writeFileSync(manifestFile,JSON.stringify({files:[]}));
run(1,...reviewArgs,'--independent-review','true');
assert.equal(fs.readFileSync(file,'utf8'),afterUnreviewed,'Empty source manifest cannot approve implementation');
manifest();
run(1,...reviewArgs,'--independent-review','true','--evidence',path.join(fixture,'missing-evidence.md'));
assert.equal(fs.readFileSync(file,'utf8'),afterUnreviewed,'Missing evidence cannot persist approval');
const summaryBefore = JSON.parse(run(0,'summary').stdout);
run(0,...reviewArgs,'--independent-review','true');
run(0,'update','AZ-998','--owner','fixture-consumer','--status','preparing','--code-dependencies','AZ-999','--note','reviewed candidate code consumption only');
const reviewedCandidate = fs.readFileSync(file,'utf8');
const summaryAfter = JSON.parse(run(0,'summary').stdout);
assert.equal(summaryAfter.done,summaryBefore.done);
assert.equal(summaryAfter.remaining,summaryBefore.remaining);
assert.deepEqual(summaryAfter.passed_gates,summaryBefore.passed_gates);
assert.deepEqual(JSON.parse(reviewedCandidate).tasks.find(t=>t.id==='AZ-998').dependencies,['AZ-999']);
run(1,'update','AZ-998','--status','review','--note','implementation approval cannot bypass acceptance');
run(1,'update','AZ-998','--status','done','--note','candidate cannot directly become accepted');
assert.equal(fs.readFileSync(file,'utf8'),reviewedCandidate);
const grandchild = {...consumer,id:'AZ-997',dependencies:['AZ-998']};
fs.writeFileSync(additionFile,JSON.stringify(grandchild));
run(0,'add','--task-json',additionFile,'--note','fixture grandchild');
const noRecursiveApproval = fs.readFileSync(file,'utf8');
run(0,'update','AZ-997','--owner','fixture-grandchild','--status','preparing','--code-dependencies','AZ-998','--note','recursive approval warning');
const afterRecursiveWarning=fs.readFileSync(file,'utf8');
fs.writeFileSync(sources,'fixture implementation v2');
run(0,'validate');
run(1,...reviewArgs,'--independent-review','true');
assert.equal(fs.readFileSync(file,'utf8'),afterRecursiveWarning,'Changed source cannot consume old fingerprints or persist failed review');
manifest();
run(0,...reviewArgs,'--independent-review','true');
run(0,'validate');
const refreshedReview = fs.readFileSync(file,'utf8');
run(1,'update','AZ-998','--code-dependencies','AZ-001','--note','cannot invent alternative dependency edges');
assert.equal(fs.readFileSync(file,'utf8'),refreshedReview);
const refreshedSummary = JSON.parse(run(0,'summary').stdout);
assert.equal(refreshedSummary.done,summaryBefore.done);
assert.deepEqual(refreshedSummary.passed_gates,summaryBefore.passed_gates);
const reviewedEvidence = fs.readFileSync(evidence,'utf8');
fs.writeFileSync(evidence,'fixture evidence changed after review');
run(0,'validate');
assert.equal(fs.readFileSync(file,'utf8'),refreshedReview,'Changed verification evidence cannot mutate ledger');
fs.writeFileSync(evidence,reviewedEvidence);
run(0,'validate');

const modified = JSON.parse(fs.readFileSync(file, 'utf8'));
modified.tasks.find(t=>t.id==='AZ-003').dependencies.push('AZ-003');
fs.writeFileSync(file, JSON.stringify(modified));
run(1, 'validate');
assert.equal(fs.readFileSync(original, 'utf8'), before, 'Canonical plan must remain untouched');
console.log('PASS: dependency/gate guards, legal transitions, completion evidence, atomic rejection, immutable completion, follow-up additions and cycle rejection; explicit independent source-pinned candidate reviews, stale-source rejection/refresh, no recursive exemptions and unchanged done/Gate counts; canonical plan unchanged');
console.log(`Isolated fixture retained: ${fixture}`);
