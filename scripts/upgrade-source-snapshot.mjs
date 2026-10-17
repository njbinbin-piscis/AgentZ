// Read-only source fingerprints. Never stages, commits, archives or reads runtime secrets.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
if (args.length && !(args.length === 2 && args[0] === '--format' && ['json', 'summary'].includes(args[1]))) throw new Error('Usage: node scripts/upgrade-source-snapshot.mjs [--format json|summary]');
const git = arguments_ => execFileSync('git', arguments_, { cwd: root, env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' }, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
const head = git(['rev-parse', '--verify', 'HEAD']).trim();
const rawStatus = git(['status', '--porcelain=v1', '-z', '--untracked-files=all']);
const indexed = git(['ls-files', '-z', '--cached', '--others', '--exclude-standard']).split('\0').filter(Boolean);
const status = [];
const parts = rawStatus.split('\0');
for (let i = 0; i < parts.length; i++) {
  if (!parts[i]) continue;
  const code = parts[i].slice(0, 2), relativePath = parts[i].slice(3);
  const priorPath = /[RC]/.test(code) ? parts[++i] : null;
  status.push({ code, path: relativePath, priorPath });
}
const manifests = new Set(['Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'src-tauri/tauri.conf.json']);
const sourceRoots = ['src/', 'src-tauri/src/', 'src-tauri/tests/', 'crates/', 'vendor/', 'extension-host/src/', 'scripts/'];
const excludedSegments = new Set(['.git', '.agentz', '.codez', '.piscis', '.cache', '.codegraph', 'node_modules', 'target', 'dist', 'build', 'out', 'logs', 'log', 'tmp', 'temp', 'artifacts', 'evidence', 'recordings', 'sessions', 'credentials', 'secrets']);
const sourceExtensions = new Set(['.rs', '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.css', '.scss', '.html', '.ps1', '.sh', '.json', '.toml', '.lock', '.tmpl']);
function policy(relativePath) {
  if (relativePath.includes('\\') || /[\x00-\x1f]/.test(relativePath) || path.isAbsolute(relativePath) || relativePath.split('/').includes('..')) return 'unsafe-path';
  const segments = relativePath.toLowerCase().split('/'), name = segments.at(-1);
  if (segments.some(segment => excludedSegments.has(segment))) return 'generated-or-runtime-directory';
  if (/^\.env(?:\.|$)|^id_(?:rsa|ecdsa|ed25519)(?:\.|$)|^(?:credentials|tokens?|auth|settings)\.(?:json|toml|ya?ml)$|(?:\.local\.|\.rs\.bk$)/.test(name)) return 'private-or-local-data';
  if (/\.(?:pem|key|p12|pfx|jks|keystore|kdbx|db|sqlite(?:3)?|log|exe|dll|msi|zip|7z|gz|xz|mp4|png|ico|icns)$/.test(name)) return 'private-data-or-artifact';
  if (manifests.has(relativePath)) return null;
  if (/(?:^|\/)(?:Cargo\.(?:toml|lock)|package(?:-lock)?\.json|tsconfig[^/]*\.json)$/.test(relativePath)) return null;
  if (/^\.github\/workflows\/[^/]+\.ya?ml$/.test(relativePath)) return null;
  if (relativePath.startsWith('docs/upgrade/contracts/') && relativePath.endsWith('.schema.json')) return null;
  if (sourceRoots.some(prefix => relativePath.startsWith(prefix)) && sourceExtensions.has(path.extname(relativePath).toLowerCase())) return null;
  if (!relativePath.includes('/') && /^(?:vite|eslint|vitest|postcss|tailwind)\.config\.[cm]?[jt]s$/.test(relativePath)) return null;
  return 'outside-build-source-allowlist';
}
const excluded = {};
const omitted = reason => { excluded[reason] = (excluded[reason] ?? 0) + 1; };
const entries = [];
let changedDuringRead = false;
for (const item of status) {
  const reason = policy(item.path);
  if (reason) { omitted(reason); continue; }
  const absolutePath = path.join(root, item.path);
  const entry = { path: item.path, git_status: item.code, tracked: item.code !== '??', sha256: null, bytes: null };
  if (item.priorPath && !policy(item.priorPath)) entry.prior_path = item.priorPath;
  if (!fs.existsSync(absolutePath)) {
    if (/[D]/.test(item.code)) { entry.state = 'deleted'; entries.push(entry); }
    else omitted('missing-during-read');
    continue;
  }
  const stat = fs.lstatSync(absolutePath);
  if (!stat.isFile() || stat.isSymbolicLink()) { omitted('non-regular-file'); continue; }
  const actual = fs.realpathSync(absolutePath);
  if (!actual.toLowerCase().startsWith(`${fs.realpathSync(root).toLowerCase()}${path.sep}`)) { omitted('outside-workspace-link'); continue; }
  if (stat.size > 2 * 1024 * 1024) { omitted('source-over-size-limit'); continue; }
  const bytes = fs.readFileSync(absolutePath);
  const after = fs.statSync(absolutePath);
  if (stat.mtimeMs !== after.mtimeMs || stat.size !== after.size) changedDuringRead = true;
  entry.bytes = bytes.length;
  entry.sha256 = crypto.createHash('sha256').update(bytes).digest('hex');
  entry.state = 'present';
  entries.push(entry);
}
entries.sort((a, b) => a.path.localeCompare(b.path));
const rootCargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
const workspaceBlock = rootCargo.match(/\[workspace\]([\s\S]*?)(?:\n\[|$)/)?.[1] ?? '';
const members = [...(workspaceBlock.match(/members\s*=\s*\[([\s\S]*?)\]/)?.[1] ?? '').matchAll(/"([^"\n]+)"/g)].map(match => match[1]);
const patches = [...rootCargo.matchAll(/\bpath\s*=\s*"([^"\n]+)"/g)].map(match => match[1]);
const rootReal = fs.realpathSync(root);
const pendingRoots = [...members, ...patches];
const discoveredRoots = new Set();
const closureRejectedCounts = {};
const rejectClosure = reason => { closureRejectedCounts[reason] = (closureRejectedCounts[reason] ?? 0) + 1; };
while (pendingRoots.length) {
  const input = pendingRoots.shift();
  if (typeof input !== 'string' || /[\\\x00-\x1f]/.test(input) || path.isAbsolute(input)) { rejectClosure('unsafe-declaration'); continue; }
  const resolved = path.resolve(root, input);
  const relativePath = path.relative(root, resolved).split(path.sep).join('/');
  if (!relativePath || relativePath.startsWith('../') || policy(`${relativePath}/Cargo.toml`) || !sourceRoots.some(prefix => `${relativePath}/`.startsWith(prefix)) && relativePath !== 'src-tauri') { rejectClosure('outside-approved-source-root'); continue; }
  if (discoveredRoots.has(relativePath)) continue;
  if (discoveredRoots.size >= 128) { rejectClosure('root-budget-exceeded'); break; }
  discoveredRoots.add(relativePath);
  const manifest = path.join(resolved, 'Cargo.toml');
  if (!fs.existsSync(manifest)) { rejectClosure('missing-manifest'); continue; }
  const stat = fs.lstatSync(manifest);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 2 * 1024 * 1024 || !fs.realpathSync(manifest).toLowerCase().startsWith(`${rootReal.toLowerCase()}${path.sep}`)) { rejectClosure('unsafe-manifest'); continue; }
  const cargo = fs.readFileSync(manifest, 'utf8');
  // Conservative inventory of literal local paths, including platform-conditioned dependencies.
  // This is deliberately not a TOML/Cargo resolver; unsupported declarations remain a review item.
  const dependencyBlocks = [...cargo.matchAll(/^(\[[^\n]+\])([^]*?)(?=^\[|(?![^]))/gm)]
    .filter(match => /(?:^|\.)(?:dependencies|dev-dependencies|build-dependencies)(?:\.|\]$)/.test(match[1].slice(1)))
    .map(match => match[2]).join('\n');
  for (const match of dependencyBlocks.matchAll(/\bpath\s*=\s*"([^"\n]+)"/g)) {
    const declaration = match[1];
    if (/[\\\x00-\x1f]/.test(declaration) || path.isAbsolute(declaration)) { rejectClosure('unsafe-declaration'); continue; }
    pendingRoots.push(path.relative(root, path.resolve(resolved, declaration)).split(path.sep).join('/'));
  }
}
const requiredRoots = [...discoveredRoots].map(relativePath => {
  if (policy(`${relativePath}/Cargo.toml`)) return { path: 'outside-approved-source-root', covered: false };
  const prefix = `${relativePath.replace(/\/$/, '')}/`;
  const all = indexed.filter(file => file.startsWith(prefix) && !policy(file));
  const changed = entries.filter(entry => entry.path.startsWith(prefix));
  return { path: relativePath, cargo_manifest_exists: fs.existsSync(path.join(root, relativePath, 'Cargo.toml')),
    nonignored_source_paths: all.length, changed_source_paths: changed.length,
    untracked_source_paths: changed.filter(entry => !entry.tracked).length,
    unchanged_tracked_paths_use_head: all.length - changed.filter(entry => entry.state !== 'deleted').length,
    covered: all.includes(`${relativePath}/Cargo.toml`) && all.some(file => file.endsWith('.rs')) };
});
const packageVersion = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).version;
const stable = !changedDuringRead && head === git(['rev-parse', '--verify', 'HEAD']).trim() && rawStatus === git(['status', '--porcelain=v1', '-z', '--untracked-files=all']);
const report = {
  schema_version: 1, captured_at: new Date().toISOString(), head, package_version: packageVersion,
  read_only: true, git_optional_locks_disabled: true, stable_during_capture: stable,
  reproducibility: 'not-established-dirty-source-is-not-head', verifies_existing_installer_source: false,
  git_status_entry_count: status.length, included_source_count: entries.length,
  excluded_counts: excluded, excluded_paths_disclosed: false, entries, required_roots: requiredRoots,
  local_path_closure_rejected_counts: closureRejectedCounts,
  limits: [
    'Git status is data, not ownership or approval. Changes may belong to the user or earlier tasks.',
    'Records only dirty tracked and nonignored untracked allowlisted sources; HEAD identifies unchanged tracked content.',
    'Hashes do not preserve source bytes or prove which snapshot produced an existing installer.',
    'Ignored runtimes, binary build resources, upstream inputs and private/local data require separate review; not read here.',
    'Source allowlist is not a secret scanner. Known private/runtime paths are excluded; source contents are never printed.',
    'Cargo member/literal-path traversal is bounded and conservative, not full TOML/Cargo resolution or source provenance approval.',
  ],
};
if (args[1] === 'summary') {
  const { entries: _entries, ...summary } = report;
  process.stdout.write(`${JSON.stringify(summary, null, 2)}\n`);
} else process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
