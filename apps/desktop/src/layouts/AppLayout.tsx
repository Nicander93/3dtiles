import { NavLink, Outlet } from 'react-router-dom';
import { isActiveStatus } from '../api/desktop';
import { useTasks } from '../hooks/useTasks';

type NavItem = {
  to: string;
  label: string;
  end?: boolean;
  badge?: boolean;
};

type NavGroup = { title: string; items: NavItem[] };

const navGroups: NavGroup[] = [
  {
    title: '浏览',
    items: [
      { to: '/', label: '工作区', end: true },
      { to: '/preview/tiles', label: '预览' },
    ],
  },
  {
    title: '处理',
    items: [
      { to: '/osgb/convert', label: 'OSGB 转换' },
      { to: '/processing', label: '任务', badge: true },
      { to: '/tiles/process', label: 'Tiles 处理' },
      { to: '/history', label: '记录' },
      { to: '/results', label: '成果' },
    ],
  },
  {
    title: '系统',
    items: [{ to: '/settings', label: '设置' }],
  },
];

export function AppLayout() {
  const { tasks } = useTasks(5000);
  const activeCount = tasks.filter((t) => isActiveStatus(t.status)).length;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-text">
            <strong>GeoForge 3D</strong>
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
                  <span className="nav-label">{item.label}</span>
                  {item.badge && activeCount > 0 ? (
                    <span className="nav-badge">{activeCount}</span>
                  ) : null}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
      </aside>

      <main className="main">
        <Outlet />
      </main>
    </div>
  );
}
