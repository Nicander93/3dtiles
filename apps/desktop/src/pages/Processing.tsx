import { useEffect, useMemo, useState } from 'react';
import { api, friendlyError, isActiveStatus, isDoneStatus } from '../api/desktop';
import type { Task, TaskStatus } from '../api/types';
import { Alert } from '../components/Alert';
import { EmptyState } from '../components/EmptyState';
import { StageStepper } from '../components/StageStepper';
import { StatusBadge } from '../components/StatusBadge';
import { useTasks } from '../hooks/useTasks';

type Tab = 'all' | 'running' | 'queued' | 'done';

const tabFilter: Record<Tab, (t: Task) => boolean> = {
  all: () => true,
  running: (t) => t.status === 'running' || t.status === 'cancelling',
  queued: (t) => t.status === 'queued',
  done: (t) => isDoneStatus(t.status),
};

function progressOf(t: Task): number {
  if (typeof t.progress === 'number' && !Number.isNaN(t.progress)) {
    const n = t.progress <= 1 ? t.progress * 100 : t.progress;
    return Math.max(0, Math.min(100, Math.round(n)));
  }
  if (t.progress && typeof t.progress === 'object') {
    const rec = t.progress as Record<string, unknown>;
    const raw = rec.percent ?? rec.pct ?? rec.value;
    if (typeof raw === 'number' && !Number.isNaN(raw)) {
      const n = raw <= 1 ? raw * 100 : raw;
      return Math.max(0, Math.min(100, Math.round(n)));
    }
  }
  if (t.status === 'completed' || t.status === 'succeeded') return 100;
  if (t.status === 'failed' || t.status === 'cancelled') return 100;
  if (t.status === 'queued') return 0;
  const stages = t.stages || [];
  if (stages.length) {
    const done = stages.filter((s) => s.status === 'done' || s.status === 'skipped').length;
    const running = stages.some((s) => s.status === 'running') ? 0.45 : 0;
    return Math.round(((done + running) / stages.length) * 100);
  }
  if (t.stage === 'done' || t.stage === 'check') return t.stage === 'done' ? 100 : 90;
  return 35;
}


function rebuildTopSummary(opts: Record<string, unknown> | undefined): string | null {
  const rt = opts?.rebuildTop as { enabled?: boolean; levels?: number } | undefined;
  if (!rt || rt.enabled !== true) return null;
  const levels = rt.levels === 2 ? 2 : 1;
  return `启用 · levels=${levels}`;
}

