import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const script = fileURLToPath(new URL('./upgrade-data-backup.mjs', import.meta.url));
const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'agentz-protected-backup-'));
const passphrase = 'synthetic-fixture-passphrase\n';
const run = (args, input = passphrase) => spawnSync(process.execPath, [script, ...args], { encoding: 'utf8', input });
// Hash a whole bundle tree so a failed restore can be proven to leave the backup untouched.
function treeHash(dir) {
  const hash = crypto.createHash('sha256');
  const walk = current => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const full = path.join(current, entry.name);
      hash.update(path.relative(dir, full));
      if (entry.isDirectory()) walk(full);
      else hash.update(fs.readFileSync(full));
    }
  };
  walk(dir);
  return hash.digest('hex');
}
// Copy a bundle and re-encrypt its manifest and/or header with mutated fields (synthetic version fixtures).
function forgeBundle(source, target, { header: headerMutation = {}, manifest: manifestMutation = {}, transformManifest } = {}) {
  fs.cpSync(source, target, { recursive: true });
  const headerPath = path.join(target, 'header.json');
  const header = JSON.parse(fs.readFileSync(headerPath, 'utf8'));
  const key = crypto.scryptSync(Buffer.from(passphrase), Buffer.from(header.salt, 'hex'), 32, { N: 32768, r: 8, p: 1, maxmem: 64 * 1024 * 1024 });
  const manifestPath = path.join(target, 'manifest.bin');
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, Buffer.from(header.manifest_iv, 'hex'));
  decipher.setAAD(Buffer.from('agentz-manifest-v1'));
  decipher.setAuthTag(Buffer.from(header.manifest_tag, 'hex'));
  const manifest = JSON.parse(Buffer.concat([decipher.update(fs.readFileSync(manifestPath)), decipher.final()]).toString('utf8'));
  Object.assign(manifest, manifestMutation);
  if (transformManifest) transformManifest(manifest);
  const iv = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  cipher.setAAD(Buffer.from('agentz-manifest-v1'));
  fs.writeFileSync(manifestPath, Buffer.concat([cipher.update(JSON.stringify(manifest)), cipher.final()]));
  Object.assign(header, { manifest_iv: iv.toString('hex'), manifest_tag: cipher.getAuthTag().toString('hex') }, headerMutation);
  fs.writeFileSync(headerPath, JSON.stringify(header));
  key.fill(0);
  return target;
}
try {
  const config = path.join(fixture, 'config');
  const project = path.join(fixture, 'project');
  const bundle = path.join(fixture, 'bundle');
  const restored = path.join(fixture, 'restored');
  fs.mkdirSync(config);
  fs.mkdirSync(path.join(project, '.agentz'), { recursive: true });
  fs.writeFileSync(path.join(config, 'config.json'), '{"api_key":"secret-marker-config"}');
  fs.writeFileSync(path.join(config, '.secret_key'), 'secret-marker-key');
  fs.writeFileSync(path.join(project, '.agentz', 'runs.db'), 'secret-marker-run');
  const nested = run(['create', '--config-dir', config, '--project-dir', project, '--output', path.join(config, 'nested-bundle'), '--quiesced', 'true']);
  assert.notEqual(nested.status, 0);
  assert.equal(fs.existsSync(path.join(config, 'nested-bundle')), false);
  const configAlias = path.join(fixture, 'config-alias');
  fs.symlinkSync(config, configAlias, process.platform === 'win32' ? 'junction' : 'dir');
  const aliasedCreate = run(['create', '--config-dir', config, '--project-dir', project, '--output', path.join(configAlias, 'aliased-bundle'), '--quiesced', 'true']);
  assert.notEqual(aliasedCreate.status, 0, 'aliased source parent must not receive a bundle');
  assert.equal(fs.existsSync(path.join(config, 'aliased-bundle')), false);
  const occupiedBundle = path.join(fixture, 'occupied-bundle');
  fs.symlinkSync(path.join(fixture, 'missing-target'), occupiedBundle, process.platform === 'win32' ? 'junction' : 'dir');
  const occupiedCreate = run(['create', '--config-dir', config, '--project-dir', project, '--output', occupiedBundle, '--quiesced', 'true']);
  assert.notEqual(occupiedCreate.status, 0, 'a dangling output link must remain occupied');
  assert.match(occupiedCreate.stderr, /Output already exists/);
  assert.ok(fs.lstatSync(occupiedBundle).isSymbolicLink());
  const create = run(['create', '--config-dir', config, '--project-dir', project, '--output', bundle, '--quiesced', 'true']);
  assert.equal(create.status, 0, create.stderr);
  assert.equal(JSON.parse(create.stdout).snapshot_consistent, false);
  assert.equal(JSON.parse(create.stdout).capture_mode, 'operator_asserted_offline');
  // Active SQLite WAL sidecar must be rejected before creating a bundle (writer quiescence enforcement).
  fs.writeFileSync(path.join(project, '.agentz', 'runs.db-wal'), 'uncheckpointed-wal');
  const walRejected = run(['create', '--config-dir', config, '--project-dir', project, '--output', path.join(fixture, 'wal-bundle'), '--quiesced', 'true']);
  assert.notEqual(walRejected.status, 0, 'active WAL sidecar must be rejected');
  assert.match(walRejected.stderr, /Active SQLite sidecar files detected/);
  fs.rmSync(path.join(project, '.agentz', 'runs.db-wal'));
  assert.ok(!fs.readFileSync(path.join(bundle, 'manifest.bin'), 'utf8').includes('secret-marker'));
  assert.equal(run(['verify', '--bundle', bundle]).status, 0);
  assert.notEqual(run(['verify', '--bundle', bundle], 'wrong-passphrase\n').status, 0);
  const oversizedSecret = run(['verify', '--bundle', bundle], 'x'.repeat(4097));
  assert.notEqual(oversizedSecret.status, 0);
  assert.match(oversizedSecret.stderr, /Passphrase must be 12\.\.4096 bytes/);
  const wrongBinding = run(['restore-preview', '--bundle', bundle, '--config-dir', project, '--project-dir', project, '--output', restored]);
  assert.notEqual(wrongBinding.status, 0);
  assert.equal(fs.existsSync(restored), false);
  const restore = run(['restore-preview', '--bundle', bundle, '--config-dir', config, '--project-dir', project, '--output', restored]);
  assert.equal(restore.status, 0, restore.stderr);
  const aliasedRestore = run(['restore-preview', '--bundle', bundle, '--config-dir', config, '--project-dir', project, '--output', path.join(configAlias, 'aliased-restore')]);
  assert.notEqual(aliasedRestore.status, 0, 'aliased source parent must not receive restored data');
  assert.equal(fs.existsSync(path.join(config, 'aliased-restore')), false);
  const occupiedRestore = run(['restore-preview', '--bundle', bundle, '--config-dir', config, '--project-dir', project, '--output', occupiedBundle]);
  assert.notEqual(occupiedRestore.status, 0, 'restore must not replace a dangling output link');
  assert.match(occupiedRestore.stderr, /Output already exists/);
  assert.ok(fs.lstatSync(occupiedBundle).isSymbolicLink());
  assert.equal(fs.readFileSync(path.join(restored, 'global-config', '.secret_key'), 'utf8'), 'secret-marker-key');
  assert.equal(fs.readFileSync(path.join(restored, 'project-1', 'runs.db'), 'utf8'), 'secret-marker-run');
  assert.notEqual(run(['restore-preview', '--bundle', bundle, '--config-dir', config, '--project-dir', project, '--output', restored]).status, 0);
  const caseBundle = path.join(fixture, 'case-collision-bundle');
  fs.cpSync(bundle, caseBundle, { recursive: true });
  const headerPath = path.join(caseBundle, 'header.json');
  const header = JSON.parse(fs.readFileSync(headerPath, 'utf8'));
  const key = crypto.scryptSync(Buffer.from(passphrase), Buffer.from(header.salt, 'hex'), 32, { N: 32768, r: 8, p: 1, maxmem: 64 * 1024 * 1024 });
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, Buffer.from(header.manifest_iv, 'hex'));
  decipher.setAAD(Buffer.from('agentz-manifest-v1'));
  decipher.setAuthTag(Buffer.from(header.manifest_tag, 'hex'));
  const manifestPath = path.join(caseBundle, 'manifest.bin');
  const encrypted = fs.readFileSync(manifestPath);
  const manifest = JSON.parse(Buffer.concat([decipher.update(encrypted), decipher.final()]).toString('utf8'));
  const original = manifest.roots[0].files.find(file => file.path === 'config.json');
  manifest.roots[0].files.push({ ...original, path: 'CONFIG.JSON', object: '999999.bin' });
  const iv = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  cipher.setAAD(Buffer.from('agentz-manifest-v1'));
  fs.writeFileSync(manifestPath, Buffer.concat([cipher.update(JSON.stringify(manifest)), cipher.final()]));
  header.manifest_iv = iv.toString('hex');
  header.manifest_tag = cipher.getAuthTag().toString('hex');
  fs.writeFileSync(headerPath, JSON.stringify(header));
  key.fill(0);
  assert.notEqual(run(['verify', '--bundle', caseBundle]).status, 0);
  const collisionPreview = path.join(fixture, 'case-collision-preview');
  assert.notEqual(run(['restore-preview', '--bundle', caseBundle, '--config-dir', config, '--project-dir', project, '--output', collisionPreview]).status, 0);
  assert.equal(fs.existsSync(collisionPreview), false);
  const ancestorBundle = forgeBundle(bundle, path.join(fixture, 'ancestor-collision-bundle'), {
    transformManifest: value => {
      const original = value.roots[0].files.find(file => file.path === 'config.json');
      value.roots[0].files.push({ ...original, path: 'CONFIG.JSON/nested', object: '999999.bin' });
    },
  });
  const ancestorVerify = run(['verify', '--bundle', ancestorBundle]);
  assert.notEqual(ancestorVerify.status, 0);
  assert.match(ancestorVerify.stderr, /Invalid manifest file/);
  const ancestorPreview = path.join(fixture, 'ancestor-collision-preview');
  assert.notEqual(run(['restore-preview', '--bundle', ancestorBundle, '--config-dir', config, '--project-dir', project, '--output', ancestorPreview]).status, 0);
  assert.equal(fs.existsSync(ancestorPreview), false);
  const extraBundle = path.join(fixture, 'extra-object-bundle');
  fs.cpSync(bundle, extraBundle, { recursive: true });
  fs.writeFileSync(path.join(extraBundle, 'objects', 'unlisted.bin'), 'unlisted marker');
  assert.notEqual(run(['verify', '--bundle', extraBundle]).status, 0);
  const extraPreview = path.join(fixture, 'extra-object-preview');
  assert.notEqual(run(['restore-preview', '--bundle', extraBundle, '--config-dir', config, '--project-dir', project, '--output', extraPreview]).status, 0);
  assert.equal(fs.existsSync(extraPreview), false);
  // Version policy: only schema 1 is readable. Older and newer versions are refused, never silently
  // migrated, and a refused restore leaves neither an output directory nor a modified backup.
  const backupBefore = treeHash(bundle);
  const versionCases = [
    ['older header schema', { header: { schema_version: 0 } }, /Unsupported bundle/],
    ['future header schema', { header: { schema_version: 2 } }, /Unsupported bundle/],
    ['older manifest schema', { manifest: { schema_version: 0 } }, /Unsupported manifest/],
    ['future manifest schema', { manifest: { schema_version: 2 } }, /Unsupported manifest/],
  ];
  for (const [label, mutation, reason] of versionCases) {
    const copy = forgeBundle(bundle, path.join(fixture, `version-${label.replace(/\s+/g, '-')}`), mutation);
    const verify = run(['verify', '--bundle', copy]);
    assert.notEqual(verify.status, 0, label);
    // Refused for the version policy itself, not for an incidental crypto or I/O failure.
    assert.match(verify.stderr, reason, label);
    const preview = path.join(fixture, `${path.basename(copy)}-preview`);
    const restore = run(['restore-preview', '--bundle', copy, '--config-dir', config, '--project-dir', project, '--output', preview]);
    assert.notEqual(restore.status, 0, label);
    assert.match(restore.stderr, reason, label);
    assert.equal(fs.existsSync(preview), false, label);
  }
  // Damaged header (truncated JSON) is a damaged backup: refused, nothing written.
  const damaged = path.join(fixture, 'damaged-header');
  fs.cpSync(bundle, damaged, { recursive: true });
  fs.writeFileSync(path.join(damaged, 'header.json'), '{"schema_version":1,');
  assert.notEqual(run(['verify', '--bundle', damaged]).status, 0);
  const damagedPreview = path.join(fixture, 'damaged-header-preview');
  assert.notEqual(run(['restore-preview', '--bundle', damaged, '--config-dir', config, '--project-dir', project, '--output', damagedPreview]).status, 0);
  assert.equal(fs.existsSync(damagedPreview), false);
  // The original backup is unchanged by every refused or damaged input above.
  assert.equal(treeHash(bundle), backupBefore, 'backup must not be modified by refused restores');
  const object = fs.readdirSync(path.join(bundle, 'objects'))[0];
  fs.appendFileSync(path.join(bundle, 'objects', object), 'tamper');
  assert.notEqual(run(['verify', '--bundle', bundle]).status, 0);
  console.log('upgrade-data-backup: synthetic encrypted create/verify/restore-preview and failure cases passed');
} finally {
  fs.rmSync(fixture, { recursive: true, force: true });
}
