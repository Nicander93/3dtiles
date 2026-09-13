import { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { DotsThree, MagnifyingGlass } from '@phosphor-icons/react';
import { api, friendlyError, isTauri } from '../api/desktop';
import type { Artifact } from '../api/types';
import { Alert } from '../components/Alert';
import { EmptyState } from '../components/EmptyState';
import { suggestOutputPath } from '../lib/formUtils';

function fmtTime(v?: number) {
  if (v == null) return '—';
  const d = new Date(v * (v < 1e12 ? 1000 : 1));
  return d.toLocaleString('zh-CN', { timeZone: 'Asia/Shanghai' });
}

async function openDir(id: string) {
  try {
    await api.openArtifactDirectory(id);
  } catch (e) {
    console.warn(friendlyError(e));
  }
}

export function Results() {
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [menuId, setMenuId] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        setArtifacts(await api.listArtifacts());
      } catch (e) {
        setError(friendlyError(e));
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return artifacts;
    return artifacts.filter(
      (a) =>
        (a.label || '').toLowerCase().includes(q) ||
        (a.path || '').toLowerCase().includes(q) ||
        (a.id || '').toLowerCase().includes(q),
    );
  }, [artifacts, query]);

  async function copyPath(path: string) {
    try {
      await navigator.clipboard?.writeText(path);
      setCopied(path);
      window.setTimeout(() => setCopied(null), 2000);
    } catch {
      setCopied(null);
    }
  }

  function processHref(a: Artifact): string {
    const params = new URLSearchParams();
    params.set('artifact', a.id);
    params.set('input', a.path);
    params.set('output', suggestOutputPath(a.path, '', '_process'));
    params.set('name', `${a.label || a.id}-process`);
    return `/tiles/process?${params.toString()}`;
  }

  function previewHref(a: Artifact): string {
    const hasTiles = a.has_tileset || a.hasTileset;
    if (!hasTiles) return '/preview/tiles';
    return `/preview/tiles?artifact=${encodeURIComponent(a.id)}`;
  }

  return (
    <div className="page">
      <div className="page-header full-width">
        <div>
          <h1>成果</h1>
        </div>
        <div className="search-wrap">
          <span className="search-wrap__icon" aria-hidden>
            <MagnifyingGlass size={16} />
          </span>
          <input
            className="input"
            placeholder="搜索成果…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            aria-label="搜索成果"
          />
        </div>
      </div>

      {error ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="warn">{error}</Alert>
        </div>
      ) : null}
      {copied ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="success">已复制路径</Alert>
        </div>
      ) : null}

      {loading && artifacts.length === 0 ? (
        <div className="muted">加载中…</div>
      ) : artifacts.length === 0 ? (
        <EmptyState title="暂无成果" description="转换成功后，输出会登记到成果列表。" />
      ) : filtered.length === 0 ? (
        <p className="muted">未找到成果</p>
      ) : (
        <div>
          {filtered.map((a) => {
            const hasTiles = a.has_tileset || a.hasTileset;
            const missing = a.available === false;
            return (
              <div className="result-row" key={a.id}>
                <div className="result-row__body">
                  <h3 title={a.path}>{a.label || a.id}</h3>
                  <div className="result-row__meta">
                    {missing ? (
                      <span style={{ color: 'var(--danger)' }}>文件不存在</span>
                    ) : (
                      <span>3D Tiles</span>
                    )}
                    <span> · </span>
                    <span>{fmtTime(a.createdAt || a.created_at)}</span>
                  </div>
                </div>
                <div className="result-row__actions">
                  <Link
                    className="btn btn-sm"
                    to={previewHref(a)}
                    aria-disabled={missing || !hasTiles}
                    style={missing || !hasTiles ? { pointerEvents: 'none', opacity: 0.45 } : undefined}
                  >
                    预览
                  </Link>
                  <Link
                    className="btn btn-sm"
                    to={processHref(a)}
                    style={missing ? { pointerEvents: 'none', opacity: 0.45 } : undefined}
                  >
                    继续处理
                  </Link>
                  {isTauri() ? (
                    <button
                      className="btn btn-sm"
                      type="button"
                      disabled={missing}
                      onClick={() => void openDir(a.id)}
                    >
                      打开目录
                    </button>
                  ) : null}
                  <div className="menu">
                    <button
                      className="btn btn-sm btn-ghost"
                      type="button"
                      aria-label="更多"
                      onClick={() => setMenuId((id) => (id === a.id ? null : a.id))}
                    >
                      <DotsThree size={18} />
                    </button>
                    {menuId === a.id ? (
                      <div className="menu__panel">
                        <button
                          className="menu__item"
                          type="button"
                          onClick={() => {
                            void copyPath(a.path);
                            setMenuId(null);
                          }}
                        >
                          复制路径
                        </button>
                      </div>
                    ) : null}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