export function Processing() {
  const { tasks, loading, error, refresh } = useTasks(2000);
  const [tab, setTab] = useState<Tab>('all');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [cancellingId, setCancellingId] = useState<string | null>(null);
  const [logText, setLogText] = useState<string>('');
  const [logOpen, setLogOpen] = useState(true);

  const counts = useMemo(
    () => ({
      all: tasks.length,
      running: tasks.filter((t) => t.status === 'running' || t.status === 'cancelling').length,
      queued: tasks.filter((t) => t.status === 'queued').length,
      done: tasks.filter((t) => isDoneStatus(t.status)).length,
    }),
    [tasks],
  );

  const filtered = useMemo(() => tasks.filter(tabFilter[tab]), [tasks, tab]);
  const selected =
    filtered.find((t) => t.id === selectedId) ||
    tasks.find((t) => t.id === selectedId) ||
    filtered[0] ||
    null;

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

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>正在处理</h1>
          <p>任务队列与阶段进度（扫描 → 转换 → 重建 → 纹理 → 检查）</p>
        </div>
        <div className="toolbar processing-toolbar" style={{ marginBottom: 0 }}>
          <button className="btn" type="button" onClick={() => void refresh()}>
            刷新
          </button>
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

      <div className="tabs" style={{ marginBottom: 16 }}>
        {(
          [
            ['all', `全部 (${counts.all})`],
            ['running', `运行中 (${counts.running})`],
            ['queued', `排队中 (${counts.queued})`],
            ['done', `已完成 (${counts.done})`],
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

      <div className="split-2">
        <div className="task-list">
          {loading && tasks.length === 0 ? (
            <div className="card card-pad muted">加载中…</div>
          ) : filtered.length === 0 ? (
            <div className="card card-pad">
              <EmptyState title="暂无任务" description="提交 OSGB 转换或 process-tileset 后，任务会出现在这里。" />
            </div>
          ) : (
            filtered.map((t) => {
              const pct = progressOf(t);
              const barClass =
                t.status === 'failed' || t.status === 'cancelled'
                  ? 'failed'
                  : isDoneStatus(t.status)
                    ? 'done'
                    : '';
              return (
                <div
                  key={t.id}
                  className={`task-card${selected?.id === t.id ? ' active' : ''}`}
                  onClick={() => setSelectedId(t.id)}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') setSelectedId(t.id);
                  }}
                  role="button"
                  tabIndex={0}
                >
                  <div className="task-card-head">
                    <h3>{t.name || t.operation || t.id}</h3>
                    <StatusBadge status={t.status as TaskStatus} />
                  </div>
                  <div className="task-meta">
                    <span>阶段 · {t.stage || '—'}</span>
                    <span>输入 · {t.input || '—'}</span>
                  </div>
                  <div className={`progress-bar ${barClass}`}>
                    <i style={{ width: `${pct}%` }} />
                  </div>
                  <div className="progress-label">{pct}%</div>
                  <div className="task-card-foot" onClick={(e) => e.stopPropagation()}>
                    <button
                      className="btn btn-ghost"
                      type="button"
                      onClick={() => {
                        setSelectedId(t.id);
                        setLogOpen(true);
                      }}
                    >
                      查看日志
                    </button>
                    {isActiveStatus(t.status) ? (
                      <button
                        className="btn btn-danger"
                        type="button"
                        disabled={cancellingId === t.id || t.status === 'cancelling'}
                        onClick={() => void cancel(t.id)}
                      >
                        {t.status === 'cancelling' || cancellingId === t.id ? '取消中…' : '取消'}
                      </button>
                    ) : null}
                  </div>
                </div>
              );
            })
          )}
        </div>

        <div className="card card-pad">
          <div className="section-title">任务详情</div>
          {!selected ? (
            <EmptyState title="未选择任务" description="从左侧列表选择一项查看阶段。" />
          ) : (
            <>
              <div style={{ marginBottom: 12 }}>
                <div style={{ fontWeight: 650, fontSize: 15, marginBottom: 6 }}>
                  {selected.name || selected.operation || selected.id}
                </div>
                <StatusBadge status={selected.status} />
                {selected.status === 'cancelling' ? (
                  <span className="muted" style={{ marginLeft: 8 }}>
                    正在取消…
                  </span>
                ) : null}
              </div>

              <div className="section-title">处理阶段</div>
              <StageStepper stages={selected.stages} orientation="horizontal" />

              <div className="summary-box" style={{ marginTop: 16, marginBottom: 16 }}>
                <dl>
                  <dt>ID</dt>
                  <dd>{selected.id}</dd>
                  <dt>操作</dt>
                  <dd>{selected.operation}</dd>
                  <dt>阶段</dt>
                  <dd>{selected.stage || '—'}</dd>
                  <dt>输入</dt>
                  <dd>{selected.input}</dd>
                  <dt>输出</dt>
                  <dd>{selected.artifactPath || selected.output}</dd>
                  {(() => {
                    const rb = rebuildTopSummary(selected.options);
                    return rb ? (
                      <>
                        <dt>顶层重建</dt>
                        <dd>{rb}</dd>
                      </>
                    ) : null;
                  })()}
                  <dt>说明</dt>
                  <dd>{selected.message || selected.error || '—'}</dd>
                </dl>
              </div>

              <details
                open={logOpen}
                onToggle={(e) => setLogOpen((e.target as HTMLDetailsElement).open)}
              >
                <summary className="muted">执行日志{selected.logPath ? ` · ${selected.logPath}` : ''}</summary>
                <pre className="log-pre">{logText || selected.log || '（暂无日志）'}</pre>
              </details>
              {isActiveStatus(selected.status) && (
                <div style={{ marginTop: 16 }}>
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
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
