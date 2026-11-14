import assert from 'node:assert/strict';
import { test } from 'node:test';
import { assertArcProgress, nativeArc } from '../../../features/tools/arc-progress.ts';

const arc = (phase: number, snapshotSeconds: number) => ({
  requested: true, backend: 'core-animation', blockedBy: [],
  placed: 1, visible: 1, animating: 1, presented: 1,
  phase, periodSeconds: 0.9, snapshotSeconds,
});

test('a presented native arc advances through a phase wrap', () => {
  assertArcProgress(arc(0.9, 10), arc(0.15, 10.225));
});
test('an animation key without presentation is insufficient', () => {
  assert.throws(() => nativeArc({...arc(0.1, 10), presented: 0}), /not visibly presented/);
});
test('an always-present overlay cannot masquerade as native rendering', () => {
  assert.throws(() => nativeArc({...arc(0.1, 10), backend: 'rust', visible: 0, blockedBy: ['deferred']}), /not visibly presented/);
});
test('a frozen native arc is rejected even if its animation key exists', () => {
  assert.throws(() => assertArcProgress(arc(0.2, 10), arc(0.2, 10.225)), /did not advance/);
});
test('an arc moving at the wrong speed is rejected', () => {
  assert.throws(() => assertArcProgress(arc(0.2, 10), arc(0.7, 10.225)), /advertised speed/);
});
test('whole-turn aliasing and delayed probes are rejected', () => {
  assert.throws(() => assertArcProgress(arc(0.2, 10), arc(0.2, 10.9)), /useful part/);
});
test('invalid phase and timing fields cannot pass presentation checks', () => {
  for (const invalid of [{phase: NaN}, {phase: 1}, {periodSeconds: 0}, {snapshotSeconds: Infinity}]) {
    assert.throws(() => nativeArc({...arc(0.1, 10), ...invalid}), /not visibly presented/);
  }
});
