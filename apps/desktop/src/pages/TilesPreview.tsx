import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { CaretDown } from '@phosphor-icons/react';
import { absolutizeLocalUrl, api, friendlyError, isTauri } from '../api/desktop';
import type { Artifact } from '../api/types';
import { Alert } from '../components/Alert';
import { selectTilesetFile } from '../lib/tauri';

type LoadState = 'idle' | 'loading' | 'ready' | 'error';

export function TilesPreview() {
  const [searchParams, setSearchParams] = useSearchParams();
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [selectedId, setSelectedId] = useState(searchParams.get('artifact') || '');
  const [tilesetUrl, setTilesetUrl] = useState('');
  const [dataName, setDataName] = useState('');
  const [inputPath, setInputPath] = useState('');
  const [frameKey, setFrameKey] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('idle');
  const [infoOpen, setInfoOpen] = useState(false);
  const [diagOpen, setDiagOpen] = useState(false);
  const [processOpen, setProcessOpen] = useState(false);
  const [diagNote, setDiagNote] = useState('');
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const hasData = Boolean(tilesetUrl);

  useEffect(() => {
    void (async () => {
      try {
        const list = await api.listArtifacts();
        setArtifacts(list);
      } catch (e) {
        setError(friendlyError(e));
      }
    })();
  }, []);

  // Load from query artifact on mount / change
  useEffect(() => {
    const artifact = searchParams.get('artifact');
    const tileset = searchParams.get('tileset');
    if (artifact) {
      void loadArtifact(artifact);
      return;
    }
    if (tileset) {
      const url = absolutizeLocalUrl(tileset);
      setTilesetUrl(url);
      setDataName('外部 tileset');
      setSelectedId('');
      setFrameKey((k) => k + 1);
      setLoadState('loading');
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [searchParams.get('artifact'), searchParams.get('tileset')]);

  useEffect(() => {
    function onMessage(ev: MessageEvent) {
      const data = ev.data;
      if (!data || typeof data !== 'object') return;
      if (data.type === 'geoforge-preview-ready') {
        setLoadState('ready');
        setError(null);
        if (data.diag) setDiagNote(String(data.diag));
      } else if (data.type === 'geoforge-preview-error') {
        setLoadState('error');
        setError(data.message || '加载失败');
      } else if (data.type === 'geoforge-preview-loading') {
        setLoadState('loading');
      }
    }
    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, []);

  async function loadArtifact(id: string) {
    setSelectedId(id);
    setError(null);
    setDiagNote('');
    if (!id) {
      setTilesetUrl('');
      setDataName('');
      setInputPath('');
      setLoadState('idle');
      return;
    }
    setLoadState('loading');
    try {
      const res = await api.previewUrl(id);
      const url = res.url || res.previewUrl || '';
      if (!url) {
        setLoadState('error');
        setError('无法解析预览地址');
        return;
      }
      setTilesetUrl(url);
      const art = artifacts.find((a) => a.id === id);
      setDataName(art?.label || id);
      setInputPath(art?.path || '');
      setFrameKey((k) => k + 1);
    } catch (e) {
      setLoadState('error');
      setError(friendlyError(e));
    }
  }

  async function openData() {
    setProcessOpen(false);
    if (isTauri()) {
      const p = await selectTilesetFile();
      if (!p) return;
      // Prefer artifact match by path; else clear selection and use path via prepare if available
      const match = artifacts.find((a) => a.path && (p === a.path || p.startsWith(a.path)));
      if (match) {
        setSearchParams({ artifact: match.id });
        void loadArtifact(match.id);
        return;
      }
      setSelectedId('');
      setDataName(p.split(/[/\\]/).slice(-2, -1)[0] || '本地数据');
      setInputPath(p.replace(/[/\\]tileset\.json$/i, ''));
      setError('请从成果列表打开已登记的数据，或先完成转换。');
      setLoadState('idle');
      setTilesetUrl('');
      return;
    }
    // Browser: pick from artifacts dropdown via prompt-like select
    const first = artifacts.find((a) => a.has_tileset || a.hasTileset);
    if (first) {
      setSearchParams({ artifact: first.id });
      void loadArtifact(first.id);
    } else {
      setError('暂无可用成果，请先完成转换。');
    }
  }

  function fitView() {
    if (!hasData || loadState !== 'ready') return;
    try {
      iframeRef.current?.contentWindow?.postMessage({ type: 'geoforge-fit' }, '*');
    } catch {
      /* ignore */
    }
  }

  const iframeSrc = hasData
    ? `/cesium-preview.html?tileset=${encodeURIComponent(tilesetUrl)}`
    : '';

  const processLinks = useMemo(() => {
    const params = new URLSearchParams();
    if (inputPath) params.set('input', inputPath);
    if (selectedId) params.set('artifact', selectedId);
    if (dataName) params.set('name', dataName);
    const q = params.toString();
    return {
      rebuild: `/tiles/process?op=rebuild&${q}`,
      texture: `/tiles/process?op=texture&${q}`,
    };
  }, [inputPath, selectedId, dataName]);

  const selected = artifacts.find((a) => a.id === selectedId) || null;

  return (
    <div className="preview-page">
      <div className="preview-toolbar">
        <span className="preview-toolbar__name" title={dataName || undefined}>
          {dataName || '未选择数据'}
        </span>
        <button className="btn btn-sm" type="button" onClick={() => void openData()}>
          打开数据
        </button>
        <button
          className="btn btn-sm"
          type="button"
          disabled={!hasData || loadState !== 'ready'}
          onClick={fitView}
        >
          适应视野
        </button>
        <div className="menu">
          <button
            className="btn btn-sm"
            type="button"
            disabled={!hasData || !inputPath}
            onClick={() => setProcessOpen((v) => !v)}
            aria-haspopup="menu"
            aria-expanded={processOpen}
          >
            处理 <CaretDown size={12} />
          </button>
          {processOpen ? (
            <div className="menu__panel" role="menu">
              <Link className="menu__item" to={processLinks.rebuild} onClick={() => setProcessOpen(false)}>
                顶层重建
              </Link>
              <Link className="menu__item" to={processLinks.texture} onClick={() => setProcessOpen(false)}>
                纹理压缩
              </Link>
            </div>
          ) : null}
        </div>
        <div className="preview-toolbar__spacer" />
        <button
          className="btn btn-sm"
          type="button"
          disabled={!hasData}
          onClick={() => setInfoOpen((v) => !v)}
        >
          {infoOpen ? '关闭信息' : '信息'}
        </button>
      </div>

      {error && loadState === 'error' ? (
        <div style={{ marginBottom: 8 }}>
          <Alert kind="error">{error}</Alert>
        </div>
      ) : null}
      {error && loadState === 'idle' ? (
        <div style={{ marginBottom: 8 }}>
          <Alert kind="warn">{error}</Alert>
        </div>
      ) : null}

      <div className={`preview-layout${infoOpen ? ' with-panel' : ''}`}>
        <div className="preview-main">
          {!hasData ? (
            <div className="preview-empty">
              <p>打开 3D Tiles 数据以开始预览</p>
              <button className="btn btn-primary" type="button" onClick={() => void openData()}>
                打开数据
              </button>
              {artifacts.some((a) => a.has_tileset || a.hasTileset) ? (
                <select
                  className="select"
                  style={{ maxWidth: 320 }}
                  value=""
                  onChange={(e) => {
                    if (!e.target.value) return;
                    setSearchParams({ artifact: e.target.value });
                    void loadArtifact(e.target.value);
                  }}
                  aria-label="从成果选择"
                >
                  <option value="">从成果选择…</option>
                  {artifacts
                    .filter((a) => a.has_tileset || a.hasTileset)
                    .map((a) => (
                      <option key={a.id} value={a.id}>
                        {a.label || a.id}
                      </option>
                    ))}
                </select>
              ) : null}
            </div>
          ) : (
            <>
              {loadState === 'loading' ? (
                <div className="preview-status">加载中…</div>
              ) : null}
              {loadState === 'ready' ? (
                <div className="preview-status">加载完成</div>
              ) : null}
              <iframe
                key={`${frameKey}:${tilesetUrl}`}
                ref={iframeRef}
                className="preview-frame"
                title="Cesium Tiles Preview"
                src={iframeSrc}
              />
            </>
          )}
        </div>

        {infoOpen ? (
          <aside className="info-panel">
            <div>
              <h3>数据信息</h3>
              <div className="summary-box">
                <dl>
                  <dt>名称</dt>
                  <dd>{dataName || '—'}</dd>
                  <dt>路径</dt>
                  <dd>{selected?.path || inputPath || '—'}</dd>
                  <dt>状态</dt>
                  <dd>
                    {loadState === 'loading'
                      ? '加载中'
                      : loadState === 'ready'
                        ? '加载完成'
                        : loadState === 'error'
                          ? '失败'
                          : '未加载'}
                  </dd>
                </dl>
              </div>
            </div>
            <details open={diagOpen} onToggle={(e) => setDiagOpen((e.target as HTMLDetailsElement).open)}>
              <summary className="muted" style={{ cursor: 'pointer', fontSize: 13 }}>
                诊断信息
              </summary>
              <div className="summary-box" style={{ marginTop: 8 }}>
                <dl>
                  <dt>预览 URL</dt>
                  <dd>{tilesetUrl || '—'}</dd>
                  <dt>说明</dt>
                  <dd>{diagNote || '—'}</dd>
                </dl>
              </div>
            </details>
          </aside>
        ) : null}
      </div>
    </div>
  );
}
