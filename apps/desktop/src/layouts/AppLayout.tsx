import { NavLink, Outlet } from 'react-router-dom';
import { isActiveStatus } from '../api/desktop';
import { useTasks } from '../hooks/useTasks';
import { isTauri } from '../lib/tauri';

type NavItem = {
  to: string;
  label: string;
  end?: boolean;
  icon: string;
  badge?: boolean;
};

type NavGroup = { title: string; items: NavItem[] };

const navGroups: NavGroup[] = [
  {
    title: '浏览',
    items: [
      { to: '/', label: '工作区', end: true, icon: '⌂' },
      { to: '/preview/tiles', label: '3D Tiles预览', icon: '◇' },
    ],
  },
  {
    title: '处理',
    items: [
      { to: '/osgb/convert', label: 'OSGB转换', icon: '⇄' },
      { to: '/processing', label: '正在处理', icon: '⟳', badge: true },
      { to: '/tiles/process', label: 'Tiles处理', icon: '▣' },
      { to: '/history', label: '处理记录', icon: '☰' },
      { to: '/results', label: '处理成果', icon: '▤' },
    ],
  },
  {
    title: '系统',
    items: [{ to: '/settings', label: '设置与帮助', icon: '⚙' }],
  },
];

export function AppLayout() {
  const { tasks } = useTasks(5000);
  const activeCount = tasks.filter((t) => isActiveStatus(t.status)).length;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">GF</div>
          <div className="brand-text">
            <strong>GeoForge 3D</strong>
            <span>本地 · 高效 · 开放</span>
          </div>
        </div>

        <nav className="nav">
          {navGroups.map((group) => (
            <div className="nav-group" key={group.title}>
              <div className="nav-group-title">{group.title}</div>
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.end}
                  className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}
                >
                  <span className="nav-icon">{item.icon}</span>
                  <span className="nav-label">{item.label}</span>
                  {item.badge && activeCount > 0 ? (
                    <span className="nav-badge" title={`${activeCount} 个进行中`}>
                      {activeCount}
                    </span>
                  ) : null}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-foot">
          <div className="sidebar-promo">
            <div className="sidebar-promo-title">让三维数据</div>
            <div className="sidebar-promo-sub">在本地创造更多可能</div>
          </div>
          <div className="brand-meta" style={{ marginTop: 10 }}>
            <span className="pill">v0.1.0</span>
            <span className="pill accent">● 本地模式</span>
          </div>
          <div className="sidebar-api-hint">
            {isTauri() ? '桌面壳 · Tauri' : '浏览器'} · API 127.0.0.1:8787
          </div>
        </div>
      </aside>

      <main className="main">
        <Outlet />
      </main>
    </div>
  );
}
