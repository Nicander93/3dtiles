import { useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { ArrowRight, MagnifyingGlass } from '@phosphor-icons/react';
import { filterToolGroups } from '../lib/tools';

export function Workspace() {
  const [query, setQuery] = useState('');
  const groups = useMemo(() => filterToolGroups(query), [query]);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>3D 地理数据工具</h1>
          <p>选择工具，开始转换、预览或处理。</p>
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

      <details className="release-note">
        <summary>规则格网试用版 · 使用范围</summary>
        <p>
          Windows · 连续规则网格 ≤16×16 · 保留原纹理 · 建议并发
          1。暂不支持稀疏数据、任意城区和生产规模部署。
        </p>
      </details>

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
                    <ArrowRight className="tool-entry__arrow" size={18} aria-hidden />
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
