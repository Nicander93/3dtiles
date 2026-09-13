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
