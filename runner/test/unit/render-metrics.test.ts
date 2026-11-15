import assert from 'node:assert/strict';
import {test} from 'node:test';
import {handoffFields, handoffMetrics, formatMetric} from '../../../features/tools/render-metrics.ts';

test('scroll comparisons retain the handoff delay that CPU and work time leave out', () => {
  const stats = {mainCpuMs: 5, workMs: {p90: 3}, dirtyToPresentMs: {samples: 240, p50: 18, p90: 31, p99: 42, max: 60}};
  const row = handoffMetrics(stats);
  assert.deepEqual(row, {dirtyToPresentSamples: 240, dirtyToPresentP50: 18, dirtyToPresentP90: 31, dirtyToPresentP99: 42, dirtyToPresentMax: 60});
  assert.deepEqual(handoffFields.map(field => row[field]), [18, 31, 42, 60]);
  assert.equal(formatMetric(row.dirtyToPresentP90!, 20), '31.00 (1.55)');
});

test('older binaries and empty intervals do not become zero-latency results', () => {
  assert.deepEqual(handoffMetrics({}), {});
  assert.deepEqual(handoffMetrics({dirtyToPresentMs: {samples: 0, p50: null, p90: null, p99: null, max: null}}), {});
  assert.equal(formatMetric(Number.NaN), 'n/a');
  assert.equal(formatMetric(20, Number.NaN), '20.00');
  assert.equal(formatMetric(20, 0), '20.00');
  assert.equal(formatMetric(0, 20), '0.00 (0.00)');
});

test('a populated but malformed handoff interval is rejected before comparison', () => {
  for (const value of [null, Number.NaN, Number.POSITIVE_INFINITY, -1]) {
    assert.throws(() => handoffMetrics({dirtyToPresentMs: {samples: 10, p50: 1, p90: value, p99: 3, max: 4}}), /Invalid handoff/);
  }
  for (const samples of [-1, 1.5, Number.NaN]) {
    assert.throws(() => handoffMetrics({dirtyToPresentMs: {samples, p50: 1, p90: 2, p99: 3, max: 4}}), /Invalid handoff/);
  }
});
