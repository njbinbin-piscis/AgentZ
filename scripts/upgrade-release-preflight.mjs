// Offline candidate inspection only: no builds, downloads, signing or publishing.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const platforms = new Set(['windows', 'macos', 'linux']);
const arches = new Set(['x64', 'arm64', 'universal']);
const MAX_ENTRIES = 10000;

function contained(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative));
}

function safePath(root, candidate) {
  const absolute = path.resolve(candidate);
  if (!contained(root, absolute)) throw new Error('path escapes inspection root');
  const parts = path.relative(root, absolute).split(path.sep).filter(Boolean);
  let current = root;
  for (const part of ['', ...parts]) {
    if (part) current = path.join(current, part);
    if (fs.existsSync(current) && fs.lstatSync(current).isSymbolicLink()) throw new Error('symbolic links are not inspected');
  }
  return absolute;
}

function jsonFile(root, relative) {
  return JSON.parse(fs.readFileSync(safePath(root, path.join(root, relative)), 'utf8'));
}

function mergeConfig(base, overlay) {
  const result = { ...base };
  for (const [key, value] of Object.entries(overlay)) {
    result[key] = value && typeof value === 'object' && !Array.isArray(value)
      ? mergeConfig(base[key] ?? {}, value) : value;
  }
  return result;
}

function inventory(root, directory, count = { value: 0 }) {
  const entries = [];
  const walk = (current) => {
    safePath(root, current);
    const stat = fs.lstatSync(current);
    if (++count.value > MAX_ENTRIES) throw new Error('inspection entry limit exceeded');
    if (stat.isFile()) { entries.push(current); return; }
    if (!stat.isDirectory()) throw new Error('unsupported filesystem entry');
    for (const name of fs.readdirSync(current).sort()) walk(path.join(current, name));
  };
  walk(directory);
  return entries;
}

export function inspectRuntimeHeader(bytes) {
  const result = (format, architectures) => ({ status: architectures.length ? 'verified' : 'unverified', format, architectures,
    scope: 'header architecture only; execution, provenance and signature unverified' });
  if (bytes.length >= 64 && bytes[0] === 0x4d && bytes[1] === 0x5a) {
    const pe = bytes.readUInt32LE(60);
    if (pe <= bytes.length - 6 && bytes.subarray(pe, pe + 4).equals(Buffer.from([0x50, 0x45, 0, 0]))) {
      return result('PE', ({ 0x8664: ['x64'], 0xaa64: ['arm64'], 0x014c: ['ia32'] })[bytes.readUInt16LE(pe + 4)] ?? []);
    }
    return result('PE', []);
  }
  if (bytes.length >= 20 && bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46]))) {
    if (bytes[4] !== 2 || ![1, 2].includes(bytes[5])) return result('ELF', []);
    const machine = bytes[5] === 1 ? bytes.readUInt16LE(18) : bytes.readUInt16BE(18);
    return result('ELF', ({ 62: ['x64'], 183: ['arm64'] })[machine] ?? []);
  }
  if (bytes.length >= 8) {
    const magic = bytes.readUInt32BE(0);
    const cpuArch = (cpu) => ({ 0x01000007: 'x64', 0x0100000c: 'arm64' })[cpu];
    if ([0xfeedfacf, 0xcffaedfe].includes(magic)) {
      const arch = cpuArch(magic === 0xcffaedfe ? bytes.readUInt32LE(4) : bytes.readUInt32BE(4));
      return result('MachO', arch ? [arch] : []);
    }
    if ([0xcafebabe, 0xbebafeca, 0xcafebabf, 0xbfbafeca].includes(magic)) {
      const little = [0xbebafeca, 0xbfbafeca].includes(magic);
      const read = (offset) => little ? bytes.readUInt32LE(offset) : bytes.readUInt32BE(offset);
      const count = read(4);
      const stride = [0xcafebabf, 0xbfbafeca].includes(magic) ? 32 : 20;
      if (count < 1 || count > 32 || 8 + count * stride > bytes.length) return result('MachO-fat', []);
      const found = [];
      for (let index = 0; index < count; index++) {
        const arch = cpuArch(read(8 + index * stride));
        if (!arch) return result('MachO-fat', []);
        found.push(arch);
      }
      return result('MachO-fat', [...new Set(found)]);
    }
  }
  return result('unknown', []);
}

async function sha256(file) {
  const digest = crypto.createHash('sha256');
  for await (const chunk of fs.createReadStream(file)) digest.update(chunk);
  return digest.digest('hex');
}

