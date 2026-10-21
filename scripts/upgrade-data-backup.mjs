// AZ-063 offline encrypted backup candidate. Restores only into a NEW review directory.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { Transform, Writable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const inventoryScript = fileURLToPath(new URL('./upgrade-data-inventory.mjs', import.meta.url));
const [command, ...args] = process.argv.slice(2);
const options = { projects: [] };
while (args.length) {
  const flag = args.shift();
  const value = args.shift();
  if (!value) throw new Error(`Missing value for ${flag}`);
  if (flag === '--project-dir') options.projects.push(value);
  else if (flag === '--config-dir') options.config = value;
  else if (flag === '--bundle') options.bundle = value;
  else if (flag === '--output') options.output = value;
  else if (flag === '--quiesced') options.quiesced = value;
  else throw new Error(`Unknown argument: ${flag}`);
}
const absolute = value => {
  if (!value || !path.isAbsolute(value)) throw new Error('An absolute path is required');
  return path.resolve(value);
};
const hex = bytes => Buffer.from(bytes).toString('hex');
const fromHex = (value, bytes) => {
  if (typeof value !== 'string' || !new RegExp(`^[a-f0-9]{${bytes * 2}}$`).test(value)) throw new Error('Invalid bundle metadata');
  return Buffer.from(value, 'hex');
};
const pathFingerprint = value => crypto.createHash('sha256').update(path.resolve(value).toLowerCase()).digest('hex');
const isWithin = (root, candidate) => {
  const relative = path.relative(path.resolve(root).toLowerCase(), path.resolve(candidate).toLowerCase());
  return relative === '' || (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
};
// Resolve the existing parent before checking an output path. A junction/symlink
// alias must not let a new bundle or restore preview land inside protected data.
const newTarget = value => {
  const target = absolute(value);
  const parent = fs.realpathSync(path.dirname(target));
  if (!fs.statSync(parent).isDirectory()) throw new Error('Output parent is not a directory');
  return path.join(parent, path.basename(target));
};
const existingRoot = value => fs.existsSync(value) ? fs.realpathSync(value) : path.resolve(value);
const overlapsSource = (target, sources) => sources.some(source =>
  isWithin(source, target) || isWithin(existingRoot(source), target));
// existsSync follows links and reports false for a dangling symlink. Treat it
// as occupied so publication never replaces an existing directory entry.
const pathOccupied = target => {
  try { fs.lstatSync(target); return true; }
  catch (error) { if (error.code === 'ENOENT') return false; throw error; }
};
const validRelative = value => typeof value === 'string' && value.length > 0 && !value.includes('\\')
  && !value.includes('\0') && !path.posix.isAbsolute(value)
  && value.split('/').every(part => part && part !== '.' && part !== '..'
    && !/[<>:"|?*]/.test(part) && !/[. ]$/.test(part)
    && !/^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part));

async function passphraseKey(salt) {
  if (process.stdin.isTTY) throw new Error('Pipe the backup passphrase on stdin; no command-line secret argument is accepted');
  const chunks = [];
  try {
    let bytes = 0;
    for await (const input of process.stdin) {
      const chunk = Buffer.isBuffer(input) ? input : Buffer.from(input);
      bytes += chunk.length;
      if (bytes > 4096) {
        chunk.fill(0);
        throw new Error('Passphrase must be 12..4096 bytes on stdin');
      }
      chunks.push(chunk);
    }
    if (bytes < 12) throw new Error('Passphrase must be 12..4096 bytes on stdin');
    const secret = Buffer.concat(chunks);
    try {
      return crypto.scryptSync(secret, salt, 32, { N: 32768, r: 8, p: 1, maxmem: 64 * 1024 * 1024 });
    } finally {
      secret.fill(0);
    }
  } finally {
    for (const chunk of chunks) chunk.fill(0);
  }
}

function newOutputDirectory(target) {
  if (pathOccupied(target)) throw new Error('Output already exists');
  const parent = path.dirname(target);
  if (!fs.statSync(parent).isDirectory()) throw new Error('Output parent is not a directory');
  return fs.mkdtempSync(path.join(parent, '.agentz-backup-stage-'));
}

function publishDirectory(stage, target) {
  if (pathOccupied(target)) throw new Error('Output appeared during operation');
  fs.renameSync(stage, target);
}

function removeOwnedStage(stage) {
  if (stage && path.basename(stage).startsWith('.agentz-backup-stage-')) fs.rmSync(stage, { recursive: true, force: true });
}

async function encryptFile(source, target, key, role, relative, expected) {
  const before = fs.lstatSync(source);
  if (!before.isFile() || before.isSymbolicLink() || before.size !== expected.bytes) throw new Error('Inventory source changed');
  const iv = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  cipher.setAAD(Buffer.from(`${role}\0${relative}`));
  const hash = crypto.createHash('sha256');
  let bytes = 0;
  const tally = new Transform({ transform(chunk, _encoding, done) {
    bytes += chunk.length;
    hash.update(chunk);
    done(null, chunk);
  } });
  await pipeline(fs.createReadStream(source), tally, cipher, fs.createWriteStream(target, { flags: 'wx' }));
  const after = fs.lstatSync(source);
  if (!after.isFile() || after.ino !== before.ino || after.mtimeMs !== before.mtimeMs
    || bytes !== expected.bytes || hash.digest('hex') !== expected.sha256) throw new Error('Inventory source changed during encryption');
  return { iv: hex(iv), tag: hex(cipher.getAuthTag()) };
}

function decryptManifest(bundle, key, header) {
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, fromHex(header.manifest_iv, 12));
  decipher.setAAD(Buffer.from('agentz-manifest-v1'));
  decipher.setAuthTag(fromHex(header.manifest_tag, 16));
  const manifestPath = path.join(bundle, 'manifest.bin');
  const manifestInfo = fs.lstatSync(manifestPath);
  if (!manifestInfo.isFile() || manifestInfo.isSymbolicLink() || manifestInfo.size > 32 * 1024 * 1024) throw new Error('Invalid manifest object');
  const ciphertext = fs.readFileSync(manifestPath);
  if (ciphertext.length > 32 * 1024 * 1024) throw new Error('Manifest too large');
  const manifest = JSON.parse(Buffer.concat([decipher.update(ciphertext), decipher.final()]).toString('utf8'));
  if (manifest.schema_version !== 1 || !Array.isArray(manifest.roots)) throw new Error('Unsupported manifest');
  let count = 0;
  let totalBytes = 0;
  const roles = new Set();
  const objects = new Set();
  for (const root of manifest.roots) {
    if (!/^global-config$|^project-[1-9][0-9]*$/.test(root.role) || roles.has(root.role)
      || !/^[a-f0-9]{64}$/.test(root.source_fingerprint) || !Array.isArray(root.files)) throw new Error('Invalid manifest root');
    roles.add(root.role);
    // A bundle must also be safe to preview on case-insensitive filesystems.
    const names = new Set();
    const directories = new Set();
    for (const file of root.files) {
      count++;
      totalBytes += file.bytes;
      const portableName = typeof file.path === 'string' ? file.path.toLowerCase() : '';
      const parts = portableName.split('/');
      const ancestors = parts.slice(0, -1).map((_, index) => parts.slice(0, index + 1).join('/'));
      if (count > 50000 || !validRelative(file.path) || names.has(portableName)
        || directories.has(portableName) || ancestors.some(parent => names.has(parent))
        || !Number.isSafeInteger(file.bytes) || file.bytes < 0
        || totalBytes > 4 * 1024 * 1024 * 1024
        || !/^[a-f0-9]{64}$/.test(file.sha256)
        || !/^\d{6}\.bin$/.test(file.object) || objects.has(file.object)) throw new Error('Invalid manifest file');
      names.add(portableName);
      for (const parent of ancestors) directories.add(parent);
      objects.add(file.object);
      fromHex(file.iv, 12);
      fromHex(file.tag, 16);
    }
  }
  if (!roles.has('global-config')) throw new Error('Missing global config root');
  return manifest;
}

