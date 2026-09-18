export function friendlyError(err: unknown): string {
  if (err instanceof Error && err.message.trim()) {
    return enhanceErrorMessage(err.message);
  }
  if (typeof err === 'string' && err.trim()) {
    return enhanceErrorMessage(err);
  }
  if (err && typeof err === 'object') {
    const value = err as Record<string, unknown>;
    if (typeof value.message === 'string' && value.message.trim()) {
      return enhanceErrorMessage(value.message);
    }
    if (typeof value.error === 'string' && value.error.trim()) {
      return enhanceErrorMessage(value.error);
    }
    if (typeof value.detail === 'string' && value.detail.trim()) {
      return enhanceErrorMessage(value.detail);
    }
    if (value.detail && typeof value.detail === 'object') {
      const detail = value.detail as Record<string, unknown>;
      if (typeof detail.message === 'string' && detail.message.trim()) {
        return enhanceErrorMessage(detail.message);
      }
    }
  }
  return '发生未知错误，请查看任务日志。';
}

function enhanceErrorMessage(msg: string): string {
  if (msg.includes('GRID_SPATIAL_MISMATCH') || msg.includes('spatial mismatch')) {
    return '稀疏网格不支持 · 原因：数据块不连续或分布不规则 · 您可以：(1) 使用"仅转换"跳过顶层重建，或 (2) 切换到连续规则网格数据（≤16×16）';
  }
  return msg;
}
