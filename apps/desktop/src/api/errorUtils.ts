export function friendlyError(err: unknown): string {
  if (err instanceof Error && err.message.trim()) return err.message;
  if (typeof err === 'string' && err.trim()) return err;
  if (err && typeof err === 'object') {
    const value = err as Record<string, unknown>;
    if (typeof value.message === 'string' && value.message.trim()) return value.message;
    if (typeof value.detail === 'string' && value.detail.trim()) return value.detail;
    if (value.detail && typeof value.detail === 'object') {
      const detail = value.detail as Record<string, unknown>;
      if (typeof detail.message === 'string' && detail.message.trim()) return detail.message;
    }
  }
  return '发生未知错误，请稍后重试。';
}