async function decryptFile(source, output, key, role, file) {
  const stat = fs.lstatSync(source);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error('Invalid encrypted object');
  const decipher = crypto.createDecipheriv('aes-256-gcm', key, fromHex(file.iv, 12));
  decipher.setAAD(Buffer.from(`${role}\0${file.path}`));
  decipher.setAuthTag(fromHex(file.tag, 16));
  const hash = crypto.createHash('sha256');
  let bytes = 0;
  const sink = output
    ? fs.createWriteStream(output, { flags: 'wx' })
    : new Writable({ write(_chunk, _encoding, done) { done(); } });
  const tally = new Transform({ transform(chunk, _encoding, done) {
    bytes += chunk.length;
    if (bytes > file.bytes) return done(new Error('Object exceeds declared size'));
    hash.update(chunk);
    done(null, chunk);
  } });
  await pipeline(fs.createReadStream(source), decipher, tally, sink);
  if (bytes !== file.bytes || hash.digest('hex') !== file.sha256) throw new Error('Object hash mismatch');
}

async function create() {
  const config = absolute(options.config);
  const projects = options.projects.map(absolute);
  const target = newTarget(options.output);
  if (overlapsSource(target, [config, ...projects.map(project => path.join(project, '.agentz'))])) {
    throw new Error('Backup output must be outside source roots');
  }
  if (options.quiesced !== 'true') throw new Error('Offline writer quiescence must be explicitly confirmed');
  const inventoryArgs = ['--config-dir', config, ...projects.flatMap(project => ['--project-dir', project])];
  const inventory = JSON.parse(execFileSync(process.execPath, [inventoryScript, ...inventoryArgs], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }));
  if (!inventory.config_key_pair_present) throw new Error('Matching config.json and .secret_key presence is required');
  if (Array.isArray(inventory.sqlite_sidecars) && inventory.sqlite_sidecars.length > 0) {
    throw new Error(`Active SQLite sidecar files detected (${inventory.sqlite_sidecars.map(s => `${s.role}:${s.path}`).join(', ')}); checkpoint and quiesce writers before creating a backup`);
  }
  const sources = [config, ...projects.map(project => path.join(project, '.agentz'))];
  const salt = crypto.randomBytes(32);
  const key = await passphraseKey(salt);
  let stage;
  try {
    stage = newOutputDirectory(target);
    fs.mkdirSync(path.join(stage, 'objects'));
    const manifest = { schema_version: 1, app_version: inventory.app_version, captured_at: new Date().toISOString(), snapshot_consistent: false, capture_mode: 'operator_asserted_offline', roots: [] };
    let objectNumber = 0;
    for (const [index, root] of inventory.roots.entries()) {
      const files = [];
      for (const file of root.files) {
        objectNumber++;
        const object = `${String(objectNumber).padStart(6, '0')}.bin`;
        const metadata = await encryptFile(path.join(sources[index], ...file.path.split('/')), path.join(stage, 'objects', object), key, root.role, file.path, file);
        files.push({ ...file, object, ...metadata });
      }
      manifest.roots.push({ role: root.role, source_fingerprint: root.source_fingerprint, files });
    }
    const afterInventory = JSON.parse(execFileSync(process.execPath, [inventoryScript, ...inventoryArgs], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 }));
    if (JSON.stringify(afterInventory.roots) !== JSON.stringify(inventory.roots)
      || !afterInventory.config_key_pair_present
      || (Array.isArray(afterInventory.sqlite_sidecars) && afterInventory.sqlite_sidecars.length > 0)) {
      throw new Error('Backup source tree changed during capture');
    }
    const iv = crypto.randomBytes(12);
    const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
    cipher.setAAD(Buffer.from('agentz-manifest-v1'));
    const plaintext = Buffer.from(JSON.stringify(manifest));
    fs.writeFileSync(path.join(stage, 'manifest.bin'), Buffer.concat([cipher.update(plaintext), cipher.final()]), { flag: 'wx' });
    fs.writeFileSync(path.join(stage, 'header.json'), JSON.stringify({ schema_version: 1, kdf: 'scrypt-N32768-r8-p1', salt: hex(salt), manifest_iv: hex(iv), manifest_tag: hex(cipher.getAuthTag()) }), { flag: 'wx' });
    publishDirectory(stage, target);
    stage = undefined;
    process.stdout.write(`${JSON.stringify({ schema_version: 1, protected_backup: true, bundle: target, files: objectNumber, snapshot_consistent: false, capture_mode: 'operator_asserted_offline' })}\n`);
  } finally {
    key.fill(0);
    removeOwnedStage(stage);
  }
}

