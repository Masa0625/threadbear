// Invalidation-to-handoff includes waiting for a frame and submitting it.
// It stops at GPUI's present_end, before the frame reaches the display.
export const handoffFields = ['dirtyToPresentP50', 'dirtyToPresentP90', 'dirtyToPresentP99', 'dirtyToPresentMax'] as const;
export type HandoffMetrics = Partial<Record<typeof handoffFields[number], number>> & {dirtyToPresentSamples?: number};
export type HandoffHistogram = {samples: number; p50: number | null; p90: number | null; p99: number | null; max: number | null};

export function handoffMetrics(stats: {dirtyToPresentMs?: HandoffHistogram}): HandoffMetrics {
  const h = stats.dirtyToPresentMs;
  if (!h) return {};
  if (!Number.isSafeInteger(h.samples) || h.samples < 0) throw new Error('Invalid handoff sample count');
  if (h.samples === 0) return {};
  if ([h.p50, h.p90, h.p99, h.max].some(n => n == null || !Number.isFinite(n) || n < 0)) {
    throw new Error('Invalid handoff quantiles');
  }
  return {dirtyToPresentSamples: h.samples, dirtyToPresentP50: h.p50!, dirtyToPresentP90: h.p90!, dirtyToPresentP99: h.p99!, dirtyToPresentMax: h.max!};
}

export function formatMetric(value: number, baseline?: number): string {
  if (!Number.isFinite(value)) return 'n/a';
  const text = value.toFixed(2);
  return baseline != null && Number.isFinite(baseline) && baseline > 0 ? `${text} (${(value / baseline).toFixed(2)})` : text;
}
