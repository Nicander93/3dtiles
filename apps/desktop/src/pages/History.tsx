import { Fragment, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api, friendlyError, isDoneStatus } from '../api/desktop';
import type { Task, TaskStatus } from '../api/types';
import { Alert } from '../components/Alert';
import { EmptyState } from '../components/EmptyState';
import { StatusBadge } from '../components/StatusBadge';
import { useTasks } from '../hooks/useTasks';

function fmtTime(v?: string | number) {
  if (v == null || v === '') return '—';
  const d = typeof v === 'number' ? new Date(v * (v < 1e12 ? 1000 : 1)) : new Date(v);
  if (Number.isNaN(d.getTime())) return String(v);
  return d.toLocaleString('zh-CN', { timeZone: 'Asia/Shanghai' });
}

function cloneOutput(path: string): string {
  const stamp = Date.now().toString(36);
  if (!path) return `/workspace/data/geoforge_outputs/rerun_${stamp}`;
  // Avoid clobbering prior output; append suffix
  if (path.endsWith('/')) return `${path.replace(/\/$/, '')}_rerun_${stamp}`;
  return `${path}_rerun_${stamp}`;
}

export function History() {
  const { tasks, loading, error, refresh } = useTasks(5000);
  const navigate = useNavigate();
  const history = tasks.filter((t) => isDoneStatus(t.status));
  const [actionError, setActionError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [logs, setLogs] = useState<Record<string, string>>({});

  async function rerun(t: Task) {
    setActionError(null);
    setBusyId(t.id);
    try {
      const out = cloneOutput(t.output);
      const res = await api.createTask({
        operation: t.operation || 'convert-osgb',
        input: { path: t.input },
        output: { path: out },
        taskName: `${t.name || t.id} (重跑)`,
        options: t.options || {},
      });
      const id = res.task?.id || res.id;
      await refresh();
      navigate(id ? `/processing` : '/processing');
    } catch (e) {
      setActionError(friendlyError(e));
    } finally {
      setBusyId(null);
    }
  }

  async function toggleLogs(t: Task) {
    if (expandedId === t.id) {
      setExpandedId(null);
      return;
    }
    setExpandedId(t.id);
    if (logs[t.id]) return;
    try {
      if (t.log) {
        setLogs((m) => ({ ...m, [t.id]: t.log || '' }));
        return;
      }
      const data = await api.getTaskLogs(t.id);
      setLogs((m) => ({ ...m, [t.id]: data.log || (data.lines || []).join('\n') }));
    } catch (e) {
      setLogs((m) => ({ ...m, [t.id]: friendlyError(e) }));
    }
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>记录</h1>
        </div>
        <button className="btn" type="button" onClick={() => void refresh()}>
          刷新
        </button>
      </div>

      {error ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="warn">{error}</Alert>
        </div>
      ) : null}
      {actionError ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="error">{actionError}</Alert>
        </div>
      ) : null}

      <div className="card" style={{ overflow: 'auto' }}>
        {loading && history.length === 0 ? (
          <div className="empty muted">加载中…</div>
        ) : history.length === 0 ? (
          <EmptyState title="暂无处理记录" description="完成、失败或取消的任务会显示在此表中。" />
        ) : (
          <table className="table">
            <thead>
              <tr>
                <th>任务</th>
                <th>操作</th>
                <th>状态</th>
                <th>阶段</th>
                <th>输入</th>
                <th>输出</th>
                <th>时间</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {history.map((t) => (
                <Fragment key={t.id}>
                  <tr>
                    <td>{t.name || t.id}</td>
                    <td>{t.operation}</td>
                    <td>
                      <StatusBadge status={t.status as TaskStatus} />
                    </td>
                    <td className="muted">{t.stage || '—'}</td>
                    <td className="muted">{t.input}</td>
                    <td className="muted">{t.artifactPath || t.output}</td>
                    <td className="muted">{fmtTime(t.updatedAt || t.createdAt)}</td>
                    <td>
                      <div className="row wrap" style={{ gap: 8 }}>
                        <button className="btn" type="button" onClick={() => void toggleLogs(t)}>
                          {expandedId === t.id ? '收起日志' : '日志'}
                        </button>
                        <button
                          className="btn btn-primary"
                          type="button"
                          disabled={busyId === t.id || !t.input}
                          onClick={() => void rerun(t)}
                        >
                          {busyId === t.id ? '提交中…' : '重新执行'}
                        </button>
                      </div>
                    </td>
                  </tr>
                  {expandedId === t.id ? (
                    <tr>
                      <td colSpan={8}>
                        <pre className="log-pre">{logs[t.id] || t.log || '（无日志）'}</pre>
                        {t.logPath ? (
                          <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>
                            logPath: {t.logPath}
                          </div>
                        ) : null}
                      </td>
                    </tr>
                  ) : null}
                </Fragment>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
