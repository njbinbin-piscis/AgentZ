// Local, dependency-free task ledger. Does not implement or publish product changes.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const command = args.shift() ?? 'summary';
const positional = [];
const opts = {};
while (args.length) {
  const value = args.shift();
  if (!value.startsWith('--')) { positional.push(value); continue; }
  const key = value.slice(2);
  if (!args.length || args[0].startsWith('--')) throw new Error(`Missing value for --${key}`);
  opts[key] = args.shift();
}
const file = path.resolve(opts.file ?? path.join(root, 'docs/upgrade/plan.json'));

// Concurrency: write commands hold an exclusive lock file for the whole process, so two sessions
// cannot interleave read-modify-write of plan.json. A stale lock must be removed by hand.
const writeCommands = new Set(['add', 'update', 'gate', 'claim', 'release', 'reeval', 'record-implementation-review', 'record-implementation-reviews']);
const lockFile = path.join(path.dirname(file), '.plan.lock');
if (writeCommands.has(command)) {
  try { fs.closeSync(fs.openSync(lockFile, 'wx')); }
  catch (error) {
    if (error.code === 'EEXIST') {
      console.error(`Ledger is locked by another write (${lockFile}). Wait for it to finish; if no ledger command is running, delete that lock file.`);
      process.exit(1);
    }
    throw error;
  }
  process.on('exit', () => { try { fs.unlinkSync(lockFile); } catch {} });
}

const plan = JSON.parse(fs.readFileSync(file, 'utf8'));
const statuses = ['pending', 'preparing', 'doing', 'review', 'blocked', 'done', 'deferred'];
const byId = new Map(plan.tasks.map(t => [t.id, t]));
const gates = new Map(plan.milestones.map(m => [m.id, m]));
// Scope-local freshness: only tasks whose status is being moved to review/done (or whose review is being
// recorded) must have a fully fresh implementation review. Other stale or non-independent reviews are warnings.
const strictReviews = new Set();
const warnings = [];
const GUI_STATUS = /^(verified|not_required|waived:ADR-\d{3})$/;

