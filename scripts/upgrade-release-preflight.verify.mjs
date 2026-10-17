import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { releasePreflight, inspectRuntimeHeader } from './upgrade-release-preflight.mjs';

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'agentz-release-preflight-'));
const write = (relative, content) => {
  const file = path.join(root, relative);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, typeof content === 'object' && !Buffer.isBuffer(content) ? JSON.stringify(content) : content);
};
const pe = (machine) => {
  const bytes = Buffer.alloc(96);
  bytes.write('MZ'); bytes.writeUInt32LE(64, 60); bytes.write('PE\0\0', 64, 'binary'); bytes.writeUInt16LE(machine, 68);
  return bytes;
};
const elf = (machine) => {
  const bytes = Buffer.alloc(64); Buffer.from([0x7f, 0x45, 0x4c, 0x46]).copy(bytes); bytes[4] = 2; bytes[5] = 1; bytes.writeUInt16LE(machine, 18); return bytes;
};
const mach = (cpu) => {
  const bytes = Buffer.alloc(32); bytes.writeUInt32BE(0xcffaedfe); bytes.writeUInt32LE(cpu, 4); return bytes;
};
const config = {
  productName: 'AgentZ', mainBinaryName: 'agentz-desktop', identifier: 'com.agentz.desktop', version: '0.9.6',
  bundle: { active: true, targets: 'all', resources: {
    '../extension-host/dist/host.js': 'extension-host/host.js', 'resources/node/': 'resources/node/',
    '../bundled/preinstall/': 'preinstall/', '../bundled/codebuddy/manifest.json': 'codebuddy/manifest.json',
    '../bundled/codebuddy/exclude.json': 'codebuddy/exclude.json',
  }, icon: ['icons/icon.ico'] },
};
const seed = () => {
  write('package.json', { name: 'agentz', version: '0.9.6' });
  write('src-tauri/Cargo.toml', '[package]\nname = "agentz-desktop"\nversion = "0.9.6"\n[lib]\nname="other"\n');
  write('src-tauri/tauri.conf.json', config);
  write('src-tauri/tauri.windows.conf.json', { bundle: { targets: ['nsis'] } });
  write('extension-host/dist/host.js', 'fixture only'); write('bundled/preinstall/fixture.txt', 'fixture');
  write('bundled/codebuddy/manifest.json', {}); write('bundled/codebuddy/exclude.json', {});
  write('src-tauri/icons/icon.ico', 'fixture only'); write('src-tauri/resources/node/win32-x64/node.exe', pe(0x8664));
  write('artifacts/agentz-unsigned.exe', pe(0x8664));
};
const options = { root, platform: 'windows', arch: 'x64', channel: 'candidate', artifactDir: path.join(root, 'artifacts') };
let passed = 0;
async function check(name, fn) { await fn(); passed++; console.log(`PASS: ${name}`); }

