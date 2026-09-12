import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { absolutizeLocalUrl, api, friendlyError } from '../api/desktop';
import type { Artifact } from '../api/types';
import { Alert } from '../components/Alert';

export function TilesPreview() {
  const [searchParams] = useSearchParams();
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [selectedId, setSelectedId] = useState(searchParams.get('artifact') || '');
  const initialTileset =
    searchParams.get('tileset') ||
    '/preview-site/tiles/tileset.json';
  const [tilesetUrl, setTilesetUrl] = useState(initialTileset);
  const [frameKey, setFrameKey] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [infoOpen, setInfoOpen] = useState(true);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  useEffect(() => {
    void (async () => {
      try {
        const list = await api.listArtifacts();
        setArtifacts(list);
        let id = selectedId;
        if (!id && list.length) {
          const withTiles = list.find((a) => a.has_tileset || a.hasTileset) || list[0];
          id = withTiles.id;
          setSelectedId(id);
        }
        // Prefer Rust get_preview_url when an artifact id is known (Tauri Phase 2).
        if (id) {
          try {
            const res = await api.previewUrl(id);
            const url = res.url || res.previewUrl || '';
            if (url) {
              setTilesetUrl(url);
              setFrameKey((k) => k + 1);
              setMessage(`已解析预览地址：${url}`);
            }
          } catch (e) {
            // Keep query tileset fallback (browser / Python static mount).
            setTilesetUrl((u) => absolutizeLocalUrl(u));
            setError(friendlyError(e));
          }
        } else {
          setTilesetUrl((u) => absolutizeLocalUrl(u));
        }
      } catch (e) {
        setError(friendlyError(e));
      }
    })();
  }, []);

  async function loadArtifact(id: string) {
    setSelectedId(id);
    setError(null);
    setMessage(null);
    if (!id) return;
    try {
      const res = await api.previewUrl(id);
      const url = res.url || res.previewUrl || '';
      setTilesetUrl(url);
      setFrameKey((k) => k + 1);
      setMessage(`已解析预览地址：${url}`);
    } catch (e) {
      setError(friendlyError(e));
    }
  }

  function reload() {
    setTilesetUrl((u) => absolutizeLocalUrl(u.trim()));
    setFrameKey((k) => k + 1);
    setMessage('已重新加载 tileset');
  }

  function fitView() {
    setMessage('已请求适应视野（Cesium home stub）');
    try {
      iframeRef.current?.contentWindow?.postMessage({ type: 'geoforge-fit' }, '*');
    } catch {
      /* ignore */
    }
  }

  const iframeSrc = `/cesium-preview.html?tileset=${encodeURIComponent(tilesetUrl)}`;
  const selected = useMemo(
    () => artifacts.find((a) => a.id === selectedId) || null,
    [artifacts, selectedId],
  );

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>预览</h1>
        </div>
        <div className="row" style={{ gap: 8 }}>
          <Link className="btn" to="/tiles/process">
            继续处理
          </Link>
          <a className="btn btn-primary" href={iframeSrc} target="_blank" rel="noreferrer">
            新窗口打开
          </a>
        </div>
      </div>

      {error ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="error">{error}</Alert>
        </div>
      ) : null}
      {message ? (
        <div style={{ marginBottom: 12 }}>
          <Alert kind="success">{message}</Alert>
        </div>
      ) : null}

      <div className="toolbar preview-toolbar">
        <div className="toolbar-group">
          <span className="toolbar-label">成果</span>
          <select
            className="select"
            style={{ maxWidth: 240 }}
            value={selectedId}
            onChange={(e) => void loadArtifact(e.target.value)}
          >
            <option value="">选择成果…</option>
            {artifacts.map((a) => (
              <option key={a.id} value={a.id}>
                {(a.label || a.id) + (a.has_tileset || a.hasTileset ? '' : '（无 tileset）')}
              </option>
            ))}
          </select>
        </div>
        <div className="toolbar-group grow">
          <span className="toolbar-label">tileset</span>
          <input
            className="input"
            style={{ flex: 1, minWidth: 180 }}
            value={tilesetUrl}
            onChange={(e) => setTilesetUrl(e.target.value)}
            placeholder="/artifacts/.../tileset.json 或 /preview-site/tiles/tileset.json"
          />
          <button className="btn btn-primary" type="button" onClick={reload}>
            加载
          </button>
        </div>
        <span className="toolbar-sep" />
        <div className="toolbar-group">
          <button className="btn" type="button" onClick={fitView} title="复位 / 适应视野">
            适应视野
          </button>
          <button className="btn" type="button" onClick={() => setInfoOpen((v) => !v)}>
            {infoOpen ? '隐藏信息' : '信息面板'}
          </button>
        </div>
      </div>

      <div className="preview-layout">
        <div className="preview-main">
          <iframe
            key={`${frameKey}:${tilesetUrl}`}
            ref={iframeRef}
            className="preview-frame fill"
            title="Cesium Tiles Preview"
            src={iframeSrc}
          />
        </div>
        {infoOpen ? (
          <aside className="info-panel">
            <div className="card card-pad">
              <h3>预览信息</h3>
              <div className="stat-grid">
                <div className="stat-chip">
                  <div className="k">成果</div>
                  <div className="v">{selected?.label || selectedId || '样例'}</div>
                </div>
                <div className="stat-chip">
                  <div className="k">tileset</div>
                  <div className="v">{tilesetUrl || '—'}</div>
                </div>
                <div className="stat-chip">
                  <div className="k">引擎</div>
                  <div className="v">CesiumJS</div>
                </div>
                <div className="stat-chip">
                  <div className="k">状态</div>
                  <div className="v">已加载 URL</div>
                </div>
              </div>
            </div>
            <div className="card card-pad">
              <h3>快捷操作</h3>
              <div className="row wrap" style={{ gap: 8 }}>
                <button className="btn btn-primary" type="button" onClick={reload}>
                  重新加载
                </button>
                <Link className="btn" to="/results">
                  处理成果
                </Link>
                <Link className="btn" to="/osgb/convert">
                  新建转换
                </Link>
              </div>
              <div className="muted" style={{ marginTop: 10, fontSize: 12 }}>
                支持相对路径与 /artifacts/{'{id}'}/tileset.json。适应视野通过 postMessage 触发 Cesium 重定位（含 ENU/局部原点）。
              </div>
            </div>
          </aside>
        ) : null}
      </div>
    </div>
  );
}
