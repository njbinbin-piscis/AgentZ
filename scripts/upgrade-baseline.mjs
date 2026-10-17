// Reproduce Git semantics underlying the reviewed issues, using only a private fixture.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'agentz-baseline-'));
function git(dir, ...args) {
  const result = spawnSync('git', ['-c','user.name=AgentZFixture','-c','user.email=fixture@example.invalid',...args], {cwd:dir,encoding:'utf8'});
  assert.equal(result.status,0,`${args.join(' ')}: ${result.stderr}`);
  return result.stdout.trim();
}
function repo(name) {
  const dir=path.join(temp,name); fs.mkdirSync(dir); git(dir,'init','-q','-b','main'); return dir;
}
const results=[];
const changes=repo('review');
fs.writeFileSync(path.join(changes,'tracked.txt'),'base\n');
git(changes,'add','tracked.txt'); git(changes,'commit','-qm','base');
git(changes,'switch','-qc','workz/task-a');
fs.writeFileSync(path.join(changes,'tracked.txt'),'modified\n');
fs.writeFileSync(path.join(changes,'new.txt'),'new\n');
const branchDiff=git(changes,'diff','--name-status','main...workz/task-a');
const status=git(changes,'status','--short');
assert.equal(branchDiff,''); assert.match(status,/tracked\.txt/); assert.match(status,/new\.txt/);
results.push({id:'F02',kind:'git_reproduction',result:'reproduced',expected:'review shows both pending files',actual:{branch_diff:branchDiff,working_status:status}});
const fresh=repo('initial-commit');
fs.writeFileSync(path.join(fresh,'user-staged.txt'),'user staging\n');
git(fresh,'add','user-staged.txt'); git(fresh,'commit','-qm','Initial commit','--allow-empty');
const tree=git(fresh,'ls-tree','--name-only','HEAD'); assert.equal(tree,'user-staged.txt');
results.push({id:'F05-staging',kind:'git_reproduction',result:'reproduced',expected:'creating isolation does not commit user staging',actual:{initial_commit_files:tree}});
const dirty=repo('dirty-start');
fs.writeFileSync(path.join(dirty,'code.txt'),'committed\n'); git(dirty,'add','code.txt'); git(dirty,'commit','-qm','base');
fs.writeFileSync(path.join(dirty,'code.txt'),'local unsaved-to-git edit\n');
const worktree=path.join(temp,'fork'); git(dirty,'worktree','add','-b','workz/task-dirty',worktree,'main');
const forkContent=fs.readFileSync(path.join(worktree,'code.txt'),'utf8'); assert.equal(forkContent.replace(/\r\n/g,'\n'),'committed\n');
results.push({id:'F05-local-start',kind:'git_reproduction',result:'reproduced',expected:'chosen starting-state semantics are explicit',actual:{main_content:fs.readFileSync(path.join(dirty,'code.txt'),'utf8'),worktree_content:forkContent}});
results.push({id:'F01',kind:'source_trace',result:'confirmed_in_source_not_desktop_reproduced',files:['src/workspaces/workz/index.tsx: worktree state/ref, runRef and openTask'],note:'openTask does not restore task worktree; runRef takes current shared worktreeRef'});
results.push({id:'F03',kind:'source_trace',result:'confirmed_in_source_not_desktop_reproduced',files:['src-tauri/src/commands/chat.rs: journal uses agent_workspace','src/workspaces/workz/index.tsx: journalListChanges uses projectDir','src-tauri/src/commands/journal.rs: opens journal in supplied project'],note:'isolated run journal write root differs from review read root'});
const record={at:new Date().toISOString(),baseline:git(root,'rev-parse','HEAD'),version:JSON.parse(fs.readFileSync(path.join(root,'package.json'),'utf8')).version,fixture:temp,method:'Private Git fixtures plus separately classified source traces; not a desktop E2E or model benchmark',results};
const outputIndex=process.argv.indexOf('--output');
if(outputIndex>=0){const output=path.resolve(process.argv[outputIndex+1]);fs.mkdirSync(path.dirname(output),{recursive:true});fs.writeFileSync(output,JSON.stringify(record,null,2)+'\n');}
console.log(JSON.stringify(record,null,2));
