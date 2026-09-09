import { useEffect, useState } from "react";
import { api, friendlyError } from "../api/client";
import { Alert } from "../components/Alert";

const SETTINGS_KEY = "geoforge.settings";

type SettingsState = {
  defaultOutputRoot: string;
  defaultRebuildTop: boolean;
  defaultRebuildLevels: number;
  defaultTextureCompress: boolean;
};

const defaults: SettingsState = {
  defaultOutputRoot: "",
  defaultRebuildTop: true,
  defaultRebuildLevels: 1,
  defaultTextureCompress: true,
};

function load(): SettingsState {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (!raw) return defaults;
    return { ...defaults, ...JSON.parse(raw) };
  } catch {
    return defaults;
  }
}

export function Settings() {
  const [form, setForm] = useState<SettingsState>(() => load());
  const [saved, setSaved] = useState(false);
  const [health, setHealth] = useState<string | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const h = await api.health();
        setHealth(`状态：${h.status}${h.version ? ` · 版本 ${h.version}` : ""}`);
        setHealthError(null);
      } catch (e) {
        setHealth(null);
        setHealthError(friendlyError(e));
      }
    })();
  }, []);

  function save() {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify(form));
    setSaved(true);
    window.setTimeout(() => setSaved(false), 2000);
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>设置与帮助</h1>
          <p>默认转换参数与本地运行说明</p>
        </div>
      </div>

      <div className="page-with-aside">
        <div className="card card-pad">
          <div className="section-title">默认参数</div>
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
              默认纹理压缩（KTX2）
            </label>
            <div className="field-hint">KTX2 经 convert/process 后的 basisu 后处理生效（非必须新 _3dtile）。</div>
          </div>
          <div className="field">
            <label>API 基址（只读展示，构建时由 VITE_API_BASE 决定）</label>
            <input className="input" value={api.baseUrl} readOnly />
            <div className="field-hint">修改请在 apps/web/.env 中设置 VITE_API_BASE 后重启 dev。</div>
          </div>
          <div className="row">
            <button className="btn btn-primary" type="button" onClick={save}>
              保存设置
            </button>
            {saved ? <span className="muted">已保存</span> : null}
          </div>
        </div>

        <aside className="aside-stack">
          <div className="card card-pad">
            <div className="section-title">后端健康检查</div>
            {health ? <Alert kind="success">{health}</Alert> : null}
            {healthError ? <Alert kind="warn">{healthError}</Alert> : null}
          </div>
          <div className="card card-pad">
            <div className="section-title">坐标与高程</div>
            <p className="muted" style={{ fontSize: 13, marginBottom: 8 }}>
              高程基准独立处理：椭球高、正常高及偏移量不可混为一谈。首版不自动猜测高程基准；
              缺格网或缺参数时请手动补齐 CRS/原点，地理导出才会放行。
            </p>
            <p className="muted" style={{ fontSize: 13, margin: 0 }}>
              详见转换页「实际采用的 CRS / 原点」与 docs/product/USER_GUIDE.md。
            </p>
          </div>
          <div className="card card-pad">
            <div className="section-title">帮助</div>
            <p className="muted" style={{ fontSize: 13, marginBottom: 8 }}>
              GeoForge 3D v0.1.0 · 本地模式。UI 只编排本地 CLI（_3dtile / rebuild-top），不上传数据。
            </p>
            <ul className="muted" style={{ margin: 0, paddingLeft: 18, fontSize: 13 }}>
              <li>一键启动：bash scripts/run_geoforge.sh → http://127.0.0.1:8787/</li>
              <li>样例 OSGB：/workspace/data/OSGBny/OSGBny</li>
              <li>原生预览需 DISPLAY（本环境常用 :2）；Qt osgb_viewer 为可选本地预览</li>
              <li>KTX2 经 basisu 后处理可用（KHR_texture_basisu）；原生 --enable-texture-compress 仍可选</li>
            </ul>
          </div>
        </aside>
      </div>
    </div>
  );
}
