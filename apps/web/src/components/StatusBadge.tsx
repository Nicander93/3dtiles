import type { TaskStatus } from '../api/types';

const labels: Record<string, string> = {
  queued: '排队中',
  running: '运行中',
  cancelling: '取消中',
  succeeded: '已完成',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
  interrupted: '已中断',
};

const cssKey: Record<string, string> = {
  queued: 'queued',
  running: 'running',
  cancelling: 'running',
  succeeded: 'completed',
  completed: 'completed',
  failed: 'failed',
  cancelled: 'cancelled',
  interrupted: 'cancelled',
};

export function StatusBadge({ status }: { status: TaskStatus | string }) {
  const key = cssKey[status] || 'queued';
  return <span className={`status-dot ${key}`}>{labels[status] ?? status}</span>;
}
