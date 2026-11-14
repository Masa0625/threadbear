import assert from 'node:assert/strict';
import { test } from 'node:test';
import { cpuTimeSeconds, windowServerPid } from '../../../features/tools/cpu-time.ts';

test('Darwin cumulative CPU time includes hundredths and unbounded minutes', () => {
  assert.equal(cpuTimeSeconds('715:36.34'), 42936.34);
  assert.equal(cpuTimeSeconds('  0:00.00\n'), 0);
  assert.equal(cpuTimeSeconds('1:02:03.25'), 3723.25);
  assert.equal(cpuTimeSeconds('2-03:04:05.50'), 183845.5);
});
test('multiple kernel identities for the same WindowServer are one process', () => {
  assert.equal(windowServerPid([{pid:617,name:'WindowServer'}, {pid:617,name:'WindowServer'}, {pid:7,name:'node'}]), 617);
});
test('missing or ambiguous WindowServer identity stops the measurement', () => {
  assert.throws(() => windowServerPid([]), /Cannot identify/);
  assert.throws(() => windowServerPid([{pid:1,name:'WindowServer'}, {pid:2,name:'WindowServer'}]), /Cannot identify/);
});
test('malformed CPU time cannot become a zero-CPU measurement', () => {
  for (const value of ['', 'NaN', '00:60.00', '1:72:03', '2-25:00:00', 'error']) {
    assert.throws(() => cpuTimeSeconds(value), /Invalid process CPU time/);
  }
});
