import assert from 'node:assert/strict';
import { readFileSync, existsSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { resolve, relative, sep } from 'node:path';
import { createHash } from 'node:crypto';

const root = resolve(import.meta.dirname, '..');
const workflow = readFileSync(resolve(root, '.github/workflows/candidate.yml'), 'utf8');
function validate(text) {
  const policy = text.split('\n').filter(line => !line.trimStart().startsWith('#')).join('\n');
  assert.match(policy, /^on:\s*\n  workflow_dispatch:\s*\n\npermissions:/m);
  assert.match(policy, /^permissions:\s*\n  contents: read\s*\n/m);
  assert.doesNotMatch(policy, /\b(?:push|pull_request|schedule|workflow_run|tags|release|publish|deployment|secrets|curl|wget)\s*:|contents:\s*write|if-no-files-found:\s*(?:warn|ignore)|tauri-action|gh\s+release|npm\s+publish|fetch-node|signing|notariz/i);
  const uses = [...policy.matchAll(/uses:\s*(\S+)/g)].map(match => match[1]);
  assert.deepEqual(uses, ['actions/checkout@v4', 'actions/setup-node@v4', 'dtolnay/rust-toolchain@stable', 'actions/upload-artifact@v4', 'actions/upload-artifact@v4']);
  assert.match(policy, /persist-credentials: false/);
  assert.match(policy, /if-no-files-found: error/);
  assert.match(policy, /--preflight/);
  assert.match(policy, /--artifacts/);
  assert.match(policy, /cargo test -p agentz-task-state --locked --test lifecycle_gate -- --nocapture/);
  assert.match(policy, /\$testExit = \$LASTEXITCODE/);
  assert.match(policy, /if \(\$testExit -ne 0\) \{ exit \$testExit \}/);
  assert.match(policy, /if: \$\{\{ always\(\) && steps\.lifecycle\.outcome != 'skipped' \}\}/);
  assert.match(policy, /name: agentz-lifecycle-gate-log/);
  assert.match(policy, /unsigned-windows-x64/);
  assert.match(policy, /node scripts\/upgrade-release-preflight\.mjs --platform windows --arch x64 --channel candidate --artifact-dir \$artifactDir/);
  assert.match(policy, /\$artifactDir = \[System\.IO\.Path\]::GetFullPath/);
  assert.match(policy, /\$preflightExit = \$LASTEXITCODE/);
  assert.match(policy, /if \(\$preflightExit -ne 0\) \{ exit \$preflightExit \}/);
  assert.match(policy, /Set-Content -LiteralPath \(Join-Path \$artifactDir 'candidate-manifest\.json'\) -Encoding utf8/);
  assert.match(policy, /^            target\/release\/bundle\/nsis\/candidate-manifest\.json$/m);
}
function files(directory) {
  return readdirSync(directory).flatMap(name => {
    const path = resolve(directory, name);
    return statSync(path).isDirectory() ? files(path) : [path];
  });
}
function preflight() {
  // Deliberately no fetch-node fallback: its downloads lack trusted digests.
  const manifestPath = resolve(root, 'docs/upgrade/resource-integrity.json');
  assert.ok(existsSync(manifestPath), 'Candidate blocked: approved resource-integrity.json is missing; unchecked runtime downloads are prohibited');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
  assert.equal(manifest.schemaVersion, 1);
  assert.equal(manifest.platform, 'windows-x64');
  assert.equal(manifest.signatureStatus, 'not_verified');
  const required = ['src-tauri/resources/node', 'bundled/preinstall'];
  const resourceFiles = required.flatMap(path => files(resolve(root, path)));
  assert.ok(resourceFiles.length > 0);
  for (const file of resourceFiles) {
    const key = relative(root, file).split(sep).join('/');
    const expected = manifest.sha256?.[key];
    assert.match(expected ?? '', /^[0-9a-f]{64}$/);
    assert.equal(createHash('sha256').update(readFileSync(file)).digest('hex'), expected, `Resource digest mismatch: ${key}`);
  }
  assert.ok(resourceFiles.some(file => /node\.exe$/i.test(file)), 'Windows Node runtime missing');
  for (const path of ['bundled/codebuddy/exclude.json', 'bundled/codebuddy/manifest.json', 'extension-host/package-lock.json', 'Cargo.lock', 'package-lock.json']) {
    assert.ok(existsSync(resolve(root, path)), `Required build input missing: ${path}`);
  }
}
validate(workflow);
if (process.argv.includes('--preflight')) preflight();
else if (process.argv.includes('--artifacts')) {
  const directory = resolve(root, 'target/release/bundle/nsis');
  const installers = readdirSync(directory).filter(name => name.endsWith('.exe'));
  assert.ok(installers.length > 0, 'Required NSIS installer missing');
  for (const name of installers) assert.ok(statSync(resolve(directory, name)).size > 0);
  writeFileSync(resolve(directory, 'candidate-status.json'), JSON.stringify({ kind: 'unsigned_candidate', platform: 'windows-x64', signatureStatus: 'not_verified', runtimeAcceptance: 'not_verified', installers }, null, 2));
} else {
  for (const mutation of [workflow.replace('workflow_dispatch:', 'push:'), workflow.replace('contents: read', 'contents: write'), workflow.replace('if-no-files-found: error', 'if-no-files-found: warn'), `${workflow}\n  release:\n`, `${workflow}\n  secrets:\n`]) {
    assert.throws(() => validate(mutation));
  }
  for (const mutation of [workflow.replace('$preflightExit = $LASTEXITCODE', '$preflightExit = 0'), workflow.replace('if ($preflightExit -ne 0) { exit $preflightExit }', ''), workflow.replace('$testExit = $LASTEXITCODE', '$testExit = 0'), workflow.replace('if ($testExit -ne 0) { exit $testExit }', ''), workflow.replace('            target/release/bundle/nsis/candidate-manifest.json', ''), workflow.replace('--channel candidate', '--channel release')]) {
    assert.throws(() => validate(mutation));
  }
}
console.log('Candidate workflow static verification passed; no CI dispatched or build executed.');
