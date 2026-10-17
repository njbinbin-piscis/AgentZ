// Freeze the complete product vendor as a reviewable overlay; never publish it.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const vendor = path.join(root, 'vendor/piscis-engine');
const companion = path.join(root, 'vendor/secret-service');
const baselineProvider = path.resolve(process.argv[2] ?? path.join(root, '../piscis-engine'));
const revision = '527e55496caf4437e4265e02869f0a8c764549f6';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
let phase = 'inventory';
function run(program, args, cwd) {
  return execFileSync(program, args, { cwd, stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 * 1024 * 1024 });
}
function git(directory, ...args) { return run('git', ['-C', directory, ...args]); }
function inventory(directory) {
  const files = [];
  function visit(current, relative = '') {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      if (entry.name === '.git') continue;
      if (['target', 'node_modules', '.agentz', '.piscis', '.env'].includes(entry.name)) {
        throw new Error('Source snapshot contains a forbidden runtime/build entry');
      }
      const name = relative ? `${relative}/${entry.name}` : entry.name;
      const file = path.join(current, entry.name);
      if (entry.isSymbolicLink()) throw new Error('Source snapshot contains a link');
      if (entry.isDirectory()) visit(file, name);
      else if (entry.isFile()) files.push({ path: name, sha256: sha(fs.readFileSync(file)) });
      else throw new Error('Source snapshot contains a non-regular entry');
    }
  }
  visit(directory);
  return files.sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
}
function equal(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) throw new Error(`${label} file-set/byte mismatch`);
}
function repository(directory) {
  git(directory, 'init', '--quiet');
  git(directory, 'config', 'core.autocrlf', 'false');
  git(directory, 'config', 'core.filemode', 'false');
  const hooks = path.join(directory, '.git', 'disabled-hooks');
  fs.mkdirSync(hooks);
  git(directory, 'config', 'core.hooksPath', hooks);
}
function replaceSource(directory, files, source) {
  const retained = new Set(files.map(f => f.path));
  // Only individually inventoried regular files inside this fresh temporary tree.
  for (const file of inventory(directory)) {
    if (!retained.has(file.path)) fs.unlinkSync(path.join(directory, file.path));
  }
  for (const file of files) {
    const destination = path.join(directory, file.path);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.copyFileSync(path.join(source, file.path), destination);
  }
}

try {
  const sources = inventory(vendor);
  const companionSources = inventory(companion);
  if (!sources.length) throw new Error('Empty vendor snapshot');
  const integrationFiles = ['Cargo.toml', 'Cargo.lock', 'src-tauri/Cargo.toml', 'crates/agentz-host/Cargo.toml',
    'crates/agentz-host/tests/kernel_contract.rs', 'scripts/upgrade-kernel-freeze.mjs'];
  const integration = integrationFiles.map(name => ({ path: name, sha256: sha(fs.readFileSync(path.join(root, name))) }));
  const output = path.join(root, '.cache', `kernel-full-freeze-${crypto.randomUUID()}`);
  fs.mkdirSync(output, { recursive: true });
  const baseline = path.join(output, 'baseline.tar');
  phase = 'baseline_archive';
  git(baselineProvider, '-c', 'core.autocrlf=false', 'archive', '--format=tar', `--output=${baseline}`, revision);
  const source = path.join(output, 'source');
  fs.mkdirSync(source);
  run('tar', ['-xf', baseline, '-C', source]);
  const baselineFiles = inventory(source);
  phase = 'baseline_index';
  repository(source);
  git(source, 'add', '--force', '--all');
  git(source, '-c', 'user.name=Codex local snapshot', '-c', 'user.email=local-snapshot@invalid',
    '-c', 'commit.gpgsign=false', 'commit', '--quiet', '-m', 'Immutable upstream archive');
  replaceSource(source, sources, vendor);
  phase = 'snapshot_bytes';
  equal(inventory(source), sources, 'Frozen vendor');
  git(source, 'add', '--force', '--all');
  const patch = path.join(output, 'full-vendor.patch');
  fs.writeFileSync(patch, git(source, 'diff', '--cached', '--binary', '--full-index', 'HEAD', '--'));

  const rebuilt = path.join(output, 'reconstructed');
  phase = 'forward_apply';
  fs.mkdirSync(rebuilt);
  run('tar', ['-xf', baseline, '-C', rebuilt]);
  repository(rebuilt);
  git(rebuilt, 'apply', '--check', patch);
  git(rebuilt, 'apply', patch);
  equal(inventory(rebuilt), sources, 'Forward reconstruction');
  phase = 'reverse_apply';
  git(rebuilt, 'apply', '--reverse', '--check', patch);
  git(rebuilt, 'apply', '--reverse', patch);
  equal(inventory(rebuilt), baselineFiles, 'Reverse restoration');
  phase = 'second_forward_apply';
  git(rebuilt, 'apply', patch);
  equal(inventory(rebuilt), sources, 'Second forward reconstruction');

  const archive = path.join(output, 'candidate.tar');
  phase = 'relocated_archive';
  // Keep the existing ../../secret-service dependency resolvable after relocation.
  const bundle = path.join(output, 'bundle');
  for (const [name, files, directory] of [
    ['piscis-engine', sources, source], ['secret-service', companionSources, companion],
  ]) {
    const destination = path.join(bundle, name);
    fs.mkdirSync(destination, { recursive: true });
    replaceSource(destination, files, directory);
  }
  const bundleFiles = inventory(bundle);
  run('tar', ['-cf', archive, '-C', bundle, '.']);
  const relocated = path.join(output, 'relocated');
  fs.mkdirSync(relocated);
  run('tar', ['-xf', archive, '-C', relocated]);
  if (fs.existsSync(path.join(relocated, '.git'))) throw new Error('Archive includes repository metadata');
  equal(inventory(relocated), bundleFiles, 'Relocated archive');
  phase = 'source_stability';
  equal(inventory(vendor), sources, 'Live vendor changed during freeze');
  equal(inventory(companion), companionSources, 'Local platform dependency changed during freeze');
  for (const file of integration) {
    if (sha(fs.readFileSync(path.join(root, file.path))) !== file.sha256) throw new Error('Integration source changed during freeze');
  }
  const manifest = {
    schema_version: 1, created_at: new Date().toISOString(), baseline_revision: revision,
    baseline_archive_sha256: sha(fs.readFileSync(baseline)),
    baseline_files: baselineFiles, vendor_files: sources, integration_files: integration,
    companion: { name: 'secret-service', files: companionSources }, bundle_files: bundleFiles,
    patch: { name: 'full-vendor.patch', sha256: sha(fs.readFileSync(patch)) },
    archive: { name: 'candidate.tar', sha256: sha(fs.readFileSync(archive)) },
    checks: { forward_bytes: true, reverse_bytes: true, second_forward_bytes: true, relocated_bytes: true, snapshot_stable: true },
    compatibility_tests_run: false, all_source_reviewed: false, release_ready: false,
  };
  fs.writeFileSync(path.join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
  console.log(JSON.stringify({ output, manifest_sha256: sha(fs.readFileSync(path.join(output, 'manifest.json'))),
    source_files: sources.length, companion_files: companionSources.length,
    patch_sha256: manifest.patch.sha256, archive_sha256: manifest.archive.sha256,
    checks: manifest.checks, release_ready: false }, null, 2));
} catch {
  // Do not print raw source/patch/provider diagnostics through a release utility.
  console.error(`Kernel freeze failed at ${phase}; no release created. Inspect local inputs and temporary snapshot.`);
  process.exitCode = 1;
}
