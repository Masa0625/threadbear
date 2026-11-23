// Temporary (diagnosis only): when each test file started and how long it took.
import path from 'node:path';
export default async function* diag(source) {
  const t0 = Date.now();
  const at = () => `${String(Math.round((Date.now() - t0) / 1000)).padStart(5)}s`;
  const started = new Map();
  const done = [];
  for await (const e of source) {
    const file = e.data?.file;
    if (!file) continue;
    const name = path.basename(file);
    if (!started.has(file)) { started.set(file, Date.now()); yield `${at()} start ${name}\n`; }
    if ((e.type === 'test:pass' || e.type === 'test:fail') && e.data.nesting === 0 && (e.data.details?.duration_ms ?? 0) > 10_000 && e.data.name !== name) {
      yield `${at()}   ${e.type === 'test:pass' ? 'pass' : 'FAIL'} ${name}: ${e.data.name} (${Math.round(e.data.details.duration_ms / 1000)}s)\n`;
    }
    if (e.type === 'test:summary') {
      const secs = Math.round((Date.now() - started.get(file)) / 1000);
      done.push([secs, name]);
      yield `${at()} done  ${name} ${secs}s${e.data.success ? '' : ' FAILED'}\n`;
    }
  }
  yield '\nSlowest files:\n';
  for (const [secs, name] of done.sort((a, b) => b[0] - a[0]).slice(0, 40)) yield `${String(secs).padStart(5)}s ${name}\n`;
}
