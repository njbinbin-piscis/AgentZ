// Reconstruct a candidate from an immutable baseline, never from dirty source.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const source = path.resolve(process.argv[2] ?? path.join(root, '../piscis-engine'));
const revision = '527e55496caf4437e4265e02869f0a8c764549f6';
const patches = ['AZ-029-v0.8.68.patch', 'AZ-079-v0.8.68.patch', 'AZ-081-v0.8.68.patch', 'AZ-082-v0.8.68.patch', 'AZ-030-v0.8.68.patch', 'AZ-023-error-class-v0.8.68.patch', 'AZ-023-loop-v0.8.68.patch', 'AZ-023-http-fixture-v0.8.68.patch'];
const output = path.join(root, '.cache', `kernel-candidate-${randomUUID()}`);
fs.mkdirSync(output, { recursive: true });
function git(cwd, ...args) {
  return execFileSync('git', ['-C', cwd, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}
git(source, '-c', 'core.autocrlf=false', 'archive', '--format=tar', `--output=${path.join(output, 'baseline.tar')}`, revision);
const checkout = path.join(output, 'source');
fs.mkdirSync(checkout);
execFileSync('tar', ['-xf', path.join(output, 'baseline.tar'), '-C', checkout]);
// An independent repository is essential: otherwise git apply may skip paths.
git(checkout, 'init', '--quiet');
// Keep immutable archive/patch bytes independent of a host's global checkout policy.
git(checkout, 'config', 'core.autocrlf', 'false');
const manifest = { revision, patches: [] };
for (const name of patches) {
  const file = path.join(root, 'docs/upgrade/patches', name);
  git(checkout, 'apply', '--check', file);
  git(checkout, 'apply', file);
  git(checkout, 'apply', '--reverse', '--check', file);
  manifest.patches.push({ name, sha256: createHash('sha256').update(fs.readFileSync(file)).digest('hex') });
}
const members = ['piscis-core', 'piscis-kernel', 'piscis-ide-tools'];
// Cargo path patches nested under AgentZ otherwise inherit its workspace.
for (const name of members) {
  const file = path.join(checkout, name, 'Cargo.toml');
  const content = fs.readFileSync(file, 'utf8');
  if (!content.startsWith('[package]')) throw new Error(`Unexpected package manifest: ${name}`);
  fs.writeFileSync(file, content.replace('[package]', '[package]\nworkspace = ".."'));
}
manifest.integration = 'Explicit package.workspace = ".." for three patched engine crates';
const config = '[patch."https://github.com/njbinbin-piscis/piscis-engine.git"]\n'
  + members.map(name => `${name} = { path = ${JSON.stringify(path.join(checkout, name).replaceAll('\\', '/'))} }`).join('\n') + '\n';
fs.writeFileSync(path.join(output, 'cargo-config.toml'), config);
// Bundle the reconstructed source, excluding temporary repository metadata.
// Consumers can extract this artifact without access to the sibling checkout.
const archive = path.join(output, 'candidate.tar');
execFileSync('tar', ['--exclude=.git', '-cf', archive, '-C', checkout, '.']);
manifest.archive = {
  name: 'candidate.tar',
  sha256: createHash('sha256').update(fs.readFileSync(archive)).digest('hex'),
};
fs.writeFileSync(path.join(output, 'manifest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(JSON.stringify({ output, config: path.join(output, 'cargo-config.toml'), ...manifest }, null, 2));
