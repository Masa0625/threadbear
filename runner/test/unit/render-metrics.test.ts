import assert from 'node:assert/strict';
import {test} from 'node:test';
import {renderRow, comparisonTable, formatMetric, type RenderStats} from '../../../features/tools/render-metrics.ts';

const stats = (): RenderStats => ({screens: [{hz: 60, presented: 100, late: 2, missedRefreshes: 2, workMs: {p50: 2, p90: 3, p99: 4, max: 5}}], mainCpuMs: 5, dirtyToPresentMs: {samples: 100, p50: 18, p90: 31, p99: 42, max: 60}});

test('actual rows and comparison output retain delay excluded from CPU and work time', () => {
  const b = renderRow('inbox', false, stats())!;
  assert.equal(b.cpuMs, 5); assert.equal(b.p90, 3); assert.equal(b.dirtyToPresentP90, 31);
  const a = {...b, dirtyToPresentP90: 20};
  const table = comparisonTable({A: [a], B: [b]}, 3);
  assert.match(table, /dirtyToPresentP90\s+20\.00 \[1\/3\]\s+31\.00 \(1\.55\) \[1\/3\]/);
  assert.match(table, /dirtyToPresentSamples\s+100\.00/);
  assert.equal(renderRow('inbox', false, {screens: [], mainCpuMs: 0}), null);
});

test('missing and empty intervals remain unavailable and expose unequal sample coverage', () => {
  const old = stats(); delete old.dirtyToPresentMs;
  const empty = {...stats(), dirtyToPresentMs: {samples: 0, p50: null, p90: null, p99: null, max: null}};
  const a = renderRow('inbox', false, old)!, b = renderRow('inbox', false, empty)!;
  assert.equal(a.dirtyToPresentP90, undefined); assert.equal(b.dirtyToPresentP90, undefined);
  assert.equal(b.dirtyToPresentSamples, 0);
  const table = comparisonTable({A: [a], B: [b, renderRow('inbox', false, stats())!]}, 3);
  assert.match(table, /dirtyToPresentP90\s+n\/a \[0\/3\]\s+31\.00 \[1\/3\]/);
  assert.equal(formatMetric(Number.NaN), 'n/a');
  assert.equal(formatMetric(20, 0), '20.00');
  assert.equal(formatMetric(0, 20), '0.00 (0.00)');
});

test('timer errors preserve CPU and work rows and keep the producer diagnostic', () => {
  const bad = renderRow('transcript', false, {...stats(), dirtyToPresentMs: {error: 'baseline histogram is incompatible'}})!;
  assert.equal(bad.cpuMs, 5); assert.equal(bad.p90, 3); assert.equal(bad.dirtyToPresentP90, undefined);
  assert.match(bad.dirtyToPresentError!, /baseline histogram is incompatible/);
  for (const value of [null, Number.NaN, Number.POSITIVE_INFINITY, -1]) {
    const r = renderRow('transcript', false, {...stats(), dirtyToPresentMs: {samples: 10, p50: 1, p90: value, p99: 3, max: 4}})!;
    assert.equal(r.cpuMs, 5); assert.match(r.dirtyToPresentError!, /Invalid handoff/);
  }
});

test('dominant-screen work and window-wide handoff are explicitly different scopes', () => {
  const s = stats(); s.screens.push({hz: 120, presented: 50, late: 0, missedRefreshes: 0, workMs: {p50: 1, p90: 2, p99: 3, max: 4}});
  s.dirtyToPresentMs = {samples: 150, p50: 18, p90: 31, p99: 42, max: 60};
  const row = renderRow('transcript', false, s)!;
  assert.equal(row.frames, 100); assert.equal(row.screenCount, 2); assert.equal(row.dirtyToPresentSamples, 150);
  assert.equal(row.dirtyToPresentScope, 'window');
  assert.match(comparisonTable({A: [row]}, 1), /Work uses the busiest screen; handoff covers the whole window/);
});
