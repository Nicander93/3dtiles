import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { api } from '../api/desktop';

const primaryEntries = [
  {
    id: 'osgb-convert',
    title: 'OSGB转换',
    desc: '倾斜摄影 OSGB → 3D Tiles，三步配置输入 / 处理 / 输出',
    to: '/osgb/convert',
    icon: '⇄',
    cta: '开始转换 →',
    primary: true,
  },
  {
    id: 'tiles-process',
    title: '3D Tiles 处理',
    desc: '对已有 tileset 做顶层重建 / 后处理',
    to: '/tiles/process',
    icon: '⚙',
    cta: '处理 tileset →',
    primary: true,
  },
];

const previewEntries = [
  {
    id: 'tiles-preview',
    title: '3D Tiles预览',
    desc: 'Cesium 加载本地成果 tileset',
    to: '/preview/tiles',
    icon: '◇',
    cta: '打开预览 →',
  },
];

const upcoming = [
  { id: 'generic-model', title: '通用模型', desc: 'FBX / OBJ 等', icon: '▢' },
  { id: 'imagery', title: '影像', desc: '正射影像切片', icon: '☁' },
  { id: 'terrain', title: '地形', desc: '地形瓦片生成', icon: '⛰' },
];

export function Workspace() {
  const [q, setQ] = useState('');
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

  const match = (title: string, desc: string) => {
    const s = q.trim().toLowerCase();
    if (!s) return true;
    return title.toLowerCase().includes(s) || desc.toLowerCase().includes(s);
  };

  const filteredPrimary = useMemo(
    () => primaryEntries.filter((t) => match(t.title, t.desc)),
    [q],
  );
  const filteredPreview = useMemo(
    () => previewEntries.filter((t) => match(t.title, t.desc)),
    [q],
  );
  const filteredUpcoming = useMemo(
    () => upcoming.filter((t) => match(t.title, t.desc)),
    [q],
  );

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>工作区</h1>
          <p>本地倾斜摄影处理与预览入口 — 选择工具开始</p>
        </div>
        <Link className="btn btn-primary" to="/osgb/convert">
          新建 OSGB 转换
        </Link>
      </div>

      <div className="page-with-aside">
        <div>
          <div className="search-bar">
            <input
              className="input"
              placeholder="搜索工具、功能或关键词…"
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
          </div>

          {filteredPrimary.length > 0 ? (
            <div className="entry-section">
              <div className="entry-section-head">
                <h2>处理入口</h2>
              </div>
              <div className="entry-grid">
                {filteredPrimary.map((t) => (
                  <Link
                    key={t.id}
                    to={t.to}
                    className={`entry-card${t.primary ? ' primary' : ''}`}
                  >
                    <div className="icon">{t.icon}</div>
                    <h3>{t.title}</h3>
                    <p>{t.desc}</p>
                    <span className="entry-cta">{t.cta}</span>
                  </Link>
                ))}
              </div>
            </div>
          ) : null}

          {filteredPreview.length > 0 ? (
            <div className="entry-section">
              <div className="entry-section-head">
                <h2>预览</h2>
              </div>
              <div className="entry-grid">
                {filteredPreview.map((t) => (
                  <Link key={t.id} to={t.to} className="entry-card">
                    <div className="icon">{t.icon}</div>
                    <h3>{t.title}</h3>
                    <p>{t.desc}</p>
                    <span className="entry-cta">{t.cta}</span>
                  </Link>
                ))}
              </div>
            </div>
          ) : null}

          {filteredUpcoming.length > 0 ? (
            <div className="entry-section">
              <div className="entry-section-head">
                <h2>即将推出</h2>
              </div>
              <div className="entry-grid">
                {filteredUpcoming.map((t) => (
                  <div key={t.id} className="entry-card disabled" title="即将推出">
                    <div className="icon">{t.icon}</div>
                    <h3>{t.title}</h3>
                    <p>{t.desc}</p>
                    <span className="coming">即将推出</span>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </div>

        <aside className="aside-stack">
          <div className="card card-pad">
            <div className="section-title">快捷状态</div>
            <div className="stat-grid">
              <div className="stat-chip">
                <div className="k">进行中</div>
                <div className="v">{activeCount != null ? activeCount : '—'}</div>
              </div>
              <div className="stat-chip">
                <div className="k">全部任务</div>
                <div className="v">{taskCount != null ? taskCount : '—'}</div>
              </div>
              <div className="stat-chip">
                <div className="k">处理成果</div>
                <div className="v">{artCount != null ? artCount : '—'}</div>
              </div>
              <div className="stat-chip">
                <div className="k">模式</div>
                <div className="v">本地</div>
              </div>
            </div>
          </div>
          <div className="card card-pad">
            <div className="section-title">常用入口</div>
            <div className="list-item">
              <Link to="/osgb/convert">OSGB转换</Link>
              <span className="muted">三栏配置</span>
            </div>
            <div className="list-item">
              <Link to="/tiles/process">Tiles处理</Link>
              <span className="muted">重建</span>
            </div>
            <div className="list-item">
              <Link to="/processing">正在处理</Link>
              <span className="muted">{activeCount != null ? `${activeCount}` : '—'}</span>
            </div>
            <div className="list-item">
              <Link to="/preview/tiles">Tiles预览</Link>
              <span className="muted">Cesium</span>
            </div>
            <div className="list-item">
              <Link to="/results">处理成果</Link>
              <span className="muted">{artCount != null ? `${artCount}` : '—'}</span>
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}
