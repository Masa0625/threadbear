// Darwin ps reports cumulative CPU time as [days-][hours:]minutes:seconds.
export function cpuTimeSeconds(value: string): number {
  const text = value.trim();
  if (!/^(?:\d+-)?\d+:\d{2}(?::\d{2})?(?:\.\d+)?$/.test(text)) throw new Error(`Invalid process CPU time: ${text}`);
  const [days, clock] = text.includes('-') ? text.split('-') : ['0', text];
  const parts = clock.split(':').map(Number);
  const seconds = parts.pop()!;
  const minutes = parts.pop()!;
  const hours = parts.pop() ?? 0;
  if (seconds >= 60 || (clock.split(':').length === 3 && minutes >= 60) || (text.includes('-') && hours >= 24)) throw new Error(`Invalid process CPU time: ${text}`);
  const result = Number(days) * 86400 + hours * 3600 + minutes * 60 + seconds;
  if (!Number.isFinite(result)) throw new Error(`Invalid process CPU time: ${text}`);
  return result;
}

export function windowServerPid(processes: Iterable<{pid: number; name: string}>): number {
  const pids = new Set([...processes].filter(process => process.name === 'WindowServer').map(process => process.pid));
  if (pids.size !== 1) throw new Error('Cannot identify one WindowServer for the shared CPU counter.');
  const pid = [...pids][0];
  if (!Number.isInteger(pid) || pid <= 0) throw new Error('Invalid WindowServer PID.');
  return pid;
}
