// Invalidation-to-handoff includes frame waiting and submission through
// GPUI's present_end. It is window-wide and stops before display/glass time.
export const handoffFields = ['dirtyToPresentP50', 'dirtyToPresentP90', 'dirtyToPresentP99', 'dirtyToPresentMax'] as const;
export type HandoffMetrics = Partial<Record<typeof handoffFields[number], number>> & {dirtyToPresentSamples?: number; dirtyToPresentScope?: 'window'; dirtyToPresentError?: string};
export type HandoffHistogram = {samples: number; p50: number | null; p90: number | null; p99: number | null; max: number | null};

export function handoffMetrics(stats: {dirtyToPresentMs?: unknown}): HandoffMetrics {
  const raw = stats.dirtyToPresentMs;
  const error = (message: string): HandoffMetrics => ({dirtyToPresentScope: 'window', dirtyToPresentError: message});
  if (raw == null) return {};
  if (typeof raw !== 'object' || Array.isArray(raw)) return error('Invalid handoff histogram');
  const h = raw as Partial<HandoffHistogram> & {error?: unknown};
  if ('error' in h) return error(typeof h.error === 'string' ? h.error : 'Invalid handoff error');
  if (typeof h.samples !== 'number' || !Number.isSafeInteger(h.samples) || h.samples < 0) return error('Invalid handoff sample count');
  if (h.samples === 0) return {dirtyToPresentScope: 'window', dirtyToPresentSamples: 0};
  if ([h.p50, h.p90, h.p99, h.max].some(n => n == null || !Number.isFinite(n) || n < 0)) {
    return error('Invalid handoff quantiles');
  }
  return {dirtyToPresentScope: 'window', dirtyToPresentSamples: h.samples, dirtyToPresentP50: h.p50!, dirtyToPresentP90: h.p90!, dirtyToPresentP99: h.p99!, dirtyToPresentMax: h.max!};
}

export function formatMetric(value: number, baseline?: number): string {
  if (!Number.isFinite(value)) return 'n/a';
  const text = value.toFixed(2);
  return baseline != null && Number.isFinite(baseline) && baseline > 0 ? `${text} (${(value / baseline).toFixed(2)})` : text;
}

export type RenderScreen = {hz: number; presented: number; late: number; missedRefreshes: number; workMs: Record<string, number>};
export type RenderStats = {screens: RenderScreen[]; mainCpuMs: number; dirtyToPresentMs?: unknown};
export type RenderRow = {scenario: string; load: boolean; hz: number; screenCount: number; frames: number; late: number; p50: number; p90: number; p99: number; max: number; cpuMs: number; cpuPerFrame: number; cpuPerSecond?: number} & HandoffMetrics;

export function renderRow(scenario: string, load: boolean, stats: RenderStats, seconds?: number): RenderRow | null {
  const s = [...stats.screens].sort((a, b) => b.presented - a.presented)[0];
  if (!s) return null;
  return {scenario, load, hz: s.hz, screenCount: stats.screens.length, frames: s.presented, late: s.late, p50: s.workMs.p50, p90: s.workMs.p90, p99: s.workMs.p99, max: s.workMs.max, cpuMs: stats.mainCpuMs, cpuPerFrame: Math.round(100 * stats.mainCpuMs / Math.max(1, s.presented)) / 100, ...(seconds ? {cpuPerSecond: Math.round(stats.mainCpuMs / seconds)} : {}), ...handoffMetrics(stats)};
}

export function comparisonTable(runs: Record<string, RenderRow[]>, rounds: number): string {
  const names = Object.keys(runs);
  const fields = ['late%', 'p50', 'p90', 'p99', 'cpuPerFrame', 'cpuPerSecond', ...handoffFields, 'dirtyToPresentSamples'];
  const width = Math.max(...fields.map(f => f.length));
  const key = (r: RenderRow) => `${r.scenario}${r.load ? ' loaded' : ''}`;
  const keys = [...new Set(Object.values(runs).flat().map(key))];
  const median = (xs: number[]) => {
    const s = [...xs].sort((a, b) => a - b);
    return s.length ? (s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2) : Number.NaN;
  };
  const pick = (rows: RenderRow[], field: string) => rows.map(row => field === 'late%' ? 100 * row.late / Math.max(1, row.frames) : row[field as keyof RenderRow]).filter((v): v is number => typeof v === 'number' && Number.isFinite(v));
  const lines = [`Medians over ${rounds} rounds; shares of A in parentheses, valid/requested rounds in brackets.`, 'Work uses the busiest screen; handoff covers the whole window. Timing in ms; handoff samples are counts.'];
  for (const k of keys) {
    lines.push(`  ${k}`);
    const rows = (name: string) => runs[name].filter(r => key(r) === k);
    for (const field of fields) {
      if (field === 'cpuPerSecond' && names.every(name => pick(rows(name), field).length === 0)) continue;
      const baseline = median(pick(rows('A'), field));
      const cells = names.map(name => {
        const values = pick(rows(name), field);
        const value = median(values);
        const text = field === 'dirtyToPresentSamples' ? (Number.isFinite(value) ? String(value) : 'n/a') : formatMetric(value, name === 'A' ? undefined : baseline);
        return `${text} [${values.length}/${rounds}]`;
      });
      lines.push(`    ${field.padEnd(width)} ${cells.join('  ')}`);
    }
  }
  return lines.join('\n');
}
