import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/desktop';

const entries = [
  { to: '/osgb/convert', title: 'OSGB 转换', desc: '倾斜摄影 OSGB → 3D Tiles' },
  { to: '/tiles/process', title: 'Tiles 处理', desc: '对已有 tileset 做顶层重建或纹理处理' },
  { to: '/preview/tiles', title: '预览', desc: '用 Cesium 打开本地 tileset' },
  { to: '/processing', title: '任务', desc: '查看进行中和排队的任务' },
  { to: '/results', title: '成果', desc: '已登记的输出目录' },
  { to: '/history', title: '记录', desc: '已完成任务与重跑' },
];

export function Workspace() {
  const [taskCount, setTaskCount] = useState<number | null>(null);
  const [artCount, setArtCount] = useState<number | null>(null);
  const [activeCount, setActiveCount] = useState<number | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [tasks, arts] = await Promise.all([api.listTasks(), api.listArtifacts()]);
        setTaskCount(tasks.length);
        setArtCount(arts.length);
        setActiveCount(
          tasks.filter((t) => t.status === 'running' || t.status === 'queued' || t.status === 'cancelling')
            .length,
        );
      } catch {
        /* ignore */
      }
    })();
  }, []);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>工作区</h1>
        </div>
        <Link className="btn btn-primary" to="/osgb/convert">
          新建转换
        </Link>
      </div>

      <p className="muted" style={{ marginBottom: 16 }}>
        进行中 {activeCount ?? '—'} · 任务 {taskCount ?? '—'} · 成果 {artCount ?? '—'}
      </p>

      <div className="card">
        <table className="table">
          <tbody>
            {entries.map((item) => (
              <tr key={item.to}>
                <td style={{ width: 140 }}>
                  <Link to={item.to}>{item.title}</Link>
                </td>
                <td className="muted">{item.desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
