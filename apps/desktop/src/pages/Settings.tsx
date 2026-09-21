import { useEffect, useState } from "react";
import { api, friendlyError, isTauri, type DesktopSettings } from "../api/desktop";
import { Alert } from "../components/Alert";
import { FormSection } from "../components/FormSection";
import { PathField } from "../components/PathField";
import { Switch } from "../components/Switch";
import { selectOutputDirectory } from "../lib/tauri";

const defaults: DesktopSettings = {
  defaultOutputRoot: "",
  defaultRebuildTop: true,
  defaultRebuildLevels: 0,
  defaultTextureCompress: false,
  defaultConvertThreads: 1,
  pythonServerUrl: "http://127.0.0.1:8787",
  resourceServerPort: 0,
};

export function Settings() {
  const [form, setForm] = useState<DesktopSettings>({ ...defaults });
  const [saved, setSaved] = useState(false);
  const [health, setHealth] = useState<string | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [resourceInfo, setResourceInfo] = useState<string | null>(null);
  const [capsText, setCapsText] = useState<string | null>(null);
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
        setHealth(`状态：${h.status || (h.ok ? "ok" : "unknown")}${h.version ? ` · 版本 ${h.version}` : ""}`);
        setHealthError(null);
      } catch (e) {
        setHealth(null);
        setHealthError(friendlyError(e));
      }
      try {
        const caps = await api.capabilities();
        const parts: string[] = [];
        if (caps.convert?.exists) parts.push("本机转换器可用");
        else if (caps.convert?.docker) parts.push("Docker 转换可用");
        else parts.push("转换器不可用");
        if (caps.postprocessBasisu?.available) parts.push("basisu 可用");
        setCapsText(parts.join(" · "));
      } catch {
        setCapsText(null);
      }
      if (isTauri()) {
        try {
          const info = await api.getResourceServerInfo();
          if (info) {
            setResourceInfo(`${info.baseUrl} · ${info.dataDir}`);
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

      <div className="page-form">
        {loadError ? (
          <div style={{ marginBottom: 12 }}>
            <Alert kind="error">{loadError}</Alert>
          </div>
        ) : null}

        <FormSection title="默认参数">
          <PathField
            label="默认输出根目录"
            value={form.defaultOutputRoot}
            placeholder="例如 D:\\output"
            onChange={(v) => setForm({ ...form, defaultOutputRoot: v })}
            onPick={
              isTauri()
                ? () =>
                    void selectOutputDirectory()
                      .then((p) => {
                        if (p) setForm({ ...form, defaultOutputRoot: p });
                      })
                      .catch((e) => setLoadError(friendlyError(e)))
                : undefined
            }
            hint="新建任务时用于建议输出路径"
          />
          <div className="field">
            <Switch
              checked={form.defaultRebuildTop}
              onChange={(v) => setForm({ ...form, defaultRebuildTop: v })}
            >
              默认开启顶层重建
            </Switch>
          </div>
          <div className="field">
            <label>默认重建层数</label>
            <select
              className="select"
              value={form.defaultRebuildLevels}
              onChange={(e) =>
                setForm({
                  ...form,
                  defaultRebuildLevels: Number(e.target.value),
                })
              }
            >
              <option value={0}>自动到根</option>
              <option value={1}>1</option>
              <option value={2}>2</option>
            </select>
          </div>
          <div className="field">
            <Switch
              checked={form.defaultTextureCompress}
              onChange={(v) => setForm({ ...form, defaultTextureCompress: v })}
            >
              默认开启纹理压缩（KTX2）（实验）
            </Switch>
            <div className="field-hint">需要 basisu 运行时；如不可用请保持关闭（keep）</div>
          </div>
          <div className="field">
            <label>默认转换并发数</label>
            <select
              className="select"
              value={form.defaultConvertThreads ?? 1}
              onChange={(e) =>
                setForm({
                  ...form,
                  defaultConvertThreads: Number(e.target.value),
                })
              }
            >
              <option value={1}>1（推荐 M1 试用版）</option>
              <option value={2}>2</option>
              <option value={4}>4</option>
              <option value={0}>自动（CPU 核心数一半）</option>
            </select>
            <div className="field-hint">M1 试用版建议使用 1 worker 以确保稳定性</div>
          </div>
          <div className="actions">
            <button type="button" className="btn btn-primary" onClick={() => void save()}>
              保存
            </button>
            {saved ? <span className="muted">已保存</span> : null}
          </div>
        </FormSection>

        <details className="advanced-block" style={{ marginTop: 8 }}>
          <summary>运行环境 / 诊断</summary>
          <div className="advanced-block__body">
            <div className="field">
              <label>API</label>
              <input className="input" value={api.baseUrl} readOnly />
            </div>
            {health ? <Alert kind="success">{health}</Alert> : null}
            {healthError ? <Alert kind="warn">{healthError}</Alert> : null}
            {capsText ? (
              <div className="field-hint" style={{ marginBottom: 12 }}>
                {capsText}
              </div>
            ) : null}
            {resourceInfo ? (
              <div className="field">
                <label>资源服务</label>
                <input className="input" value={resourceInfo} readOnly />
              </div>
            ) : null}
            {isTauri() ? (
              <div className="field">
                <label>执行服务 URL</label>
                <input
                  className="input"
                  value={form.pythonServerUrl}
                  onChange={(e) => setForm({ ...form, pythonServerUrl: e.target.value })}
                  placeholder="http://127.0.0.1:8787"
                />
                <div className="field-hint">仅在本机转换器不可用时作为后备</div>
              </div>
            ) : null}
            <div className="actions">
              <button type="button" className="btn" onClick={() => void save()}>
                保存诊断项
              </button>
            </div>
          </div>
        </details>
      </div>
    </div>
  );
}
