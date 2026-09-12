import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api, friendlyError, isTauri } from '../api/desktop';
import type { Artifact } from '../api/types';
import { Alert } from '../components/Alert';
import { EmptyState } from '../components/EmptyState';

function fmtTime(v?: number) {
  if (v == null) return '—';
  const d = new Date(v * (v < 1e12 ? 1000 : 1));
  return d.toLocaleString('zh-CN', { timeZone: 'Asia/Shanghai' });
}

async function openDir(id: string) {
  try {
    await api.openArtifactDirectory(id);
  } catch (e) {
    // ignore if desktop cannot open
    console.warn(friendlyError(e));
  }
}

export function Results() {
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);

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

  async function copyPath(path: string) {
    try {
      await navigator.clipboard?.writeText(path);
      setCopied(path);
    } catch {
      setCopied(null);
    }
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>成果</h1>
        </div>
        <Link className="btn btn-primary" to="/tiles/process">
          处理已有 Tiles
        </Link>
      </div>

      {error ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="warn">{error}</Alert>
        </div>
      ) : null}
      {copied ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="success">已复制：{copied}</Alert>
        </div>
      ) : null}

      {loading && artifacts.length === 0 ? (
        <div className="muted">加载中…</div>
      ) : artifacts.length === 0 ? (
        <div className="card">
          <EmptyState title="暂无成果" description="转换成功后，输出会登记到成果列表。" />
        </div>
      ) : (
        <div className="result-grid">
          {artifacts.map((a) => {
            const hasTiles = a.has_tileset || a.hasTileset;
            return (
              <div className="card card-pad result-card" key={a.id}>
                <h3>{a.label || a.id}</h3>
                <p className="muted" style={{ fontSize: 12, marginBottom: 8 }}>
                  {a.path}
                </p>
                <p className="muted" style={{ fontSize: 12, marginBottom: 12 }}>
                  {a.id} · {hasTiles ? '有 tileset.json' : '无 tileset'} · {fmtTime(a.createdAt || a.created_at)}
                </p>
                <div className="row wrap">
                  <Link
                    className="btn btn-primary"
                    to={
                      hasTiles
                        ? `/preview/tiles?artifact=${encodeURIComponent(a.id)}&tileset=${encodeURIComponent(`/artifacts/${a.id}/tileset.json`)}`
                        : `/preview/tiles`
                    }
                  >
                    预览
                  </Link>
                  <Link
                    className="btn"
                    to={`/tiles/process?artifact=${encodeURIComponent(a.id)}&input=${encodeURIComponent(a.path)}&name=${encodeURIComponent((a.label || a.id) + '-process')}`}
                  >
                    继续处理
                  </Link>
                  <button className="btn" type="button" onClick={() => void copyPath(a.path)}>
                    打开路径（复制）
                  </button>
                  {isTauri() ? (
                    <button className="btn" type="button" onClick={() => void openDir(a.id)}>
                      打开目录
                    </button>
                  ) : null}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
