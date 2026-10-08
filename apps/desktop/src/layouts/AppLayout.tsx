import { useEffect, useState } from 'react';
import { Link, NavLink, Outlet, useLocation } from 'react-router-dom';
import {
  CaretDoubleLeft,
  CaretDoubleRight,
  CaretRight,
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
  const toolPage = !['/', '/processing', '/history', '/results', '/settings'].includes(
    location.pathname,
  );
  const titles: Record<string, string> = {
    '/': '工具',
    '/processing': '任务',
    '/history': '任务',
    '/results': '成果',
    '/settings': '设置',
    '/osgb/convert': 'OSGB 转换',
    '/model/convert': '通用模型转换',
    '/tiles/merge': '3D Tiles 合并',
    '/tiles/clip': '范围裁剪',
    '/preview/tiles': '预览与编辑',
    '/tiles/process': '3D Tiles 优化',
  };
  const processTitle = new URLSearchParams(location.search).get('op');
  const title =
    location.pathname === '/tiles/process'
      ? processTitle === 'rebuild'
        ? '顶层重建'
        : processTitle === 'texture'
          ? '纹理压缩'
          : '3D Tiles 优化'
      : titles[location.pathname] || 'GeoForge 3D';

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
            <img src="/brand/geoforge-logo-v4.png" alt="" />
          </div>
          <div className="brand-text">
            <strong>GeoForge 3D</strong>
          </div>
        </div>

        <nav className="nav" aria-label="主导航">
          {mainNav.map((item) => {
            const Icon = item.icon;
            return (
              <Link
                key={item.to}
                to={item.to}
                title={item.label}
                aria-label={item.label}
                className={`nav-item${location.pathname === item.to || (item.to === '/' && toolPage) ? ' active' : ''}`}
                aria-current={location.pathname === item.to || (item.to === '/' && toolPage) ? 'page' : undefined}
              >
                <span className="nav-icon">
                  <Icon size={18} weight="regular" />
                </span>
                <span className="nav-label">{item.label}</span>
                {item.badge && activeCount > 0 ? (
                  <span className="nav-badge">{activeCount}</span>
                ) : null}
              </Link>
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
        <header className="topbar">
          <nav className="crumbs" aria-label="当前位置">
            {toolPage ? (
              <>
                <Link to="/">工具</Link>
                <CaretRight size={12} aria-hidden />
              </>
            ) : null}
            <strong>{title}</strong>
          </nav>
          <Link className="btn btn-ghost btn-sm" to="/settings">
            设置与帮助
          </Link>
        </header>
        <div className={`workspace${isPreview ? ' workspace--preview' : ''}`}>
          <Outlet />
        </div>
      </main>
    </div>
  );
}