async function inspectOrRestore(restore) {
  const bundle = absolute(options.bundle);
  let target;
  if (restore) {
    target = newTarget(options.output);
    const sources = [absolute(options.config), ...options.projects.map(project => path.join(absolute(project), '.agentz'))];
    if (overlapsSource(target, [bundle, ...sources])) throw new Error('Restore preview output must be outside bundle and source roots');
  }
  const headerInfo = fs.lstatSync(path.join(bundle, 'header.json'));
  if (!headerInfo.isFile() || headerInfo.isSymbolicLink() || headerInfo.size > 4096) throw new Error('Invalid bundle header');
  const header = JSON.parse(fs.readFileSync(path.join(bundle, 'header.json'), 'utf8'));
  if (header.schema_version !== 1 || header.kdf !== 'scrypt-N32768-r8-p1') throw new Error('Unsupported bundle');
  const key = await passphraseKey(fromHex(header.salt, 32));
  let stage;
  try {
    const manifest = decryptManifest(bundle, key, header);
    const objectDir = path.join(bundle, 'objects');
    const objectDirInfo = fs.lstatSync(objectDir);
    if (!objectDirInfo.isDirectory() || objectDirInfo.isSymbolicLink()) throw new Error('Invalid object directory');
    const expectedObjects = new Set(manifest.roots.flatMap(root => root.files.map(file => file.object)));
    const actualObjects = fs.readdirSync(objectDir, { withFileTypes: true });
    if (actualObjects.length !== expectedObjects.size || actualObjects.some(entry => !entry.isFile() || !expectedObjects.has(entry.name))) {
      throw new Error('Bundle objects do not match manifest');
    }
    if (restore) {
      const expected = [absolute(options.config), ...options.projects.map(project => path.join(absolute(project), '.agentz'))];
      if (expected.length !== manifest.roots.length || manifest.roots.some((root, index) => root.source_fingerprint !== pathFingerprint(expected[index]))) {
        throw new Error('Restore source binding does not match');
      }
      stage = newOutputDirectory(target);
    }
    for (const root of manifest.roots) {
      const rootOutput = stage && path.join(stage, root.role);
      if (rootOutput) fs.mkdirSync(rootOutput);
      for (const file of root.files) {
        const output = rootOutput && path.join(rootOutput, ...file.path.split('/'));
        if (output) fs.mkdirSync(path.dirname(output), { recursive: true });
        await decryptFile(path.join(bundle, 'objects', file.object), output, key, root.role, file);
      }
    }
    if (stage) {
      publishDirectory(stage, target);
      stage = undefined;
    }
    process.stdout.write(`${JSON.stringify({ schema_version: 1, authenticated: true, files: manifest.roots.reduce((n, root) => n + root.files.length, 0), restored_to_new_directory: restore })}\n`);
  } finally {
    key.fill(0);
    removeOwnedStage(stage);
  }
}

if (command === 'create') await create();
else if (command === 'verify') await inspectOrRestore(false);
else if (command === 'restore-preview') await inspectOrRestore(true);
else throw new Error('Usage: create|verify|restore-preview with absolute paths and passphrase on stdin');
