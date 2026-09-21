import { useEffect, useMemo, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router-dom';
import {
  CheckCircle,
  Clock,
  Cube,
  Image,
  Stack,
  XCircle,
} from '@phosphor-icons/react';
import { api, friendlyError, isActiveStatus, isDoneStatus } from '../api/desktop';
import type { Task, TaskStatus } from '../api/types';
import { Alert } from '../components/Alert';
import { EmptyState } from '../components/EmptyState';
import { StageStepper } from '../components/StageStepper';
import { StatusBadge } from '../components/StatusBadge';
import { useTasks } from '../hooks/useTasks';
import { cloneOutputPath, realProgressPercent } from '../lib/formUtils';

type StatusFilter = 'all' | 'active' | 'done' | 'failed';

const statusFilter: Record<StatusFilter, (t: Task) => boolean> = {
  all: () => true,
  active: (t) => t.status === 'running' || t.status === 'queued' || t.status === 'cancelling',
  done: (t) => t.status === 'completed' || t.status === 'succeeded',
  failed: (t) => t.status === 'failed' || t.status === 'cancelled' || t.status === 'interrupted',
};

function opLabel(op?: string): string {
  if (op === 'convert-osgb') return 'OSGB 转换';
  if (op === 'convert-model') return '通用模型转换';
  if (op === 'process-tileset') return 'Tiles 处理';
  return op || '—';
}

function TaskIcon({ operation }: { operation?: string }) {
  if (operation === 'convert-osgb') return <Stack size={18} />;
  if (operation === 'convert-model') return <Cube size={18} />;
  if (operation === 'process-tileset') return <Cube size={18} />;
  return <Image size={18} />;
}

function fmtTime(v?: string | number) {
  if (v == null || v === '') return '—';
  const d = typeof v === 'number' ? new Date(v * (v < 1e12 ? 1000 : 1)) : new Date(v);
  if (Number.isNaN(d.getTime())) return String(v);
  return d.toLocaleString('zh-CN', { timeZone: 'Asia/Shanghai' });
}

function statusLine(t: Task): { text: string; pct: number | null } {
  const pct = realProgressPercent(t.progress, t.status);
  if (t.status === 'running' || t.status === 'cancelling') {
    const stage = t.stage ? String(t.stage) : '处理中';
    if (pct != null) return { text: `${stage} · ${pct}%`, pct };
    return { text: stage, pct: null };
  }
  if (t.status === 'queued') return { text: '排队中', pct: null };
  if (t.status === 'completed' || t.status === 'succeeded') return { text: '已完成', pct: 100 };
  if (t.status === 'failed') {
    const reason = t.error || t.message;
    return { text: reason ? `失败 · ${reason}` : '失败', pct: null };
  }
  if (t.status === 'cancelled') return { text: '已取消', pct: null };
  if (t.status === 'interrupted') return { text: '已中断', pct: null };
  return { text: t.status, pct };
}

function StatusIcon({ status }: { status: string }) {
  if (status === 'running' || status === 'cancelling') return <Clock size={16} />;
  if (status === 'queued') return <Clock size={16} />;
  if (status === 'completed' || status === 'succeeded') return <CheckCircle size={16} color="var(--success)" />;
  if (status === 'failed' || status === 'cancelled' || status === 'interrupted') {
    return <XCircle size={16} color="var(--danger)" />;
  }
  return null;
}

function rebuildHref(t: Task): string {
  const params = new URLSearchParams();
  if (t.input) params.set('input', t.input);
  if (t.output) params.set('output', cloneOutputPath(t.output));
  if (t.name) params.set('name', t.name);
  const q = params.toString();
  if (t.operation === 'process-tileset') return `/tiles/process?${q}`;
  if (t.operation === 'convert-model') return `/model/convert?${q}`;
  return `/osgb/convert?${q}`;
}

function diagnosticPaths(t: Task): string[] {
  if (!t.progress || typeof t.progress !== 'object') return [];
  const progress = t.progress as Record<string, unknown>;
  return ['stderrLogPath', 'stdoutLogPath']
    .map((key) => progress[key])
    .filter((value): value is string => typeof value === 'string' && value.length > 0);
}

function progressText(t: Task, key: string): string | null {
  if (!t.progress || typeof t.progress !== 'object') return null;
  const value = (t.progress as Record<string, unknown>)[key];
  return typeof value === 'string' && value.trim() ? value : null;
}

function failureSuggestion(code: string | null): string | null {
  if (!code) return null;
  if (code === 'PATH_OUTPUT_EXISTS') return '请更换一个尚不存在的成果目录。';
  if (code === 'PATH_OUTPUT_NOT_WRITABLE') return '请检查输出目录权限，或选择可写磁盘。';
  if (code === 'CONVERTER_EXIT_NONZERO') return '查看 stderr 诊断文件，确认输入数据和 converter 依赖。';
  if (code === 'TASK_CANCELLED') return '任务已取消；原始输入和既有成果应保持不变。';
  return '请查看完整日志和诊断文件，再决定是否重试。';
}

export function Processing() {
  const { tasks, loading, error, refresh } = useTasks(2000);
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const [tab, setTab] = useState<StatusFilter>('all');
  const [typeFilter, setTypeFilter] = useState<string>('all');
  const [selectedId, setSelectedId] = useState<string | null>(searchParams.get('task'));
  const [actionError, setActionError] = useState<string | null>(null);
  const [cancellingId, setCancellingId] = useState<string | null>(null);
  const [logText, setLogText] = useState('');
  const [logOpen, setLogOpen] = useState(false);
  const [query, setQuery] = useState('');

  useEffect(() => {
    const id = searchParams.get('task');
    if (id) setSelectedId(id);
  }, [searchParams]);

  const filtered = useMemo(() => {
    let list = tasks.filter(statusFilter[tab]);
    if (typeFilter !== 'all') {
      list = list.filter((t) => t.operation === typeFilter);
    }
    const q = query.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (t) =>
          (t.name || '').toLowerCase().includes(q) ||
          (t.operation || '').toLowerCase().includes(q) ||
          (t.input || '').toLowerCase().includes(q),
      );
    }
    return list;
  }, [tasks, tab, typeFilter, query]);

  const selected = selectedId
    ? tasks.find((t) => t.id === selectedId) || null
    : null;
  const selectedErrorCode = selected ? progressText(selected, 'errorCode') : null;
  const selectedFailedStage = selected ? progressText(selected, 'failedStage') : null;
  const selectedSuggestion = failureSuggestion(selectedErrorCode);

  useEffect(() => {
    if (!selected) {
      setLogText('');
      return;
    }
    if (selected.log) {
      setLogText(selected.log);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const data = await api.getTaskLogs(selected.id);
        if (!cancelled) setLogText(data.log || (data.lines || []).join('\n'));
      } catch {
        if (!cancelled) setLogText('');
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [selected?.id, selected?.log, selected?.updatedAt]);

  async function cancel(id: string) {
    setActionError(null);
    setCancellingId(id);
    try {
      await api.cancelTask(id);
      await refresh();
    } catch (e) {
      setActionError(friendlyError(e));
    } finally {
      setCancellingId(null);
    }
  }

  function selectTask(id: string | null) {
    setSelectedId(id);
    setLogOpen(false);
    if (id) {
      setSearchParams({ task: id }, { replace: true });
    } else {
      setSearchParams({}, { replace: true });
    }
  }

  function rowActions(t: Task) {
    if (isActiveStatus(t.status)) {
      return (
        <button
          className="btn btn-sm"
          type="button"
          disabled={cancellingId === t.id || t.status === 'cancelling'}
          onClick={(e) => {
            e.stopPropagation();
            void cancel(t.id);
          }}
        >
          {t.status === 'cancelling' || cancellingId === t.id ? '取消中…' : '取消'}
        </button>
      );
    }
    if (t.status === 'completed' || t.status === 'succeeded') {
      return (
        <div className="row wrap" style={{ gap: 6 }} onClick={(e) => e.stopPropagation()}>
          {t.artifactId || t.output ? (
            <Link className="btn btn-sm" to="/results">
              查看成果
            </Link>
          ) : null}
          <button className="btn btn-sm" type="button" onClick={() => navigate(rebuildHref(t))}>
            重新处理
          </button>
        </div>
      );
    }
    if (t.status === 'failed' || t.status === 'cancelled' || t.status === 'interrupted') {
      return (
        <div className="row wrap" style={{ gap: 6 }} onClick={(e) => e.stopPropagation()}>
          <button className="btn btn-sm" type="button" onClick={() => selectTask(t.id)}>
            查看详情
          </button>
          <button className="btn btn-sm" type="button" onClick={() => navigate(rebuildHref(t))}>
            重新处理
          </button>
        </div>
      );
    }
    return null;
  }

  return (
    <div className="page" style={{ display: 'flex', flexDirection: 'column' }}>
      <div className="page-header full-width">
        <div>
          <h1>任务</h1>
        </div>
        <div className="search-wrap">
          <input
            className="input"
            placeholder="搜索任务…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="搜索任务"
            style={{ paddingLeft: 10 }}
          />
        </div>
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

      <div className="filter-row">
        <span className="filter-row__label">状态</span>
        <div className="tabs">
          {(
            [
              ['all', '全部'],
              ['active', '进行中'],
              ['done', '已完成'],
              ['failed', '失败'],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              className={`tab${tab === id ? ' active' : ''}`}
              onClick={() => setTab(id)}
            >
              {label}
            </button>
          ))}
        </div>
        <span className="filter-row__label">类型</span>
        <select
          className="select"
          style={{ width: 160 }}
          value={typeFilter}
          onChange={(e) => setTypeFilter(e.target.value)}
        >
          <option value="all">全部</option>
          <option value="convert-osgb">OSGB 转换</option>
          <option value="convert-model">通用模型转换</option>
          <option value="process-tileset">Tiles 处理</option>
        </select>
      </div>

      <div className="split-drawer">
        <div className="split-drawer__list">
          {loading && tasks.length === 0 ? (
            <div className="muted">加载中…</div>
          ) : filtered.length === 0 ? (
            tasks.length === 0 ? (
              <div>
                <EmptyState title="还没有任务" description="选择一个工具开始处理。" />
                <div style={{ marginTop: 12, textAlign: 'center' }}>
                  <Link className="btn btn-primary" to="/">
                    选择工具
                  </Link>
                </div>
              </div>
            ) : (
              <p className="muted">没有符合筛选条件的任务。</p>
            )
          ) : (
            <div className="task-table-wrap">
              <table className="table">
                <thead>
                  <tr>
                    <th>名称</th>
                    <th>类型</th>
                    <th>状态</th>
                    <th>时间</th>
                    <th>操作</th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.map((t) => {
                    const line = statusLine(t);
                    return (
                      <tr
                        key={t.id}
                        onClick={() => selectTask(t.id)}
                        style={{
                          cursor: 'pointer',
                          background: selected?.id === t.id ? 'var(--surface-selected)' : undefined,
                        }}
                      >
                        <td>
                          <div className="task-name">
                            <span className="task-name__icon" aria-hidden>
                              <TaskIcon operation={t.operation} />
                            </span>
                            {t.name || t.operation || t.id}
                          </div>
                        </td>
                        <td className="muted">{opLabel(t.operation)}</td>
                        <td>
                          <div className="task-status-cell">
                            <span className="row" style={{ gap: 6 }}>
                              <StatusIcon status={t.status} />
                              <span>{line.text}</span>
                            </span>
                            {(t.status === 'running' || t.status === 'cancelling') && (
                              <div
                                className={`progress-bar${line.pct == null ? ' indeterminate' : ''}`}
                              >
                                <i style={{ width: `${line.pct ?? 40}%` }} />
                              </div>
                            )}
                          </div>
                        </td>
                        <td className="muted">{fmtTime(t.updatedAt || t.createdAt)}</td>
                        <td>{rowActions(t)}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {selected ? (
          <aside className="split-drawer__panel">
            <div className="drawer-head">
              <div>
                <h2>{selected.name || selected.operation || selected.id}</h2>
                <div style={{ marginTop: 6 }}>
                  <StatusBadge status={selected.status as TaskStatus} />
                </div>
              </div>
              <button className="btn btn-ghost btn-sm" type="button" onClick={() => selectTask(null)}>
                关闭
              </button>
            </div>

            <div className="section-title">处理阶段</div>
            <StageStepper stages={selected.stages} orientation="horizontal" />

            <div className="summary-box" style={{ marginTop: 16, marginBottom: 16 }}>
              <dl>
                <dt>操作</dt>
                <dd>{opLabel(selected.operation)}</dd>
                <dt>阶段</dt>
                <dd>{selected.stage || '—'}</dd>
                <dt>输入</dt>
                <dd>{selected.input || '—'}</dd>
                <dt>输出</dt>
                <dd>{selected.artifactPath || selected.output || '—'}</dd>
                <dt>说明</dt>
                <dd>{selected.message || selected.error || '—'}</dd>
                {selectedErrorCode ? (
                  <>
                    <dt>错误代码</dt>
                    <dd>{selectedErrorCode}</dd>
                  </>
                ) : null}
                {selectedFailedStage ? (
                  <>
                    <dt>失败阶段</dt>
                    <dd>{selectedFailedStage}</dd>
                  </>
                ) : null}
                {selectedSuggestion && selected.status !== 'succeeded' ? (
                  <>
                    <dt>建议</dt>
                    <dd>{selectedSuggestion}</dd>
                  </>
                ) : null}
                {diagnosticPaths(selected).length ? (
                  <>
                    <dt>诊断日志</dt>
                    <dd>
                      {diagnosticPaths(selected).map((path) => (
                        <div key={path} style={{ wordBreak: 'break-all' }}>{path}</div>
                      ))}
                    </dd>
                  </>
                ) : null}
              </dl>
            </div>

            <details open={logOpen} onToggle={(e) => setLogOpen((e.target as HTMLDetailsElement).open)}>
              <summary className="muted" style={{ cursor: 'pointer' }}>
                执行日志
              </summary>
              <pre className="log-pre">{logText || selected.log || '（暂无日志）'}</pre>
            </details>

            <div className="actions">
              {isActiveStatus(selected.status) ? (
                <button
                  className="btn btn-danger"
                  type="button"
                  disabled={cancellingId === selected.id || selected.status === 'cancelling'}
                  onClick={() => void cancel(selected.id)}
                >
                  {selected.status === 'cancelling' || cancellingId === selected.id
                    ? '取消中…'
                    : '取消任务'}
                </button>
              ) : null}
              {isDoneStatus(selected.status) ? (
                <button className="btn" type="button" onClick={() => navigate(rebuildHref(selected))}>
                  重新处理
                </button>
              ) : null}
            </div>
          </aside>
        ) : null}
      </div>
    </div>
  );
}
