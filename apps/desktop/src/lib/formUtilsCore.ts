/** Suggest output path from input basename under optional default root. */
export function suggestOutputPath(input: string, defaultRoot: string, suffix: string): string {
  const trimmed = input.trim().replace(/[/\\]+$/, '');
  if (!trimmed) return '';
  const parts = trimmed.split(/[/\\]/);
  const base = parts[parts.length - 1] || 'output';
  const name = base.toLowerCase().endsWith(suffix.toLowerCase()) ? base : `${base}${suffix}`;
  const root = defaultRoot.trim().replace(/[/\\]+$/, '');
  if (root) {
    const sep = root.includes('\\') ? '\\' : '/';
    return `${root}${sep}${name}`;
  }
  const sep = trimmed.includes('\\') ? '\\' : '/';
  const parent = parts.slice(0, -1).join(sep);
  return parent ? `${parent}${sep}${name}` : name;
}

export function cloneOutputPath(path: string): string {
  const stamp = Date.now().toString(36);
  if (!path) return `output_rerun_${stamp}`;
  if (path.endsWith('/') || path.endsWith('\\')) {
    return `${path.replace(/[/\\]$/, '')}_rerun_${stamp}`;
  }
  return `${path}_rerun_${stamp}`;
}

export function pathsEqual(a: string, b: string): boolean {
  const norm = (p: string) => p.trim().replace(/[/\\]+$/, '').replace(/\\/g, '/').toLowerCase();
  return Boolean(a.trim()) && norm(a) === norm(b);
}

/** Extract numeric progress percent only when real completed/total exist. */
export function realProgressPercent(progress: unknown, status?: string): number | null {
  if (status === 'completed' || status === 'succeeded') return 100;
  if (progress && typeof progress === 'object') {
    const rec = progress as Record<string, unknown>;
    const completed = rec.completed;
    const total = rec.total;
    if (typeof completed === 'number' && typeof total === 'number' && total > 0) {
      return Math.max(0, Math.min(100, Math.round((completed / total) * 100)));
    }
    const raw = rec.percent ?? rec.pct;
    if (typeof raw === 'number' && !Number.isNaN(raw) && rec.total != null) {
      const n = raw <= 1 ? raw * 100 : raw;
      return Math.max(0, Math.min(100, Math.round(n)));
    }
  }
  return null;
}
