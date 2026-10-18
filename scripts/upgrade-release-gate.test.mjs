import test from 'node:test';
import assert from 'node:assert/strict';
import { releaseGate } from './upgrade-release-gate.mjs';

const accepted = () => ({
  milestones: ['M0', 'M1', 'M2', 'M3', 'M4', 'M5', 'M6', 'M7', 'M8', 'MB']
    .map(id => ({ id, status: 'passed' })),
  tasks: [{ id: 'AZ-069', status: 'done' }],
});

test('publishing requires every release gate and the accepted release decision', () => {
  assert.deepEqual(releaseGate(accepted(), '0.9.11', 'refs/tags/v0.9.11'), []);
  const plan = accepted();
  plan.milestones.find(item => item.id === 'M7').status = 'pending';
  plan.tasks[0].status = 'deferred';
  assert.deepEqual(releaseGate(plan, '0.9.11', 'refs/tags/v0.9.11'), [
    'M7 gate is not passed', 'AZ-069 release decision is not accepted',
  ]);
});

test('publishing rejects a mismatched version tag', () => {
  assert.deepEqual(releaseGate(accepted(), '0.9.11', 'refs/tags/v0.9.10'), [
    'release ref refs/tags/v0.9.10 must be refs/tags/v0.9.11',
  ]);
  assert.deepEqual(releaseGate(accepted(), '0.9.11', 'refs/heads/main'), [
    'release ref refs/heads/main must be refs/tags/v0.9.11',
  ]);
});