try {
  seed();
  await check('complete unsigned fixture is candidate-input ready only', async () => {
    const result = await releasePreflight(options);
    assert.equal(result.status, 'verified'); assert.equal(result.candidate_inputs_ready, true);
    assert.equal(result.release_ready, false);
    for (const key of ['signature', 'notarization', 'installation']) assert.deepEqual(result[key], { status: 'unverified', executed: false });
    assert.match(result.artifacts[0].sha256, /^[0-9a-f]{64}$/);
    assert.equal(result.checks.find((c) => c.id === 'bundle-targets').targets.join(','), 'nsis');
  });
  await check('version disagreement fails candidate inputs', async () => {
    write('package.json', { name: 'agentz', version: '0.9.7' });
    assert.equal((await releasePreflight(options)).checks.find((c) => c.id === 'version').status, 'unverified'); seed();
  });
  await check('missing resources and removed declarations fail', async () => {
    fs.unlinkSync(path.join(root, 'extension-host/dist/host.js'));
    assert.equal((await releasePreflight(options)).status, 'missing'); seed();
    const altered = structuredClone(config); delete altered.bundle.resources['../extension-host/dist/host.js'];
    write('src-tauri/tauri.conf.json', altered);
    assert.equal((await releasePreflight(options)).checks.find((c) => c.id.startsWith('required-resource:')).status, 'missing'); seed();
  });
  await check('runtime directory name does not override wrong or unknown architecture', async () => {
    write('src-tauri/resources/node/win32-x64/node.exe', pe(0xaa64));
    assert.equal((await releasePreflight(options)).checks.find((c) => c.id === 'runtime:x64').status, 'unverified');
    write('src-tauri/resources/node/win32-x64/node.exe', 'not a binary');
    assert.equal((await releasePreflight(options)).candidate_inputs_ready, false); seed();
  });
  await check('missing, non-directory and relative artifact paths fail', async () => {
    assert.equal((await releasePreflight({ ...options, artifactDir: path.join(root, 'missing') })).status, 'missing');
    assert.equal((await releasePreflight({ ...options, artifactDir: path.join(root, 'package.json') })).status, 'missing');
    await assert.rejects(releasePreflight({ ...options, artifactDir: 'relative' }), /absolute/);
  });
  await check('escaping resource paths are rejected without reading outside fixture', async () => {
    const altered = structuredClone(config); altered.bundle.resources['../../outside.txt'] = 'outside.txt';
    write('src-tauri/tauri.conf.json', altered);
    assert.equal((await releasePreflight(options)).checks.find((c) => c.id === 'resource:../../outside.txt').status, 'missing'); seed();
  });
  await check('bad installed mapping and empty candidate artifact fail', async () => {
    const altered = structuredClone(config); altered.bundle.resources['../extension-host/dist/host.js'] = '../outside.js';
    write('src-tauri/tauri.conf.json', altered);
    assert.equal((await releasePreflight(options)).checks.find((c) => c.id === 'resource-mapping').status, 'unverified'); seed();
    write('artifacts/agentz-unsigned.exe', '');
    assert.equal((await releasePreflight(options)).candidate_inputs_ready, false); seed();
  });
  await check('ELF and Mach-O architectures and truncated headers classify accurately', async () => {
    assert.deepEqual(inspectRuntimeHeader(elf(62)).architectures, ['x64']);
    assert.deepEqual(inspectRuntimeHeader(elf(183)).architectures, ['arm64']);
    assert.deepEqual(inspectRuntimeHeader(mach(0x01000007)).architectures, ['x64']);
    assert.deepEqual(inspectRuntimeHeader(mach(0x0100000c)).architectures, ['arm64']);
    assert.equal(inspectRuntimeHeader(pe(0x8664).subarray(0, 65)).status, 'unverified');
    const fat = Buffer.alloc(48); fat.writeUInt32BE(0xcafebabe); fat.writeUInt32BE(2, 4);
    fat.writeUInt32BE(0x01000007, 8); fat.writeUInt32BE(0x0100000c, 28);
    assert.deepEqual(inspectRuntimeHeader(fat).architectures, ['x64', 'arm64']);
    assert.equal(inspectRuntimeHeader(fat.subarray(0, 16)).status, 'unverified');
  });
  await check('universal candidate requires both runtime architectures', async () => {
    write('src-tauri/resources/node/darwin-arm64/node', mach(0x0100000c));
    write('artifacts/AgentZ.dmg', 'fixture image'); write('artifacts/AgentZ.app/Contents/MacOS/agentz-desktop', mach(0x0100000c));
    const universal = { ...options, platform: 'macos', arch: 'universal' };
    assert.equal((await releasePreflight(universal)).checks.find((c) => c.id === 'runtime:x64').status, 'missing');
    write('src-tauri/resources/node/darwin-x64/node', mach(0x01000007));
    assert.equal((await releasePreflight(universal)).candidate_inputs_ready, true);
  });
  await check('channel and platform combinations reject without side effects', async () => {
    await assert.rejects(releasePreflight({ ...options, channel: 'stable' }), /candidate/);
    await assert.rejects(releasePreflight({ ...options, arch: 'universal' }), /combination/);
    const script = path.join(path.dirname(fileURLToPath(import.meta.url)), 'upgrade-release-preflight.mjs');
    const result = spawnSync(process.execPath, [script, '--platform', 'windows', '--arch', 'x64', '--channel', 'candidate', '--artifact-dir', 'relative'], { encoding: 'utf8' });
    assert.equal(result.status, 2); assert.match(result.stderr, /absolute/);
  });
  console.log(`PASS: ${passed} offline fixture groups; no real bundle, signing, install or release acceptance`);
} finally {
  // Only the UUID-like mkdtemp directory owned by this test is removed.
  assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
  assert.ok(path.basename(root).startsWith('agentz-release-preflight-'));
  fs.rmSync(root, { recursive: true, force: true });
}
