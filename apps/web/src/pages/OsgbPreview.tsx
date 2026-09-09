import { useMemo, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api, friendlyError } from '../api/client';
import { Alert } from '../components/Alert';

const SAMPLE = '/workspace/data/OSGBny/OSGBny';

export function OsgbPreview() {
  const [searchParams] = useSearchParams();
  const [path, setPath] = useState(searchParams.get('path') || SAMPLE);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [models, setModels] = useState<Array<{ name: string; url: string }>>([]);
  const [webSrc, setWebSrc] = useState('/preview-site/osgb.html');
  const [infoOpen, setInfoOpen] = useState(true);
  const iframeRef = useRef<HTMLIFrameElement>(null);

  const exportLink = useMemo(
    () => `/osgb/convert?input=${encodeURIComponent(path.trim())}`,
    [path],
  );

  function fitView() {
    setMessage('已请求复位视角（Web 预览 stub；原生请用 Reset View）');
    try {
      iframeRef.current?.contentWindow?.postMessage({ type: 'geoforge-fit' }, '*');
    } catch {
      /* ignore */
    }
  }

  async function openNative() {
    setError(null);
    setMessage(null);
    if (!path.trim()) {
      setError('请填写 OSGB 目录路径。');
      return;
    }
    setBusy(true);
    try {
      const res = await api.nativeOsgbPreview(path.trim());
      if (!res.ok) {
        setError(res.error || '原生预览启动失败（可能缺少 DISPLAY 或 conda 环境）。');
      } else {
        setMessage(`已启动原生预览 pid=${res.pid} DISPLAY=${res.display || '?'}`);
      }
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  async function prepareWeb() {
    setError(null);
    setMessage(null);
    setBusy(true);
    try {
      const res = await api.prepareOsgb(path.trim());
      setModels(res.models || []);
      if (res.models?.length) {
        setWebSrc('/preview-site/osgb.html');
        setMessage(`已准备 ${res.models.length} 个 GLB 模型（Web 预览）。错误：${(res.errors || []).length}`);
      } else {
        setError('未生成 GLB；可改用原生预览。' + ((res.errors || []).join('; ') || ''));
      }
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>OSGB预览</h1>
          <p>浏览本地 OSGB 三维模型，支持原生预览与 Web GLB 回退</p>
        </div>
        <Link className="btn btn-primary" to={exportLink}>
          导出 3D Tiles
        </Link>
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
        <div className="toolbar-group grow">
          <span className="toolbar-label">路径</span>
          <input
            className="input"
            style={{ flex: 1, minWidth: 180 }}
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="OSGB 根目录"
          />
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
        <span className="toolbar-sep" />
        <div className="toolbar-group">
          <button className="btn btn-primary" type="button" disabled={busy} onClick={() => void openNative()}>
            打开原生预览
          </button>
          <button className="btn" type="button" disabled={busy} onClick={() => void prepareWeb()}>
            准备 Web GLB
          </button>
          <a className="btn" href={webSrc} target="_blank" rel="noreferrer">
            新窗口
          </a>
        </div>
      </div>

      <div className="preview-layout">
        <div className="preview-main">
          <iframe
            ref={iframeRef}
            className="preview-frame fill"
            title="OSGB Web Preview"
            src={webSrc}
          />
        </div>
        {infoOpen ? (
          <aside className="info-panel">
            <div className="card card-pad">
              <h3>数据详情</h3>
              <div className="stat-grid">
                <div className="stat-chip">
                  <div className="k">路径</div>
                  <div className="v">{path || '—'}</div>
                </div>
                <div className="stat-chip">
                  <div className="k">样例</div>
                  <div className="v">OSGBny</div>
                </div>
                <div className="stat-chip">
                  <div className="k">Web 模型</div>
                  <div className="v">{models.length || '未准备'}</div>
                </div>
                <div className="stat-chip">
                  <div className="k">预览路径</div>
                  <div className="v">OSGB → GLB / 原生</div>
                </div>
              </div>
            </div>
            <div className="card card-pad">
              <h3>快捷操作</h3>
              <div className="row wrap" style={{ gap: 8 }}>
                <Link className="btn btn-primary" to={exportLink}>
                  导出 3D Tiles
                </Link>
                <Link className="btn" to="/preview/tiles">
                  3D Tiles 预览
                </Link>
              </div>
              {models.length > 0 ? (
                <div className="muted" style={{ marginTop: 10, fontSize: 12 }}>
                  已加载：{models.map((m) => m.name).join(', ')}
                </div>
              ) : (
                <div className="muted" style={{ marginTop: 10, fontSize: 12 }}>
                  提示：原生预览需 DISPLAY；无显示时可「准备 Web GLB」。
                </div>
              )}
            </div>
          </aside>
        ) : null}
      </div>
    </div>
  );
}
