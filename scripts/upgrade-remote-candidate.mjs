// Read-only AZ-045 review bundle. Writes only to stdout, never runs acceptance.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
if (args.length && !(args.length === 2 && args[0] === '--format' && ['json', 'markdown'].includes(args[1]))) {
  throw new Error('Usage: node scripts/upgrade-remote-candidate.mjs [--format json|markdown]');
}
const format = args[1] ?? 'json';
const sourcePaths = [
  'src-tauri/src/commands/ext_host.rs',
  'src-tauri/src/commands/chat_turn.rs',
  'src-tauri/src/commands/ide.rs',
  'src-tauri/src/commands/remote.rs',
  'src-tauri/src/commands/git_workspace.rs',
  'src-tauri/src/remote/mod.rs',
  'src-tauri/src/remote/vfs.rs',
  'src-tauri/src/remote/forward.rs',
  'src-tauri/src/remote/lsp.rs',
  'src-tauri/src/remote/mirror.rs',
  'src-tauri/src/remote/ssh_setup.rs',
  'src-tauri/src/remote/live_tests.rs',
  'src-tauri/src/tools/remote_fs.rs',
  'scripts/upgrade-remote-candidate.mjs',
  'scripts/upgrade-ssh-fixture.ps1',
];
function fingerprint(relativePath) {
  const bytes = fs.readFileSync(path.join(root, relativePath));
  return { path: relativePath, bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') };
}
const plan = JSON.parse(fs.readFileSync(path.join(root, 'docs/upgrade/plan.json'), 'utf8'));
const task = plan.tasks.find(task => task.id === 'AZ-045');
if (!task) throw new Error('AZ-045 missing from plan');
const evidencePaths = [...new Set([
  ...task.evidence,
  'docs/upgrade/evidence/AZ-045-forward-recovery.md',
  'docs/upgrade/evidence/AZ-045-final-audit.md',
  'docs/upgrade/evidence/AZ-045-isolated-ssh.md',
  'docs/upgrade/evidence/AZ-045-chat-context-binding.md',
])];
const evidence = evidencePaths.map(relativePath => {
  const source = fs.readFileSync(path.join(root, relativePath), 'utf8');
  return {
    ...fingerprint(relativePath),
    recordedStatements: source.split(/\r?\n/).filter(line => /会话\d+|passed|ignored|测试已通过|尚未|未运行/.test(line)),
  };
});
const matrix = [
  { area: 'VFS malformed URI / Unicode', status: 'historical-local-evidence', evidence: 'AZ-045-remote-recovery-candidate.md: rustc 4 tests' },
  { area: 'Bound session / replacement process zero stale frames', status: 'historical-local-evidence', evidence: 'AZ-045-remote-recovery-candidate.md: session 55905, 7 tests' },
  { area: 'Mirror / remote LSP / command boundaries', status: 'historical-local-evidence', evidence: 'AZ-045-remote-recovery-candidate.md: remote filtered test runs; source versions differ' },
  { area: 'Forward readiness / diagnostics / budgets', status: 'historical-local-evidence', evidence: 'AZ-045-remote-recovery-candidate.md: session 13793, 11 tests; later candidate changes require rerun' },
  { area: 'Forward generation lifecycle / bounded queue / failed cache', status: 'latest-source-needs-verified-run', evidence: 'AZ-045-forward-recovery.md and AZ-045-final-audit.md; this script never runs tests' },
  { area: 'Real SSH disconnect / reconnect / duplicate write', status: 'not-accepted', evidence: 'Real remote tests previously ignored; no controlled SSH failure acceptance' },
  { area: 'Changed SSH host key rejection', status: 'isolated-client-handshake-passed-product-not-accepted', evidence: 'AZ-045-isolated-ssh.md: owned localhost sshd and independent known_hosts positive/negative controls passed; authentication, execution and product entry points unaccepted' },
  { area: 'Desktop file / LSP / terminal / Agent unified state', status: 'not-accepted', evidence: 'Desktop GUI and model journey not run; user deferred GUI' },
];
const bundle = {
  schemaVersion: 1,
  capturedAt: new Date().toISOString(),
  task: { id: task.id, status: task.status, milestone: task.milestone, acceptanceCriteria: task.acceptance_criteria },
  readOnly: true,
  runsTests: false,
  acceptancePassed: false,
  note: 'Fingerprints identify current source, not tested binaries. Recorded statements are historical claims, not verification of this snapshot. No ledger or filesystem writes.',
  sources: sourcePaths.map(fingerprint),
  evidence,
  matrix,
};
if (format === 'json') {
  process.stdout.write(`${JSON.stringify(bundle, null, 2)}\n`);
} else {
  const rows = matrix.map(row => `| ${row.area} | ${row.status} | ${row.evidence} |`).join('\n');
  const hashes = bundle.sources.map(item => `- ${item.path}: ${item.sha256}`).join('\n');
  process.stdout.write(`# AZ-045 candidate snapshot\n\n${bundle.note}\n\nTask: ${task.status}; acceptance: unverified.\n\n| Area | State | Evidence scope |\n| --- | --- | --- |\n${rows}\n\n## SHA-256 source fingerprints\n\n${hashes}\n`);
}
