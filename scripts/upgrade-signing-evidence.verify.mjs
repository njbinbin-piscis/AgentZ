// Synthetic contract tests; never native signing / installation acceptance.
import assert from 'node:assert/strict';
import { rawHash, validateSigningEvidence } from './upgrade-signing-evidence.mjs';

const digest = 'a'.repeat(64);
function fixture() {
  const tool = {
    provenance: 'fixture', tool: 'signtool', version: 'fixture-version', exit_code: 0,
    raw_result: 'synthetic contract payload; never actual tool execution',
    captured_at: '2026-10-07T01:00:00.000Z', artifact_sha256: digest, identity: 'test-pinned-identity',
  };
  tool.raw_sha256 = rawHash(tool.raw_result);
  const step = time => {
    const raw = 'synthetic installation record';
    return { provenance: 'fixture', executed: true, result: 'passed', executed_at: time,
      artifact_sha256: digest, raw_result: raw, raw_sha256: rawHash(raw), recording_sha256: 'b'.repeat(64) };
  };
  return {
    schema_version: 1, provenance: 'fixture', recorded_at: '2026-10-07T05:00:00.000Z', platform: 'windows', arch: 'x64',
    artifact: { name: 'fixture.msi', sha256_expected: digest, sha256_observed: digest },
    signing: {
      tool_available: true, certificate_available: true, expected_identity: 'test-pinned-identity', actual_identity: 'test-pinned-identity',
      executed: true, status: 'valid', verified_at: '2026-10-07T01:00:00.000Z', tool_result: tool,
      validity: { signature_valid: true, certificate_valid_at_signing: true,
        certificate_not_before: '2026-01-01T00:00:00.000Z', certificate_not_after: '2027-01-01T00:00:00.000Z',
        timestamp_required: true, timestamp_valid: true, timestamp_at: '2026-10-07T00:00:00.000Z' },
    },
    notarization: { required: false, executed: false, status: 'unverified', tool_result: null, staple_executed: false, staple_status: 'unverified', staple_result: null },
    installation: { executed: true, status: 'complete', clean_machine: true, machine_id: 'fixture-machine', platform: 'windows', arch: 'x64',
      steps: { install: step('2026-10-07T02:00:00.000Z'), launch: step('2026-10-07T03:00:00.000Z'), uninstall: step('2026-10-07T04:00:00.000Z') } },
  };
}

const cases = [
  ['complete synthetic record stays candidate', document => {}, result => {
    assert.equal(result.schema_valid, true); assert.equal(result.record_kind, 'candidate'); assert.equal(result.review_ready, false);
    assert.equal(result.signing_status, 'candidate'); assert.equal(result.installation_status, 'candidate');
  }],
  ['outer actual cannot promote fixture receipts', document => { document.provenance = 'actual'; }, result => assert.equal(result.record_kind, 'candidate')],
  ['expected identity mismatch', document => { document.signing.expected_identity = 'other-pinned-identity'; }, result => assert.equal(result.signing_status, 'invalid')],
  ['observed artifact mismatch', document => { document.artifact.sha256_observed = 'c'.repeat(64); }, result => assert.equal(result.signing_status, 'invalid')],
  ['tool captured wrong artifact', document => { document.signing.tool_result.artifact_sha256 = 'c'.repeat(64); }, result => assert.equal(result.signing_status, 'invalid')],
  ['tool captured wrong signer identity', document => { document.signing.tool_result.identity = 'other-captured-identity'; }, result => assert.equal(result.signing_status, 'invalid')],
  ['raw output tampered', document => { document.signing.tool_result.raw_result += 'tampered'; }, result => assert.equal(result.signing_status, 'invalid')],
  ['tool unavailable', document => { document.signing.tool_available = false; }, result => assert.equal(result.signing_status, 'missing')],
  ['certificate unavailable', document => { document.signing.certificate_available = false; }, result => assert.equal(result.signing_status, 'missing')],
  ['expected identity missing', document => { document.signing.expected_identity = null; }, result => assert.equal(result.signing_status, 'missing')],
  ['mock unexecuted record never becomes acceptance', document => { document.signing.executed = false; document.signing.status = 'unverified'; }, result => assert.equal(result.record_kind, 'candidate')],
  ['bad verification exit', document => { document.signing.tool_result.exit_code = 1; }, result => assert.equal(result.signing_status, 'invalid')],
  ['timestamp invalid', document => { document.signing.validity.timestamp_valid = false; }, result => assert.equal(result.signing_status, 'invalid')],
  ['Windows timestamp cannot be opted out', document => { document.signing.validity.timestamp_required = false; }, result => assert.equal(result.signing_status, 'invalid')],
  ['timestamp outside certificate validity', document => { document.signing.validity.certificate_not_after = '2026-09-01T00:00:00.000Z'; }, result => assert.equal(result.signing_status, 'invalid')],
  ['signature invalid independently of installation', document => { document.signing.validity.signature_valid = false; }, result => {
    assert.equal(result.signing_status, 'invalid'); assert.equal(result.installation_status, 'candidate');
  }],
  ['installation platform mismatch', document => { document.installation.platform = 'linux'; }, result => assert.equal(result.installation_status, 'invalid')],
  ['missing uninstall recording', document => { document.installation.steps.uninstall.recording_sha256 = null; }, result => assert.ok(result.missing.some(item => item.startsWith('installation.uninstall')))],
  ['installation logs tampered', document => { document.installation.steps.launch.raw_result += 'tampered'; }, result => assert.equal(result.installation_status, 'invalid')],
  ['installation chronology invalid', document => { document.installation.steps.launch.executed_at = '2026-10-07T01:30:00.000Z'; }, result => assert.equal(result.installation_status, 'invalid')],
  ['macOS notarization required', document => { document.platform = 'macos'; document.installation.platform = 'macos'; }, result => assert.equal(result.notarization_status, 'invalid')],
  ['macOS missing notarization stays unverified', document => { document.platform = 'macos'; document.installation.platform = 'macos'; document.notarization.required = true; }, result => assert.ok(result.missing.some(item => item.startsWith('notarization:')))],
  ['invalid calendar date', document => { document.recorded_at = '2026-02-30T00:00:00.000Z'; }, result => assert.equal(result.schema_valid, false)],
  ['truthy string execution rejected', document => { document.signing.executed = 'true'; }, result => assert.equal(result.schema_valid, false)],
  ['unsupported architecture rejected', document => { document.arch = 'unknown'; }, result => assert.equal(result.schema_valid, false)],
  ['unknown field rejected', document => { document.acceptance_passed = true; }, result => assert.equal(result.schema_valid, false)],
];

for (const [name, edit, check] of cases) {
  const document = fixture();
  edit(document);
  const before = JSON.stringify(document);
  const result = validateSigningEvidence(document);
  check(result);
  assert.equal(result.acceptance_passed, false, `${name}: cannot assert native acceptance`);
  assert.equal(result.review_ready, false, `${name}: synthetic evidence cannot become ready`);
  assert.equal(JSON.stringify(document), before, `${name}: parser must not mutate input`);
}
console.log(`${cases.length} signing contract fixtures passed; 0 native signing/installations executed`);
