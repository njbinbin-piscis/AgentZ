// Offline record validation only: no subprocesses, artifact reads, installs or secrets.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(import.meta.url);
const schema = JSON.parse(fs.readFileSync(path.resolve(path.dirname(scriptPath), '../docs/upgrade/contracts/signing-evidence.schema.json'), 'utf8'));
export const rawHash = raw => crypto.createHash('sha256').update(raw, 'utf8').digest('hex');

function schemaErrors(value, rule, location = '$', errors = []) {
  if (rule.$ref) return schemaErrors(value, schema.$defs[rule.$ref.split('/').at(-1)], location, errors);
  const actualType = value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value;
  if (rule.type) {
    const types = [].concat(rule.type);
    if (!types.some(type => type === actualType || (type === 'integer' && Number.isInteger(value)))) {
      errors.push(`${location}: incorrect type`); return errors;
    }
  }
  if ('const' in rule && value !== rule.const) errors.push(`${location}: unsupported value`);
  if (rule.enum && !rule.enum.includes(value)) errors.push(`${location}: unsupported value`);
  if (typeof value === 'string') {
    if (rule.minLength && value.length < rule.minLength) errors.push(`${location}: empty value`);
    if (rule.pattern && !new RegExp(rule.pattern).test(value)) errors.push(`${location}: invalid format`);
    if (rule.format === 'date-time' && (!Number.isFinite(Date.parse(value)) || new Date(value).toISOString() !== value)) errors.push(`${location}: invalid UTC timestamp`);
  }
  if (value !== null && actualType === 'object') {
    for (const key of rule.required ?? []) if (!Object.hasOwn(value, key)) errors.push(`${location}.${key}: required`);
    for (const [key, field] of Object.entries(value)) {
      if (rule.properties?.[key]) schemaErrors(field, rule.properties[key], `${location}.${key}`, errors);
      else if (rule.additionalProperties === false) errors.push(`${location}.${key}: unexpected field`);
    }
  }
  return errors;
}