function requireValue(condition, message) { if (!condition) throw new Error(message); }
// Actor identity is the base name before any suffix, e.g. "Codex (宿主)" or "Codex current_source_review ...".
function actorBase(value) { return String(value ?? '').trim().split(/[\s(（]/)[0].toLowerCase(); }
function sameActor(a, b) { return actorBase(a) !== '' && actorBase(a) === actorBase(b); }
function reviewPath(value) {
  requireValue(typeof value === 'string' && value.trim(), 'Implementation review requires a file path');
  const resolved = path.resolve(root, value);
  const real = fs.realpathSync(resolved);
  const inside = (base) => {
    const relative = path.relative(fs.realpathSync(base), real);
    return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative));
  };
  requireValue(inside(root) || inside(path.dirname(file)), 'Review file escapes repository/isolated ledger directory');
  requireValue(fs.statSync(real).isFile(), 'Implementation review path must be a file');
  return resolved;
}
function fileHash(value) {
  return crypto.createHash('sha256').update(fs.readFileSync(reviewPath(value))).digest('hex');
}
function currentHash(value) {
  try { return fileHash(value); } catch { return null; }
}
// Returns a list of problems with a task's implementation review. Empty list = fresh and independent.
function reviewProblems(t, checkHashes) {
  const review = t.implementation_review;
  if (!review || review.independent_review !== true || !review.reviewer) {
    return [`${t.id}: implementation review missing or not independent`];
  }
  const problems = [];
  if (sameActor(review.reviewer, t.owner)) problems.push(`${t.id}: reviewer "${review.reviewer}" is the same actor as owner "${t.owner}"`);
  if (!(review.scope?.trim() && review.verification?.trim() && review.recorded_at && review.evidence && review.source_manifest && Array.isArray(review.sources) && review.sources.length)) {
    problems.push(`${t.id}: implementation review lacks scope, verification, evidence or sources`);
    return problems;
  }
  if (new Set(review.sources.map(s => s.path)).size !== review.sources.length) problems.push(`${t.id}: duplicate reviewed source path`);
  for (const record of [review.evidence, review.source_manifest, ...review.sources]) {
    if (!(typeof record.path === 'string' && /^[0-9a-f]{64}$/.test(record.sha256 ?? ''))) { problems.push(`${t.id}: invalid reviewed file fingerprint`); continue; }
    if (checkHashes && currentHash(record.path) !== record.sha256) problems.push(`${t.id}: fingerprint changed: ${record.path}`);
  }
  return problems;
}
function enforce(problems, strict) {
  if (!problems.length) return;
  if (strict) throw new Error(problems.join('; '));
  warnings.push(...problems);
}
function reviewCurrent(t) {
  const problems = reviewProblems(t, true);
  requireValue(!problems.length, problems.join('; '));
}
function candidateDependencies(t) {
  for (const dependency of t.dependencies) {
    const upstream = byId.get(dependency);
    if (upstream?.status === 'done') continue;
    requireValue(t.code_dependencies?.includes(dependency), `${t.id}: candidate work has incomplete task dependencies`);
    requireValue(['preparing', 'review'].includes(upstream?.status), `${t.id}: upstream is not a candidate/review implementation`);
    const strict = strictReviews.has(t.id) || strictReviews.has(upstream.id);
    if (!upstream.implementation_review) enforce([`${t.id}: upstream ${dependency} has no implementation review`], strict);
    else enforce(reviewProblems(upstream, true), strict);
  }
}
function checkCycles(items, edgeKey, label) {
  const map = new Map(items.map(item => [item.id, item]));
  const seen = new Set(), active = new Set();
  function visit(id) {
    requireValue(map.has(id), `${label}: unknown dependency ${id}`);
    requireValue(!active.has(id), `${label}: dependency cycle at ${id}`);
    if (seen.has(id)) return;
    active.add(id);
    for (const dependency of map.get(id)[edgeKey]) visit(dependency);
    active.delete(id); seen.add(id);
  }
  for (const item of items) visit(item.id);
}
function validate() {
  requireValue(plan.schema_version === 1, 'Unsupported schema version');
  requireValue(byId.size === plan.tasks.length, 'Duplicate task ID');
  requireValue(gates.size === plan.milestones.length, 'Duplicate milestone ID');
  checkCycles(plan.tasks, 'dependencies', 'tasks');
  checkCycles(plan.milestones, 'depends_on', 'milestones');
  for (const t of plan.tasks) {
    requireValue(gates.has(t.milestone), `${t.id}: unknown milestone`);
    requireValue(statuses.includes(t.status), `${t.id}: invalid status`);
    requireValue(t.estimate_days > 0 && t.estimate_days <= 2, `${t.id}: split work exceeding 2 days`);
    for (const key of ['scope', 'deliverable', 'risk', 'rollback', 'owner_role', 'next_action'])
      requireValue(typeof t[key] === 'string' && t[key].trim(), `${t.id}: missing ${key}`);
    for (const key of ['acceptance_criteria', 'verification'])
      requireValue(Array.isArray(t[key]) && t[key].length, `${t.id}: missing ${key}`);
    requireValue(Array.isArray(t.evidence) && Array.isArray(t.history), `${t.id}: missing evidence/history`);
    if (t.code_dependencies !== undefined) {
      requireValue(Array.isArray(t.code_dependencies) && new Set(t.code_dependencies).size === t.code_dependencies.length
        && t.code_dependencies.every(id => t.dependencies.includes(id)), `${t.id}: code dependencies must be unique existing acceptance dependencies`);
    }
    if (t.implementation_review !== undefined) enforce(reviewProblems(t, true), strictReviews.has(t.id));
    if (t.status === 'preparing') {
      requireValue(t.owner && t.started_at, `${t.id}: candidate work needs owner/start time`);
      candidateDependencies(t);
    }
    if (['doing', 'review', 'done'].includes(t.status)) {
      requireValue(t.owner && t.started_at, `${t.id}: active work needs owner/start time`);
      requireValue(!unmet(t).length, `${t.id}: active work has incomplete dependencies/gates`);
    }
    if (t.status === 'done') {
      requireValue(t.reviewer && t.evidence.length && t.completion_summary && t.review_notes && t.completed_at && t.actual_days !== null, `${t.id}: done needs reviewer, evidence, summary, retrospective, actual days and completion time`);
      requireValue(t.dependencies.every(id => byId.get(id)?.status === 'done'), `${t.id}: done dependency is incomplete`);
    }
    if (['blocked', 'deferred'].includes(t.status)) requireValue(t.blocker, `${t.id}: blocked/deferred needs a reason`);
    if (t.actual_days !== null) requireValue(Number.isFinite(t.actual_days) && t.actual_days >= 0, `${t.id}: invalid actual days`);
  }
  for (const m of plan.milestones) {
    requireValue(['pending', 'passed'].includes(m.status), `${m.id}: invalid gate status`);
    if (m.status === 'passed') {
      requireValue(m.decision_owner && m.evidence.length && m.decision_note, `${m.id}: gate needs owner, evidence and decision`);
      requireValue(plan.tasks.filter(t => t.milestone === m.id).every(t => t.status === 'done'), `${m.id}: gate has incomplete tasks`);
      requireValue(m.depends_on.every(id => gates.get(id)?.status === 'passed'), `${m.id}: prerequisite gate incomplete`);
    }
  }
  // A task must not depend on a milestone that waits for that task's milestone.
  function needsGate(id, target, seen = new Set()) {
    if (id === target) return true;
    if (seen.has(id)) return false;
    seen.add(id);
    return gates.get(id).depends_on.some(dep => needsGate(dep, target, seen));
  }
  for (const t of plan.tasks) for (const dep of t.dependencies) {
    const other = byId.get(dep);
    if (other.milestone !== t.milestone)
      requireValue(!needsGate(other.milestone, t.milestone), `${t.id}: milestone/task dependency deadlock via ${dep}`);
  }
}
function unmet(t) {
  return [
    ...t.dependencies.filter(id => byId.get(id).status !== 'done'),
    ...gates.get(t.milestone).depends_on.filter(id => gates.get(id).status !== 'passed')
  ];
}
function assertClaim(t) {
  if (t.claimed_by && !(opts.agent && sameActor(opts.agent, t.claimed_by))) {
    throw new Error(`${t.id} is claimed by ${t.claimed_by}; pass --agent matching the claim, or ask the claimant to release it`);
  }
}
function printable(t) {
  return `${t.id} | ${t.milestone} | ${t.status} | ${t.owner ?? '未分配'} (${t.owner_role}) | ${t.estimate_days}d | ${t.title}`;
}
function currentNext(t) { return t.reeval?.next_action ?? t.next_action; }
function render() {
  const out = ['# AgentZ 原子任务执行台账', '', '> 由 scripts/upgrade-plan.mjs render 从 plan.json 生成；状态更新请使用 CLI 或修改 JSON 后 validate/render。', '',
    `共 ${plan.tasks.length} 项任务；理想工作量 ${plan.tasks.reduce((s,t) => s+t.estimate_days,0)} 有效工作日；不等于日历工期。`, ''];
  for (const m of plan.milestones) {
    out.push(`## ${m.id} · ${m.title}`, '', `Gate：${m.status}；前置：${m.depends_on.join(', ') || '无'}。`, '', `退出条件：${m.exit_criterion}。`, '');
    for (const t of plan.tasks.filter(t => t.milestone === m.id)) {
      out.push(`### ${t.id} · ${t.title}`, '',
        `- 状态：${t.status}；负责人：${t.owner ?? '未分配'}；角色：${t.owner_role}；优先级：${t.priority}。`,
        `- 仓库：${t.repository}；验收依赖：${t.dependencies.join(', ') || '无'}；候选代码依赖：${t.code_dependencies?.join(', ') || '沿用验收依赖'}；预计：${t.estimate_days}d；实际：${t.actual_days ?? '待记录'}d。`,
        `- 范围：${t.scope}。`, `- 交付物：${t.deliverable}。`,
        ...t.acceptance_criteria.map(c => `- 验收：${c}。`), ...t.verification.map(v => `- 验证：${v}。`),
        `- 关联审查：${t.finding_refs.join(', ')}；风险：${t.risk}。`, `- 回退：${t.rollback}。`,
        `- 阻塞：${t.blocker ?? '无'}；下一步：${t.next_action}。`,
        ...(t.reeval ? [`- 当前复评：${t.reeval.verified_state}；结论：${t.reeval.conclusion_proposal}；下一步：${currentNext(t)}。`,
          `- 复评剩余：${(t.reeval.remaining_work ?? []).join('；') || '见复评报告'}；验证缺口：${(t.reeval.verification_gaps ?? []).join('；') || '见复评报告'}。`] : []),
        `- 证据：${t.evidence.join('；') || '待附提交/PR/测试日志/截图或报告' }。`,
        `- 完成摘要：${t.completion_summary ?? '待填写'}；复盘：${t.review_notes ?? '待填写'}。`, '');
      out.push(`验收人：${t.reviewer ?? '未分配'}。`, '');
    }
  }
  fs.writeFileSync(path.join(path.dirname(file), 'tasks.md'), out.join('\n') + '\n');
  const done=plan.tasks.filter(t=>t.status==='done');
  const active=plan.tasks.filter(t=>['preparing','doing','review'].includes(t.status));
  const blocked=plan.tasks.filter(t=>t.status==='blocked');
  const ready=plan.tasks.filter(t=>t.status==='pending' && !unmet(t).length);
  const progress=['# AgentZ 实施进度', '', `更新时间：${new Date().toLocaleString('zh-CN', {timeZone:'Asia/Tokyo',hour12:false})}（Asia/Tokyo）；由台账自动生成。`, '',
    `任务：${plan.tasks.length}；已验收：${done.length}；执行/评审：${active.length}；阻塞：${blocked.length}。`, '',
    `Gate通过：${plan.milestones.filter(m=>m.status==='passed').map(m=>m.id).join(', ') || '无'}。`, '',
    ...(plan.reassessment ? ['## 当前全面复评', '',
      `复评日期：${plan.reassessment.date}；覆盖：${plan.reassessment.coverage_ids.length}/${plan.tasks.length}。历史done与Gate记录保留，不代表当前源码重新通过验收。`, '',
      `报告：[当前评估与执行计划](${path.relative(path.dirname(file),path.resolve(root,plan.reassessment.report.path)).replaceAll('\\','/')})。源码与检查范围见报告；未经评审修改不自动追认完成。`, '',
      ...plan.reassessment.execution_batches.map(b=>`- ${b.title}：${b.task_ids.join(', ')}；${b.exit_criteria}。`), '',
      ...plan.tasks.map(t=>`- ${t.id} ${t.title}：${t.reeval?.verified_state ?? '未复评'}；下一步：${currentNext(t)}。`), ''] : []),
    '## 当前执行', '', ...active.map(t=>`- ${printable(t)}；下一步：${currentNext(t)}。`),
    ...(active.length?[]:['- 暂无活动任务。']), '', '## 阻塞', '',
    ...blocked.map(t=>`- ${t.id} ${t.title}：${t.blocker}；下一步：${t.next_action}。`),
    ...(blocked.length?[]:['- 暂无登记阻塞。']), '', '## 下一批可执行', '',
    ...ready.map(t=>`- ${printable(t)}`), '', '## 已完成与证据', '',
    ...done.map(t=>`- ${t.id} ${t.title}：${t.evidence.map(e=>`[证据](${path.relative(path.dirname(file),path.resolve(root,e)).replaceAll('\\','/')})`).join('；')}；${t.completion_summary}。`),
    '', 'preparing只表示候选实现；显式代码依赖可消费有独立复核及当前源码指纹的上游候选，不代表其已验收。正式review/done和Gate仍要求原验收依赖，不以实现复核替代桌面、模型或平台验收。', '', '完成数只表示任务验收，不能作为产品成熟度或发布许可。桌面、真实模型和平台验收按各自Gate记录。', ''];
  fs.writeFileSync(path.join(path.dirname(file),'progress.md'),progress.join('\n'));
}
function save() {
  validate();
  const temp = `${file}.${process.pid}.tmp`;
  fs.writeFileSync(temp, JSON.stringify(plan, null, 2) + '\n');
  fs.renameSync(temp, file);
  render();
}
try {
  validate();
  if (command === 'validate') {
    console.log(`VALID: ${plan.tasks.length} tasks, ${plan.milestones.length} milestones; dependencies and records consistent`);
    if (warnings.length) {
      const unique = [...new Set(warnings)];
      console.log(`WARNINGS (${unique.length} distinct; not blocking, fix when the task moves to review/done):`);
      for (const w of unique) console.log(`  - ${w}`);
    }
  }
  else if (command === 'render') { render(); console.log('Rendered tasks.md'); }
  else if (command === 'summary') {
    const done = plan.tasks.filter(t => t.status === 'done');
    console.log(JSON.stringify({tasks:plan.tasks.length, done:done.length, remaining:plan.tasks.length-done.length,
      ready:plan.tasks.filter(t => t.status === 'pending' && !unmet(t).length).map(t=>t.id),
      assigned:plan.tasks.filter(t=>t.owner).length, blocked:plan.tasks.filter(t=>t.status==='blocked').map(t=>({id:t.id,reason:t.blocker})),
      claimed:plan.tasks.filter(t=>t.claimed_by).map(t=>({id:t.id,by:t.claimed_by})),
      effort_days:plan.tasks.reduce((s,t)=>s+t.estimate_days,0), completed_estimated_days:done.reduce((s,t)=>s+t.estimate_days,0),
      passed_gates:plan.milestones.filter(m=>m.status==='passed').map(m=>m.id)},null,2));
  } else if (command === 'list' || command === 'ready') {
    for (const t of plan.tasks) {
      if (opts.milestone && t.milestone !== opts.milestone) continue;
      if (opts.status && t.status !== opts.status) continue;
      if (command === 'ready' && (t.status !== 'pending' || unmet(t).length)) continue;
      console.log(printable(t));
    }
  } else if (command === 'show') {
    requireValue(byId.has(positional[0]), 'Unknown task');
    const t = byId.get(positional[0]); console.log(JSON.stringify({...t,unmet:unmet(t)},null,2));
  } else if (command === 'add') {
    requireValue(opts['task-json'] && opts.note, 'Add needs --task-json and --note');
    const task = JSON.parse(fs.readFileSync(path.resolve(opts['task-json']), 'utf8'));
    requireValue(/^AZ-\d{3}$/.test(task.id) && !byId.has(task.id), 'Invalid or duplicate task ID');
    requireValue(typeof task.title === 'string' && task.title.trim(), 'Task title is required');
    requireValue(task.status === 'pending' && task.owner === null && task.reviewer === null, 'New tasks must be unassigned pending work');
    requireValue(gates.get(task.milestone)?.status === 'pending', 'Cannot add work to a passed gate');
    requireValue(Array.isArray(task.dependencies) && Array.isArray(task.finding_refs), 'New task dependencies/findings are required');
    task.history.push({at:new Date().toISOString(),note:opts.note,before:null,after:{status:'pending',owner:null,blocker:null},evidence:null});
    plan.tasks.push(task); byId.set(task.id,task);
    plan.revision = (plan.revision ?? 1) + 1;
    save(); console.log(printable(task));
  } else if (command === 'claim') {
    const t = byId.get(positional[0]); requireValue(t, 'Unknown task');
    requireValue(opts.agent && opts.note, 'claim needs --agent and --note');
    requireValue(!t.claimed_by || sameActor(t.claimed_by, opts.agent), `${t.id} already claimed by ${t.claimed_by}`);
    const previous = t.claimed_by ?? null;
    t.claimed_by = opts.agent; t.claimed_at = new Date().toISOString();
    t.history.push({at:t.claimed_at,note:opts.note,claim:{from:previous,to:opts.agent}});
    save(); console.log(`${t.id} claimed by ${t.claimed_by}`);
  } else if (command === 'release') {
    const t = byId.get(positional[0]); requireValue(t, 'Unknown task');
    requireValue(opts.agent && opts.note, 'release needs --agent and --note');
    requireValue(t.claimed_by && sameActor(t.claimed_by, opts.agent), `${t.id} is not claimed by ${opts.agent}`);
    t.history.push({at:new Date().toISOString(),note:opts.note,claim:{from:t.claimed_by,to:null}});
    t.claimed_by = null; t.claimed_at = null;
    save(); console.log(`${t.id} released`);
  } else if (command === 'reeval') {
    // Records a code-level re-evaluation for many tasks at once. Does not change status.
    requireValue(positional[0] && opts.note, 'reeval needs FILE and --note');
    const entries = JSON.parse(fs.readFileSync(path.resolve(positional[0]), 'utf8'));
    requireValue(Array.isArray(entries) && entries.length && new Set(entries.map(e => e.id)).size === entries.length, 'reeval needs a unique-id array');
    for (const entry of entries) requireValue(byId.has(entry.id), `Unknown task ${entry.id}`);
    if (opts.complete === 'true') {
      requireValue(entries.length === plan.tasks.length, 'Complete reassessment must cover every task');
      for (const entry of entries) requireValue(entry.verified_state?.trim() && entry.conclusion_proposal?.trim() && entry.next_action?.trim()
        && Array.isArray(entry.remaining_work) && Array.isArray(entry.verification_gaps), `${entry.id}: complete reassessment needs state, conclusion, next action and remaining/verification lists`);
    }
    let assessment;
    if (opts['assessment-json']) {
      requireValue(opts.complete === 'true', 'Assessment metadata requires complete task coverage');
      assessment = JSON.parse(fs.readFileSync(path.resolve(opts['assessment-json']), 'utf8'));
      requireValue(assessment.date && typeof assessment.report === 'string' && typeof assessment.source_manifest === 'string'
        && Array.isArray(assessment.execution_batches) && assessment.execution_batches.length && Array.isArray(assessment.checks), 'Assessment metadata is incomplete');
      for (const batch of assessment.execution_batches) requireValue(batch.title?.trim() && batch.exit_criteria?.trim()
        && Array.isArray(batch.task_ids) && batch.task_ids.every(id=>byId.has(id)), 'Invalid reassessment execution batch');
      const manifest = JSON.parse(fs.readFileSync(reviewPath(assessment.source_manifest), 'utf8'));
      requireValue(Array.isArray(manifest.files) && manifest.files.length, 'Assessment source manifest is empty');
      for (const source of manifest.files) requireValue(fileHash(source.path) === source.sha256, `Assessment source changed: ${source.path}`);
      assessment = {...assessment, report:{path:assessment.report,sha256:fileHash(assessment.report)},
        source_manifest:{path:assessment.source_manifest,sha256:fileHash(assessment.source_manifest)},
        entries:{path:positional[0],sha256:fileHash(positional[0])},coverage_ids:entries.map(e=>e.id)};
    }
    const at = new Date().toISOString();
    for (const entry of entries) {
      const t = byId.get(entry.id); requireValue(t, `Unknown task ${entry.id}`);
      t.reeval = {...entry, recorded_at: at};
      if (opts['apply-next'] === 'true' && t.status !== 'done') {
        requireValue(entry.next_action?.trim(), `${t.id}: missing reassessment next action`);
        t.next_action = entry.next_action;
      }
      t.history.push({at, note: opts.note, reeval: {verified_state: entry.verified_state, severity: entry.severity, conclusion_proposal: entry.conclusion_proposal}});
    }
    if (assessment) { plan.reassessment = {...assessment,recorded_at:at}; plan.revision = (plan.revision ?? 1) + 1; }
    save(); console.log(`Re-evaluation recorded for ${entries.length} tasks; statuses unchanged`);
  } else if (command === 'record-implementation-reviews') {
    requireValue(opts['reviews-json'] && opts.note, 'Batch review needs --reviews-json and --note');
    const entries = JSON.parse(fs.readFileSync(path.resolve(opts['reviews-json']), 'utf8'));
    requireValue(Array.isArray(entries) && entries.length && new Set(entries.map(e => e.id)).size === entries.length, 'Batch review needs unique task IDs');
    const prepared = entries.map(entry => {
      const t = byId.get(entry.id);
      requireValue(t && ['preparing', 'review'].includes(t.status), 'Batch review requires candidate/review tasks');
      assertClaim(t);
      requireValue(entry.reviewer && !sameActor(entry.reviewer, t.owner) && entry.independent_review === true
        && entry.scope?.trim() && entry.verification?.trim() && entry.evidence && entry.source_manifest,
      'Batch review requires a reviewer whose base name differs from the owner, scope, verification and evidence');
      const manifest = JSON.parse(fs.readFileSync(reviewPath(entry.source_manifest), 'utf8'));
      requireValue(Array.isArray(manifest.files) && manifest.files.length, 'Source manifest needs nonempty files');
      const sources = manifest.files.map(source => {
        requireValue(typeof source.path === 'string' && /^[0-9a-f]{64}$/.test(source.sha256 ?? '')
          && fileHash(source.path) === source.sha256, 'Batch source fingerprint mismatch');
        return {path:source.path,sha256:source.sha256};
      });
      return {t, review:{reviewer:entry.reviewer,independent_review:true,scope:entry.scope,verification:entry.verification,
        recorded_at:new Date().toISOString(),sources,evidence:{path:entry.evidence,sha256:fileHash(entry.evidence)},
        source_manifest:{path:entry.source_manifest,sha256:fileHash(entry.source_manifest)}}};
    });
    for (const {t, review} of prepared) {
      t.implementation_review = review;
      strictReviews.add(t.id);
      reviewCurrent(t);
      t.history.push({at:new Date().toISOString(),note:opts.note,implementation_review:review});
    }
    save(); console.log(`Implementation reviews refreshed atomically: ${prepared.map(p=>p.t.id).join(', ')}; acceptance/Gates unchanged`);
  } else if (command === 'record-implementation-review') {
    const t = byId.get(positional[0]); requireValue(t, 'Unknown task');
    assertClaim(t);
    requireValue(['preparing', 'review'].includes(t.status), 'Only preparing/review tasks can receive implementation review');
    requireValue(opts.reviewer && !sameActor(opts.reviewer, t.owner) && opts['independent-review'] === 'true'
      && opts.scope?.trim() && opts.verification?.trim() && opts.evidence && opts['source-manifest'] && opts.note,
    'Implementation review needs a reviewer whose base name differs from the owner, --independent-review true, --scope, --verification, --evidence, --source-manifest and --note');
    const manifest = JSON.parse(fs.readFileSync(reviewPath(opts['source-manifest']), 'utf8'));
    requireValue(Array.isArray(manifest.files) && manifest.files.length, 'Source manifest needs a nonempty files array');
    const sources = manifest.files.map(source => {
      requireValue(typeof source.path === 'string' && /^[0-9a-f]{64}$/.test(source.sha256 ?? ''), 'Source manifest needs path and SHA256');
      requireValue(fileHash(source.path) === source.sha256, `Source manifest fingerprint changed: ${source.path}`);
      return {path:source.path,sha256:source.sha256};
    });
    t.implementation_review = {reviewer:opts.reviewer,independent_review:true,scope:opts.scope,verification:opts.verification,
      recorded_at:new Date().toISOString(),sources,evidence:{path:opts.evidence,sha256:fileHash(opts.evidence)},
      source_manifest:{path:opts['source-manifest'],sha256:fileHash(opts['source-manifest'])}};
    strictReviews.add(t.id);
    reviewCurrent(t);
    t.history.push({at:new Date().toISOString(),note:opts.note,implementation_review:t.implementation_review});
    save(); console.log(`Implementation reviewed: ${t.id}; status ${t.status} unchanged; acceptance/Gates unchanged`);
  } else if (command === 'update') {
    const t = byId.get(positional[0]); requireValue(t, 'Unknown task');
    requireValue(opts.note, 'Every update needs --note for audit history');
    requireValue(t.status !== 'done', 'Done records are immutable; add a follow-up task for new work');
    assertClaim(t);
    const before = JSON.parse(JSON.stringify(t));
    if (opts.owner) t.owner = opts.owner;
    if (opts['code-dependencies'] !== undefined) {
      t.code_dependencies = opts['code-dependencies'].split(',').map(id=>id.trim()).filter(Boolean);
    }
    if (opts.reviewer) t.reviewer = opts.reviewer;
    if (opts.evidence) t.evidence.push(opts.evidence);
    if (opts.next) t.next_action = opts.next;
    if (opts.summary) t.completion_summary = opts.summary;
    if (opts.review) t.review_notes = opts.review;
    if (opts['actual-days']) t.actual_days = Number(opts['actual-days']);
    if (opts.gui !== undefined) {
      requireValue(GUI_STATUS.test(opts.gui), 'gui must be verified, not_required, or waived:ADR-NNN');
      t.gui_status = opts.gui;
    }
    if (opts.status) {
      requireValue(statuses.includes(opts.status), 'Invalid status');
      const transitions = {pending:['preparing','doing','blocked','deferred'], preparing:['pending','doing','review','blocked','deferred'], doing:['review','blocked','deferred'], review:['done','doing','blocked','deferred'], blocked:['pending','preparing','doing','review','deferred'], deferred:['pending']};
      requireValue(opts.status === t.status || transitions[t.status].includes(opts.status), `Illegal transition ${t.status} → ${opts.status}`);
      if (['doing','review','done'].includes(opts.status)) {
        requireValue(t.owner, 'Assign --owner before active work');
        requireValue(!unmet(t).length, `Unmet dependencies/gates: ${unmet(t).join(', ')}`);
      }
      if (['review','done'].includes(opts.status)) {
        // Moving to review/done requires a fresh, independent implementation review of THIS task only.
        strictReviews.add(t.id);
        requireValue(t.implementation_review, `${t.id}: ${opts.status} needs an implementation review recorded first`);
        reviewCurrent(t);
      }
      if (opts.status === 'done') requireValue(GUI_STATUS.test(t.gui_status ?? '') || GUI_STATUS.test(opts.gui ?? ''), `${t.id}: done needs --gui verified|not_required|waived:ADR-NNN`);
      if (opts.status === 'blocked' || opts.status === 'deferred') requireValue(opts.reason, 'Blocked/deferred needs --reason');
      t.blocker = ['blocked','deferred'].includes(opts.status) ? opts.reason : null;
      t.status = opts.status;
      if (['preparing','doing'].includes(t.status)) t.started_at ??= new Date().toISOString();
      if (t.status === 'done') t.completed_at = new Date().toISOString();
    }
    t.history.push({at:new Date().toISOString(),note:opts.note,before:{status:before.status,owner:before.owner,blocker:before.blocker},after:{status:t.status,owner:t.owner,blocker:t.blocker},evidence:opts.evidence ?? null});
    save(); console.log(printable(t));
  } else if (command === 'gate') {
    const m = gates.get(positional[0]); requireValue(m, 'Unknown milestone');
    requireValue(m.status === 'pending', 'Gate already passed');
    requireValue(opts.owner && opts.evidence && opts.note, 'Gate needs --owner --evidence --note');
    m.status = 'passed'; m.decision_owner=opts.owner; m.evidence.push(opts.evidence); m.decision_note=opts.note;
    m.history.push({at:new Date().toISOString(),decision:'passed',owner:opts.owner,note:opts.note});
    save(); console.log(`Gate passed: ${m.id}`);
  } else throw new Error('Commands: validate, render, summary, list, ready, show ID, add --task-json FILE, claim ID --agent NAME, release ID --agent NAME, reeval FILE, update ID, record-implementation-review ID, record-implementation-reviews, gate ID');
} catch (error) { console.error(error.message); process.exitCode=1; }
