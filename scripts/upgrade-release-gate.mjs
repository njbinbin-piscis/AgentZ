#!/usr/bin/env node
// Fail closed before the publishing workflow starts any bundle job.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const requiredGates = ['M0', 'M1', 'M2', 'M3', 'M4', 'M5', 'M6', 'M7', 'M8', 'MB'];

export function releaseGate(plan, version, gitRef) {
  const errors = [];
  if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) errors.push('invalid product version');
  if (gitRef !== `refs/tags/v${version}`) {
    errors.push(`release ref ${gitRef ?? '(missing)'} must be refs/tags/v${version}`);
  }
  const milestones = new Map((plan?.milestones ?? []).map(item => [item.id, item]));
  for (const id of requiredGates) {
    if (milestones.get(id)?.status !== 'passed') errors.push(`${id} gate is not passed`);
  }
  if (plan?.tasks?.find(item => item.id === 'AZ-069')?.status !== 'done') {
    errors.push('AZ-069 release decision is not accepted');
  }
  return errors;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const plan = JSON.parse(readFileSync(path.join(root, 'docs/upgrade/plan.json'), 'utf8'));
  const version = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8')).version;
  const errors = releaseGate(plan, version, process.env.GITHUB_REF);
  if (errors.length) {
    console.error(`Publishing gate closed for AgentZ ${version}:\n${errors.map(error => `- ${error}`).join('\n')}`);
    process.exitCode = 1;
  } else {
    console.log(`Publishing gate open for AgentZ ${version}; inspect signed artifacts and decision receipts separately.`);
  }
}
