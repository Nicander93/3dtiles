import { useEffect, useState } from "react";
import { api, friendlyError, isTauri, type DesktopSettings } from "../api/desktop";
import { Alert } from "../components/Alert";

const defaults: DesktopSettings = {
  defaultOutputRoot: "",
  defaultRebuildTop: true,
  defaultRebuildLevels: 1,
  defaultTextureCompress: true,
  pythonServerUrl: "http://127.0.0.1:8787",
  resourceServerPort: 0,
};

export function Settings() {
  const [form, setForm] = useState<DesktopSettings>({ ...defaults });
  const [saved, setSaved] = useState(false);
  const [health, setHealth] = useState<string | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [resourceInfo, setResourceInfo] = useState<string | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const s = await api.getSettings();
        setForm({ ...defaults, ...s });
      } catch (e) {
        setLoadError(friendlyError(e));
      }
      try {
        const h = await api.health();
        setHealth(`状态：${h.status}${h.version ? ` · 版本 ${h.version}` : ""}`);
        setHealthError(null);
      } catch (e) {
        setHealth(null);
        setHealthError(friendlyError(e));
      }
      if (isTauri()) {
        try {
          const info = await api.getResourceServerInfo();
          if (info) {
            setResourceInfo(`Rust artifact server ${info.baseUrl} · data ${info.dataDir}`);
          }
        } catch {
          setResourceInfo(null);
        }
      }
    })();
  }, []);

  async function save() {
    try {
      const next = await api.updateSettings(form);
      setForm({ ...defaults, ...next });
      setSaved(true);
      window.setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setLoadError(friendlyError(e));
    }
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>设置</h1>
        </div>
      </div>

      <div className="page-with-aside">
        <div className="card card-pad">
          <div className="section-title">默认参数</div>
          {loadError && <Alert kind="error">{loadError}</Alert>}
          <div className="field">
            <label>默认输出根目录</label>
            <input
              className="input"
              value={form.defaultOutputRoot}
              onChange={(e) => setForm({ ...form, defaultOutputRoot: e.target.value })}
              placeholder="例如 /data/outputs"
            />
          </div>
          <div className="field">
            <label className="switch">
              <input
                type="checkbox"
                checked={form.defaultRebuildTop}
                onChange={(e) => setForm({ ...form, defaultRebuildTop: e.target.checked })}
              />
              默认开启顶层重建
            </label>
          </div>
          <div className="field">
            <label>默认重建层数（rebuildTop.levels）</label>
            <select
              className="select"
              value={form.defaultRebuildLevels === 2 ? 2 : 1}
              onChange={(e) =>
                setForm({
                  ...form,
                  defaultRebuildLevels: Number(e.target.value) === 2 ? 2 : 1,
                })
              }
            >
              <option value={1}>1 — 默认</option>
              <option value={2}>2 — 两层金字塔</option>
            </select>
          </div>
          <div className="field">
            <label className="switch">
              <input
                type="checkbox"
                checked={form.defaultTextureCompress}
                onChange={(e) => setForm({ ...form, defaultTextureCompress: e.target.checked })}
              />
              默认开启纹理压缩（KTX2）
            </label>
          </div>
          {isTauri() && (
            <div className="field">
              <label>Python desktop_server URL（Phase 2 执行桥）</label>
              <input
                className="input"
                value={form.pythonServerUrl}
                onChange={(e) => setForm({ ...form, pythonServerUrl: e.target.value })}
                placeholder="http://127.0.0.1:8787"
              />
            </div>
          )}
          <div className="actions">
            <button type="button" className="btn btn-primary" onClick={() => void save()}>
              保存
            </button>
            {saved && <span className="muted">已保存</span>}
          </div>
        </div>

        <aside className="card card-pad">
          <div className="section-title">运行时</div>
          <div className="field">
            <label>API / Desktop</label>
            <input className="input" value={api.baseUrl} readOnly />
          </div>
          {health && <Alert kind="success">{health}</Alert>}
          {healthError && <Alert kind="warn">{healthError}</Alert>}
          {resourceInfo && <Alert kind="success">{resourceInfo}</Alert>}
          <p className="muted" style={{ marginTop: 12 }}>
            任务和成果保存在本地。转换走 Processor；找不到 Processor 时才会用 Python 服务。
          </p>
        </aside>
      </div>
    </div>
  );
}
