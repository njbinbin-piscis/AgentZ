// Read-only inventory for AZ-063 migration planning. This is not a backup.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
let configDir;
const projectDirs = [];
while (args.length) {
  const flag = args.shift();
  const value = args.shift();
  if (!value || !path.isAbsolute(value)) throw new Error(`${flag} requires an absolute path`);
  if (flag === '--config-dir' && !configDir) configDir = path.resolve(value);
  else if (flag === '--project-dir') projectDirs.push(path.resolve(value));
  else throw new Error(`Unknown or duplicate argument: ${flag}`);
}
if (!configDir) throw new Error('Usage: node scripts/upgrade-data-inventory.mjs --config-dir ABS [--project-dir ABS ...]');

const roots = [
  { role: 'global-config', source: configDir },
  ...projectDirs.map((project, index) => ({ role: `project-${index + 1}`, source: path.join(project, '.agentz') })),
];
const normalized = roots.map(root => path.resolve(root.source).toLowerCase());
if (new Set(normalized).size !== normalized.length) throw new Error('Duplicate inventory root');
for (let i = 0; i < normalized.length; i++) {
  for (let j = i + 1; j < normalized.length; j++) {
    const relative = path.relative(normalized[i], normalized[j]);
    const reverse = path.relative(normalized[j], normalized[i]);
    const inside = value => value === '' || (value !== '..' && !value.startsWith(`..${path.sep}`) && !path.isAbsolute(value));
    if (inside(relative) || inside(reverse)) {
      throw new Error('Overlapping inventory roots');
    }
  }
}
const MAX_FILES = 50000;
const MAX_BYTES = 4 * 1024 * 1024 * 1024;
let totalFiles = 0;
let totalBytes = 0;

function fingerprint(value) {
  return crypto.createHash('sha256').update(path.resolve(value).toLowerCase()).digest('hex');
}

function hashRegularFile(file, expected) {
  const digest = crypto.createHash('sha256');
  const buffer = Buffer.allocUnsafe(1024 * 1024);
  const handle = fs.openSync(file, 'r');
  try {
    const opened = fs.fstatSync(handle);
    if (!opened.isFile() || opened.size !== expected.size || opened.ino !== expected.ino) {
      throw new Error('File changed before inventory read');
    }
    let bytes = 0;
    while (true) {
      const count = fs.readSync(handle, buffer, 0, buffer.length, null);
      if (!count) break;
      bytes += count;
      if (bytes > expected.size) throw new Error('File grew during inventory read');
      digest.update(buffer.subarray(0, count));
    }
    const finished = fs.fstatSync(handle);
    const currentPath = fs.lstatSync(file);
    if (bytes !== expected.size || finished.size !== expected.size || finished.mtimeMs !== expected.mtimeMs
      || !currentPath.isFile() || currentPath.ino !== expected.ino || currentPath.mtimeMs !== expected.mtimeMs) {
      throw new Error('File changed during inventory read');
    }
    return digest.digest('hex');
  } finally {
    fs.closeSync(handle);
  }
}

function inventoryRoot(root) {
  const info = fs.lstatSync(root.source);
  if (!info.isDirectory() || info.isSymbolicLink()) throw new Error(`${root.role} must be a real directory`);
  const files = [];
  const visit = (dir, relative) => {
    const entries = fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en'));
    for (const entry of entries) {
      const child = path.join(dir, entry.name);
      const childRelative = relative ? `${relative}/${entry.name}` : entry.name;
      const stat = fs.lstatSync(child);
      if (stat.isSymbolicLink()) throw new Error(`Symlink in ${root.role}: ${childRelative}`);
      if (stat.isDirectory()) {
        visit(child, childRelative);
      } else if (stat.isFile()) {
        totalFiles += 1;
        totalBytes += stat.size;
        if (totalFiles > MAX_FILES || totalBytes > MAX_BYTES) throw new Error('Inventory limit exceeded');
        files.push({ path: childRelative, bytes: stat.size, sha256: hashRegularFile(child, stat) });
      } else {
        throw new Error(`Unsupported file type in ${root.role}: ${childRelative}`);
      }
    }
  };
  visit(root.source, '');
  return { role: root.role, source_fingerprint: fingerprint(root.source), files };
}

const inventory = roots.map(inventoryRoot);
const globalFiles = new Set(inventory[0].files.map(file => file.path));
const sqliteSidecars = inventory.flatMap(root => root.files
  .filter(file => /\.(?:db|sqlite|sqlite3)-(?:wal|shm|journal)$/i.test(file.path))
  .map(file => ({ role: root.role, path: file.path })));
const report = {
  schema_version: 1,
  captured_at: new Date().toISOString(),
  app_version: JSON.parse(fs.readFileSync(path.join(repository, 'package.json'), 'utf8')).version,
  read_only: true,
  protected_backup: false,
  snapshot_consistent: false,
  config_key_pair_present: globalFiles.has('config.json') && globalFiles.has('.secret_key'),
  sqlite_sidecars: sqliteSidecars,
  requires_quiesced_database_copy: inventory.some(root => root.files.some(file => /\.(?:db|sqlite|sqlite3)(?:-(?:wal|shm|journal))?$/i.test(file.path))),
  total_files: totalFiles,
  total_bytes: totalBytes,
  roots: inventory,
  limits: [
    'Hashes and file lists do not preserve bytes or prove a consistent SQLite snapshot.',
    'No backup, restore, secret protection or cross-version credential validation is performed.',
    'A real backup must quiesce writers and capture matching config.json and .secret_key versions.',
  ],
};
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
