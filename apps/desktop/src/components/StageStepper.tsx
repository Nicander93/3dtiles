import type { TaskStageInfo } from '../api/types';

const defaults: TaskStageInfo[] = [
  { id: 'scan', label: '扫描', status: 'pending' },
  { id: 'convert', label: '转换', status: 'pending' },
  { id: 'rebuild', label: '顶层重建', status: 'pending' },
  { id: 'texture', label: '纹理', status: 'pending' },
  { id: 'check', label: '检查', status: 'pending' },
];

function statusMark(status: string): string {
  if (status === 'done') return '✓';
  if (status === 'running') return '…';
  if (status === 'failed') return '!';
  if (status === 'skipped') return '–';
  return '';
}

export function StageStepper({
  stages,
  orientation = 'vertical',
}: {
  stages?: TaskStageInfo[];
  orientation?: 'vertical' | 'horizontal';
}) {
  const items = stages && stages.length > 0 ? stages : defaults;

  if (orientation === 'horizontal') {
    return (
      <div className="stepper-h" role="list">
        {items.map((s, i) => (
          <div className="stepper-h-item" key={s.id} role="listitem">
            <div className="stepper-h-node">
              <div className={`stepper-h-dot ${s.status}`}>{statusMark(s.status)}</div>
              <div className="stepper-h-label">{s.label}</div>
            </div>
            {i < items.length - 1 ? (
              <div className={`stepper-h-line${s.status === 'done' || s.status === 'skipped' ? ' done' : ''}`} />
            ) : null}
          </div>
        ))}
      </div>
    );
  }

  return (
    <div className="stepper">
      {items.map((s, i) => (
        <div className="stepper-item" key={s.id}>
          <div className="stepper-rail">
            <div className={`stepper-dot ${s.status}`} />
            {i < items.length - 1 ? <div className="stepper-line" /> : null}
          </div>
          <div>
            <div style={{ fontWeight: 600, fontSize: 13 }}>{s.label}</div>
            <div className="muted" style={{ fontSize: 12 }}>
              {s.message ||
                (s.status === 'pending'
                  ? '等待中'
                  : s.status === 'running'
                    ? '进行中…'
                    : s.status === 'done'
                      ? '已完成'
                      : s.status === 'failed'
                        ? '失败'
                        : '跳过')}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