export function validateSigningEvidence(input) {
  const errors = schemaErrors(input, schema);
  const result = {
    schema_valid: errors.length === 0,
    record_kind: 'unverified', signing_status: 'unverified', notarization_status: 'unverified', installation_status: 'unverified',
    review_ready: false, acceptance_passed: false, acceptance_status: 'requires-independent-native-review',
    errors, missing: [],
    limitation: 'Only validates supplied evidence consistency. Does not authenticate execution, inspect artifact bytes, verify signatures, install packages or open recordings.',
  };
  if (errors.length) return result;
  const { artifact, signing, notarization, installation } = input;
  const receipts = [signing.tool_result, notarization.tool_result, notarization.staple_result, ...Object.values(installation.steps)].filter(Boolean);
  const candidate = input.provenance !== 'actual' || receipts.some(receipt => receipt.provenance !== 'actual');
  result.record_kind = !candidate && signing.executed ? 'actual-claim' : 'candidate';
  const signingErrors = [], notarizationErrors = [], installationErrors = [];
  const signingMissing = [], notarizationMissing = [], installationMissing = [];
  const error = (list, message) => { list.push(message); result.errors.push(message); };
  const missing = (list, message) => { list.push(message); result.missing.push(message); };
  const validIdentity = value => typeof value === 'string' && value.trim().length > 0;
  const checkTool = (tool, executed, label, issues, absent) => {
    if (!executed) { missing(absent, `${label}: not executed`); return; }
    if (!tool) { missing(absent, `${label}: tool result missing`); return; }
    if (!tool.raw_result.trim()) error(issues, `${label}: empty raw result`);
    if (rawHash(tool.raw_result) !== tool.raw_sha256) error(issues, `${label}: raw result digest mismatch`);
    if (tool.artifact_sha256 !== artifact.sha256_expected) error(issues, `${label}: artifact digest mismatch`);
    if (tool.exit_code !== 0) error(issues, `${label}: tool exit was not successful`);
    if (tool.captured_at > input.recorded_at) error(issues, `${label}: capture later than record`);
    if (tool.identity !== signing.actual_identity) error(issues, `${label}: captured identity mismatch`);
  };
  if (artifact.sha256_observed === null) missing(signingMissing, 'artifact: observed digest missing');
  else if (artifact.sha256_expected !== artifact.sha256_observed) error(signingErrors, 'artifact: expected/observed digest mismatch');
  if (!signing.tool_available) missing(signingMissing, 'signing: verification tool missing');
  if (!signing.certificate_available) missing(signingMissing, 'signing: certificate/trust material missing');
  if (!validIdentity(signing.expected_identity)) missing(signingMissing, 'signing: expected identity missing');
  if (!validIdentity(signing.actual_identity)) missing(signingMissing, 'signing: actual identity unverified');
  if (validIdentity(signing.expected_identity) && validIdentity(signing.actual_identity) && signing.expected_identity !== signing.actual_identity) error(signingErrors, 'signing: expected/actual identity mismatch');
  checkTool(signing.tool_result, signing.executed, 'signing', signingErrors, signingMissing);
  if (signing.status === 'invalid') error(signingErrors, 'signing: reported invalid');
  else if (signing.status !== 'valid') missing(signingMissing, `signing: reported ${signing.status}`);
  if (!signing.verified_at) missing(signingMissing, 'signing: verification time missing');
  else if (signing.verified_at > input.recorded_at) error(signingErrors, 'signing: verification later than record');
  const validity = signing.validity;
  for (const field of ['signature_valid', 'certificate_valid_at_signing']) {
    if (validity[field] === false) error(signingErrors, `signing: ${field} false`);
    else if (validity[field] !== true) missing(signingMissing, `signing: ${field} unverified`);
  }
  if (['windows', 'macos'].includes(input.platform) && !validity.timestamp_required) error(signingErrors, 'signing: release timestamp required by contract');
  if (validity.timestamp_required) {
    if (validity.timestamp_valid === false) error(signingErrors, 'signing: timestamp invalid');
    else if (validity.timestamp_valid !== true || !validity.timestamp_at) missing(signingMissing, 'signing: timestamp unverified');
  }
  if (validity.timestamp_at && !validity.timestamp_required && validity.timestamp_valid !== true) {
    if (validity.timestamp_valid === false) error(signingErrors, 'signing: optional timestamp explicitly invalid');
    else missing(signingMissing, 'signing: optional timestamp unverified');
  }
  if (!validity.certificate_not_before || !validity.certificate_not_after) missing(signingMissing, 'signing: certificate validity window missing');
  else {
    if (validity.certificate_not_before > validity.certificate_not_after) error(signingErrors, 'signing: reversed certificate window');
    const reference = validity.timestamp_valid === true ? validity.timestamp_at ?? signing.verified_at : signing.verified_at;
    if (reference && (reference < validity.certificate_not_before || reference > validity.certificate_not_after)) error(signingErrors, 'signing: signing/reference time outside certificate window');
  }
  if (validity.timestamp_at && signing.verified_at && validity.timestamp_at > signing.verified_at) error(signingErrors, 'signing: timestamp later than verification');
  if (input.platform === 'macos' && !notarization.required) error(notarizationErrors, 'notarization: required for macOS release contract');
  if (notarization.required) {
    checkTool(notarization.tool_result, notarization.executed, 'notarization', notarizationErrors, notarizationMissing);
    checkTool(notarization.staple_result, notarization.staple_executed, 'staple', notarizationErrors, notarizationMissing);
    for (const field of ['status', 'staple_status']) {
      if (notarization[field] === 'invalid') error(notarizationErrors, `notarization: ${field} invalid`);
      else if (notarization[field] !== 'valid') missing(notarizationMissing, `notarization: ${field} unverified`);
    }
  } else if (notarization.executed || notarization.staple_executed || notarization.tool_result || notarization.staple_result) {
    error(notarizationErrors, 'notarization: unnecessary execution claims must be represented as required evidence');
  }
  if (installation.platform !== input.platform || installation.arch !== input.arch) error(installationErrors, 'installation: platform/architecture mismatch');
  if (!installation.executed) missing(installationMissing, 'installation: not executed');
  if (installation.clean_machine !== true) missing(installationMissing, 'installation: clean machine unverified');
  if (!validIdentity(installation.machine_id)) missing(installationMissing, 'installation: machine identity missing');
  if (installation.status === 'failed') error(installationErrors, 'installation: reported failed');
  else if (installation.status !== 'complete') missing(installationMissing, `installation: reported ${installation.status}`);
  let previous = null;
  for (const name of ['install', 'launch', 'uninstall']) {
    const step = installation.steps[name];
    if (!step.executed || step.result !== 'passed') {
      if (step.result === 'failed') error(installationErrors, `installation.${name}: failed`);
      else missing(installationMissing, `installation.${name}: unverified`);
    }
    if (!step.executed) continue;
    if (!step.executed_at || !step.raw_result || !step.raw_sha256 || !step.recording_sha256) missing(installationMissing, `installation.${name}: time/log/recording missing`);
    if (step.raw_result && step.raw_sha256 && rawHash(step.raw_result) !== step.raw_sha256) error(installationErrors, `installation.${name}: raw result digest mismatch`);
    if (step.artifact_sha256 !== artifact.sha256_expected) error(installationErrors, `installation.${name}: artifact digest mismatch`);
    if (step.executed_at && (step.executed_at > input.recorded_at || (previous && step.executed_at < previous))) error(installationErrors, `installation.${name}: invalid step chronology`);
    previous = step.executed_at ?? previous;
  }
  const state = (issues, absent) => issues.length ? 'invalid' : candidate ? 'candidate' : absent.length ? 'unverified' : 'evidence-ready-for-review';
  result.signing_status = state(signingErrors, signingMissing);
  if (!signingErrors.length && (!signing.tool_available || !signing.certificate_available || !validIdentity(signing.expected_identity))) result.signing_status = 'missing';
  result.notarization_status = state(notarizationErrors, notarizationMissing);
  if (!notarization.required && !notarizationErrors.length) result.notarization_status = 'not-required';
  result.installation_status = state(installationErrors, installationMissing);
  result.review_ready = !candidate && !result.errors.length && !result.missing.length;
  return result;
}

if (process.argv[1] && path.resolve(process.argv[1]) === scriptPath) {
  if (process.argv.length !== 3) throw new Error('Usage: node scripts/upgrade-signing-evidence.mjs <evidence.json>');
  const input = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
  const result = validateSigningEvidence(input);
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  process.exitCode = !result.schema_valid ? 2 : result.errors.length ? 3 : result.record_kind === 'candidate' ? 5 : result.review_ready ? 0 : 4;
}
