// CALayer animation does not require GPUI frames. Check its actual presentation
// outside the CPU interval instead of accepting a static or permanently hidden arc.
type NativeArc = {
  requested: boolean; backend: string; blockedBy: string[];
  placed: number; visible: number; animating: number; presented: number;
  phase: number; periodSeconds: number; snapshotSeconds: number;
};

export function nativeArc(raw: unknown): NativeArc {
  const arc = raw as NativeArc | undefined;
  const counts = arc && [arc.placed, arc.visible, arc.animating, arc.presented];
  if (!arc || arc.requested !== true || arc.backend !== 'core-animation'
    || !Array.isArray(arc.blockedBy) || arc.blockedBy.length
    || !counts?.every(n => Number.isInteger(n) && n >= 1)
    || arc.placed < arc.visible || arc.animating < arc.visible || arc.presented < arc.visible
    || !Number.isFinite(arc.phase) || arc.phase < 0 || arc.phase >= 1
    || !Number.isFinite(arc.periodSeconds) || arc.periodSeconds <= 0 || arc.periodSeconds > 10
    || !Number.isFinite(arc.snapshotSeconds) || arc.snapshotSeconds < 0) {
    throw new Error(`Native working arcs are not visibly presented: ${JSON.stringify(raw)}`);
  }
  return arc;
}

export function assertArcProgress(before: unknown, after: unknown): void {
  const a = nativeArc(before), b = nativeArc(after);
  const expected = (b.snapshotSeconds - a.snapshotSeconds) / a.periodSeconds;
  if (Math.abs(a.periodSeconds - b.periodSeconds) > 1e-6 || expected < 0.1 || expected > 0.75) {
    throw new Error('Native arc probe did not span a useful part of a turn.');
  }
  const observed = (b.phase - a.phase + 1) % 1;
  const difference = Math.abs(observed - expected);
  if (observed < 0.04 || Math.min(difference, 1 - difference) > 0.08) {
    throw new Error(`Native working arc did not advance at its advertised speed: ${JSON.stringify({expected, observed})}`);
  }
}
