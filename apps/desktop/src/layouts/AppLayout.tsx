import { useEffect, useState } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router-dom';
import {
  CaretDoubleLeft,
  CaretDoubleRight,
  Cube,
  Folder,
  GearSix,
  SquaresFour,
  ListChecks,
} from '@phosphor-icons/react';
import { isActiveStatus } from '../api/desktop';
import { useTasks } from '../hooks/useTasks';

const SIDEBAR_KEY = 'geoforge.sidebar.collapsed';

type NavItem = {
  to: string;
  label: string;
  end?: boolean;
  badge?: boolean;
  icon: typeof SquaresFour;
};

const mainNav: NavItem[] = [
  { to: '/', label: '工具', end: true, icon: SquaresFour },
  { to: '/processing', label: '任务', badge: true, icon: ListChecks },
  { to: '/results', label: '成果', icon: Folder },
];

export function AppLayout() {
  const { tasks } = useTasks(5000);
  const activeCount = tasks.filter((t) => isActiveStatus(t.status)).length;
  const location = useLocation();
  const isPreview = location.pathname.startsWith('/preview');

  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem(SIDEBAR_KEY) === '1';
    } catch {
      return false;
    }
  });

  useEffect(() => {
    try {
      localStorage.setItem(SIDEBAR_KEY, collapsed ? '1' : '0');
    } catch {
      /* ignore */
    }
  }, [collapsed]);

  const effectiveCollapsed = isPreview || collapsed;

  return (
    <div className="app-shell">
      <aside className={`sidebar${effectiveCollapsed ? ' collapsed' : ''}`}>
        <div className="brand">
          <div className="brand-mark" aria-hidden>
            <Cube size={22} weight="regular" />
          </div>
          <div className="brand-text">
            <strong>GeoForge 3D</strong>
          </div>
        </div>

        <nav className="nav" aria-label="主导航">
          {mainNav.map((item) => {
            const Icon = item.icon;
            return (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.end}
                title={item.label}
                aria-label={item.label}
                className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}
              >
                <span className="nav-icon">
                  <Icon size={18} weight="regular" />
                </span>
                <span className="nav-label">{item.label}</span>
                {item.badge && activeCount > 0 ? (
                  <span className="nav-badge">{activeCount}</span>
                ) : null}
              </NavLink>
            );
          })}
        </nav>

        <div className="sidebar-foot">
          <NavLink
            to="/settings"
            title="设置"
            aria-label="设置"
            className={({ isActive }) => `nav-item${isActive ? ' active' : ''}`}
          >
            <span className="nav-icon">
              <GearSix size={18} weight="regular" />
            </span>
            <span className="nav-label">设置</span>
          </NavLink>
          {!isPreview ? (
            <button
              type="button"
              className="nav-item"
              title={collapsed ? '展开侧栏' : '收起侧栏'}
              aria-label={collapsed ? '展开侧栏' : '收起侧栏'}
              onClick={() => setCollapsed((v) => !v)}
            >
              <span className="nav-icon">
                {collapsed ? (
                  <CaretDoubleRight size={18} weight="regular" />
                ) : (
                  <CaretDoubleLeft size={18} weight="regular" />
                )}
              </span>
              <span className="nav-label">{collapsed ? '展开' : '收起'}</span>
            </button>
          ) : null}
        </div>
      </aside>

      <main className="main">
        <Outlet />
      </main>
    </div>
  );
}