export async function releasePreflight({ root = repository, platform, arch, channel, artifactDir }) {
  if (!platforms.has(platform) || !arches.has(arch) || (arch === 'universal' && platform !== 'macos')) throw new Error('invalid platform/architecture combination');
  if (channel !== 'candidate') throw new Error('only explicit candidate channel is supported');
  if (typeof artifactDir !== 'string' || !path.isAbsolute(artifactDir)) throw new Error('artifact directory must be absolute');
  root = path.resolve(root);
  const checks = [];
  const add = (id, status, details) => checks.push({ id, status, ...details });
  let config;
  try {
    const pkg = jsonFile(root, 'package.json');
    const cargo = fs.readFileSync(safePath(root, path.join(root, 'src-tauri/Cargo.toml')), 'utf8');
    const section = cargo.match(/^\[package\]\s*\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m)?.[1] ?? '';
    const cargoVersion = section.match(/^version\s*=\s*"([^"\n]+)"/m)?.[1];
    const cargoName = section.match(/^name\s*=\s*"([^"\n]+)"/m)?.[1];
    config = jsonFile(root, 'src-tauri/tauri.conf.json');
    const overlay = `src-tauri/tauri.${platform}.conf.json`;
    if (fs.existsSync(path.join(root, overlay))) {
      config = mergeConfig(config, jsonFile(root, overlay));
      add('platform-overlay', 'verified', { path: overlay });
    } else add('platform-overlay', platform === 'windows' ? 'missing' : 'verified', { path: overlay, inherited: true });
    const versions = { package: pkg.version, cargo: cargoVersion, tauri: config.version };
    const sameVersion = typeof pkg.version === 'string' && /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(pkg.version)
      && cargoVersion === pkg.version && config.version === pkg.version;
    add('version', sameVersion ? 'verified' : 'unverified', { versions });
    add('identity', pkg.name === 'agentz' && cargoName === 'agentz-desktop' && config.mainBinaryName === cargoName
      && config.identifier === 'com.agentz.desktop' && config.productName === 'AgentZ' ? 'verified' : 'unverified',
    { package_name: pkg.name, cargo_name: cargoName, binary: config.mainBinaryName, identifier: config.identifier, product_name: config.productName });
  } catch (error) { add('configuration', 'missing', { reason: error.message }); }

  const formats = { windows: ['nsis'], macos: ['app', 'dmg'], linux: ['deb', 'appimage'] }[platform];
  let targets = [];
  if (config) {
    const configured = config.bundle?.targets;
    targets = configured === 'all' ? formats : Array.isArray(configured) ? configured : typeof configured === 'string' ? [configured] : [];
    add('bundle-targets', config.bundle?.active === true && targets.length > 0 && targets.every((target) => formats.includes(target)) ? 'verified' : 'unverified', { configured, targets });
    const resources = config.bundle?.resources;
    const sources = Array.isArray(resources) ? resources : resources && typeof resources === 'object' ? Object.keys(resources) : [];
    if (!sources.length) add('resources', 'missing', { reason: 'no bundle resources configured' });
    for (const required of ['../extension-host/dist/host.js', 'resources/node/', '../bundled/preinstall/', '../bundled/codebuddy/manifest.json', '../bundled/codebuddy/exclude.json']) {
      if (!sources.includes(required)) add(`required-resource:${required}`, 'missing', { reason: 'required source is not declared in bundle resources' });
    }
    const expectedMapping = { '../extension-host/dist/host.js': 'extension-host/host.js', 'resources/node/': 'resources/node',
      '../bundled/preinstall/': 'preinstall', '../bundled/codebuddy/manifest.json': 'codebuddy/manifest.json', '../bundled/codebuddy/exclude.json': 'codebuddy/exclude.json' };
    if (Array.isArray(resources)) add('resource-mapping', 'unverified', { reason: 'implicit resource mapping cannot establish required installed paths' });
    else if (resources && typeof resources === 'object') {
      const destinationsValid = Object.values(resources).every((value) => typeof value === 'string' && value.length > 0 && !/^[\\/]|^[A-Za-z]:/.test(value) && !value.replaceAll('\\', '/').split('/').includes('..'));
      const requiredMapping = Object.entries(expectedMapping).every(([source, destination]) => resources[source]?.replaceAll?.('\\', '/').replace(/\/$/, '') === destination);
      add('resource-mapping', destinationsValid && requiredMapping ? 'verified' : 'unverified', { scope: 'configured destinations only; installed contents unverified' });
    }
    for (const source of [...sources, ...(config.bundle?.icon ?? [])]) {
      try {
        if (typeof source !== 'string' || /[*?\[\]]/.test(source)) throw new Error('resource glob or invalid path requires explicit inventory');
        const resolved = safePath(root, path.resolve(root, 'src-tauri', source));
        const files = inventory(root, resolved);
        add(`resource:${source}`, files.length ? 'verified' : 'missing', { files: files.length, scope: 'source files exist; packaged copy and content semantics unverified' });
      } catch (error) { add(`resource:${source}`, 'missing', { reason: error.message }); }
    }
  }
  const runtimePlatform = { windows: 'win32', macos: 'darwin', linux: 'linux' }[platform];
  for (const expected of arch === 'universal' ? ['x64', 'arm64'] : [arch]) {
    const relative = `src-tauri/resources/node/${runtimePlatform}-${expected}/${platform === 'windows' ? 'node.exe' : 'node'}`;
    let fd;
    try {
      const file = safePath(root, path.join(root, relative));
      fd = fs.openSync(file, 'r');
      const bytes = Buffer.alloc(65536);
      const length = fs.readSync(fd, bytes, 0, bytes.length, 0);
      const header = inspectRuntimeHeader(bytes.subarray(0, length));
      const expectedFormat = { windows: 'PE', macos: 'MachO', linux: 'ELF' }[platform];
      const compatible = header.architectures.includes(expected) && (header.format === expectedFormat || platform === 'macos' && header.format === 'MachO-fat');
      add(`runtime:${expected}`, header.status === 'verified' && compatible ? 'verified' : 'unverified', { path: relative, expected, header });
    } catch (error) { add(`runtime:${expected}`, 'missing', { path: relative, reason: error.message }); }
    finally { if (fd !== undefined) fs.closeSync(fd); }
  }

  const artifacts = [];
  try {
    const directory = safePath(path.resolve(artifactDir), artifactDir);
    if (!fs.statSync(directory).isDirectory()) throw new Error('artifact path is not a directory');
    const files = inventory(directory, directory);
    const detected = new Set();
    const extension = { nsis: '.exe', dmg: '.dmg', deb: '.deb', appimage: '.appimage' };
    for (const file of files) {
      const relative = path.relative(directory, file).split(path.sep).join('/');
      const target = targets.find((value) => value === 'app' ? relative.split('/').some((part) => part.endsWith('.app')) : relative.toLowerCase().endsWith(extension[value] ?? '\0'));
      if (!target) continue;
      if (fs.statSync(file).size === 0) throw new Error('empty candidate artifact');
      detected.add(target);
      artifacts.push({ path: relative, target, bytes: fs.statSync(file).size, sha256: await sha256(file),
        status: 'verified', scope: 'bytes hashed only; bundle format, architecture, provenance and signature unverified' });
    }
    for (const target of targets) add(`artifact:${target}`, detected.has(target) ? 'verified' : 'missing', { scope: 'matching candidate files only; installability unverified' });
    if (!artifacts.length) add('artifacts', 'missing', { reason: 'no matching candidate files' });
  } catch (error) { add('artifact-directory', 'missing', { reason: error.message }); }
  const status = checks.some((check) => check.status === 'missing') ? 'missing' : checks.some((check) => check.status === 'unverified') ? 'unverified' : 'verified';
  return { schema_version: 1, purpose: 'offline candidate source/resources inspection', platform, arch, channel,
    artifact_directory: artifactDir, status, candidate_inputs_ready: status === 'verified', release_ready: false,
    checks, artifacts, signature: { status: 'unverified', executed: false }, notarization: { status: 'unverified', executed: false },
    installation: { status: 'unverified', executed: false }, limits: ['No build, network, signing, installation or publishing performed',
      'Source-resource presence and runtime headers do not prove packaged contents or runtime execution', 'SHA256 measures inspected bytes; no trusted expected digest or signature comparison'] };
}

function argumentsOf(args) {
  const allowed = new Set(['platform', 'arch', 'channel', 'artifact-dir']);
  const options = {};
  for (let index = 0; index < args.length; index += 2) {
    const key = args[index]?.replace(/^--/, '');
    if (!args[index]?.startsWith('--') || !allowed.has(key) || Object.hasOwn(options, key) || !args[index + 1] || args[index + 1].startsWith('--')) throw new Error('expected unique --platform --arch --channel --artifact-dir arguments');
    options[key] = args[index + 1];
  }
  return { platform: options.platform, arch: options.arch, channel: options.channel, artifactDir: options['artifact-dir'] };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const manifest = await releasePreflight(argumentsOf(process.argv.slice(2)));
    console.log(JSON.stringify(manifest, null, 2));
    process.exitCode = manifest.candidate_inputs_ready ? 0 : 1;
  } catch (error) { console.error(`preflight rejected: ${error.message}`); process.exitCode = 2; }
}
