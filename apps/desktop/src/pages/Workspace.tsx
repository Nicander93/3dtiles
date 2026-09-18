import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { MagnifyingGlass } from '@phosphor-icons/react';
import { filterToolGroups } from '../lib/tools';

export function Workspace() {
  const [query, setQuery] = useState('');
  const groups = useMemo(() => filterToolGroups(query), [query]);

  return (
    <div className="page">
      <div className="page-header full-width">
        <div>
          <h1>工具</h1>
        </div>
        <div className="search-wrap">
          <span className="search-wrap__icon" aria-hidden>
            <MagnifyingGlass size={16} />
          </span>
          <input
            className="input"
            placeholder="搜索工具…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="搜索工具"
          />
        </div>
      </div>

      <div
        style={{
          padding: '12px 16px',
          marginBottom: 16,
          backgroundColor: 'var(--color-bg-secondary)',
          borderRadius: 6,
          border: '1px solid var(--color-border)',
        }}
      >
        <div style={{ fontSize: 14, fontWeight: 500, marginBottom: 4 }}>M1 试用版</div>
        <div style={{ fontSize: 13, color: 'var(--color-text-secondary)', lineHeight: 1.5 }}>
          当前版本：Windows · 连续规则网格 ≤16×16 · keep 纹理推荐 · 建议并发 1 worker
          <br />
          不支持：稀疏数据、任意城区、生产规模部署
        </div>
      </div>

      {groups.length === 0 ? (
        <p className="muted">未找到工具</p>
      ) : (
        groups.map((group) => (
          <section className="tool-group" key={group.id}>
            <h2 className="tool-group__title">{group.title}</h2>
            <div className="tool-grid">
              {group.tools.map((tool) => {
                const Icon = tool.icon;
                return (
                  <Link key={tool.id} className="tool-entry" to={tool.to}>
                    <span className="tool-entry__icon" aria-hidden>
                      <Icon size={20} weight="regular" />
                    </span>
                    <span className="tool-entry__body">
                      <h3>{tool.title}</h3>
                      <p>{tool.desc}</p>
                    </span>
                  </Link>
                );
              })}
            </div>
          </section>
        ))
      )}
    </div>
  );
}
