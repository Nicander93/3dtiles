import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api, friendlyError } from '../api/desktop';
import type { CapabilitiesResponse, TextureMode } from '../api/types';
import { AdvancedBlock } from '../components/AdvancedBlock';
import { Alert } from '../components/Alert';
import { FormSection } from '../components/FormSection';
import { PathField } from '../components/PathField';
import { SubmitBar } from '../components/SubmitBar';
import { Switch } from '../components/Switch';
import { pathsEqual, suggestOutputPath } from '../lib/formUtils';
import {
  rebuildLevelsLabel,
  rebuildQualityOptions,
  type RebuildQuality,
} from '../lib/rebuildQuality';
import { ktx2Etc1sEnabled, ktx2UastcEnabled } from '../lib/textureCaps';
import { isTauri, selectInputDirectory, selectOutputDirectory, selectTilesetFile } from '../lib/tauri';

const CONFIG_KEY = 'geoforge.tiles.process.config';
const OUTPUT_TOUCHED_KEY = 'geoforge.tiles.process.outputTouched';

type FormState = {
  input: string;
  output: string;
  name: string;
  rebuildTop: boolean;
  quality: RebuildQuality;
  rebuildLevels: number;
  textureMode: TextureMode;
};

const defaults: FormState = {
  input: '',
  output: '',
  name: '',
  rebuildTop: true,
  quality: 'balanced',
  rebuildLevels: 0,
  textureMode: 'keep',
};

function loadConfig(): FormState {
  try {
    const raw = localStorage.getItem(CONFIG_KEY);
    if (!raw) return defaults;
    return { ...defaults, ...(JSON.parse(raw) as Partial<FormState>) };
  } catch {
    return defaults;
  }
}

function pageTitle(op: string | null): string {
  if (op === 'rebuild') return '顶层重建';
  if (op === 'texture') return '纹理压缩';
  return 'Tiles 处理';
}

export function ProcessTiles() {
  const [searchParams] = useSearchParams();
  const op = searchParams.get('op');
  const title = pageTitle(op);
  const opApplied = useRef(false);

  const [form, setForm] = useState<FormState>(() => loadConfig());
  const [outputTouched, setOutputTouched] = useState(() => {
    try {
      return localStorage.getItem(OUTPUT_TOUCHED_KEY) === '1';
    } catch {
      return false;
    }
  });
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState<ReactNode>(null);
  const [error, setError] = useState<string | null>(null);
  const [caps, setCaps] = useState<CapabilitiesResponse | null>(null);
  const [defaultOutputRoot, setDefaultOutputRoot] = useState('');
  const [advancedOpen, setAdvancedOpen] = useState(false);

  useEffect(() => {
    void api.capabilities().then(setCaps).catch(() => setCaps(null));
    void api.getSettings().then((s) => setDefaultOutputRoot(s.defaultOutputRoot || '')).catch(() => {});
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem(CONFIG_KEY, JSON.stringify(form));
      localStorage.setItem(OUTPUT_TOUCHED_KEY, outputTouched ? '1' : '0');
    } catch {
      /* ignore */
    }
  }, [form, outputTouched]);

  // Entry presets from tool home
  useEffect(() => {
    if (opApplied.current) return;
    if (op === 'rebuild') {
      setForm((f) => ({ ...f, rebuildTop: true }));
      opApplied.current = true;
    } else if (op === 'texture') {
      setForm((f) => ({
        ...f,
        textureMode: ktx2Etc1sEnabled(caps, true) ? 'ktx2-etc1s' : f.textureMode,
        rebuildTop: f.rebuildTop,
      }));
      opApplied.current = true;
    }
  }, [op, caps]);

  useEffect(() => {
    const input = searchParams.get('input') || searchParams.get('path');
    const output = searchParams.get('output');
    const name = searchParams.get('name');
    const artifact = searchParams.get('artifact');
    if (!input && !output && !name && !artifact) return;
    setForm((f) => ({
      ...f,
      ...(input ? { input } : {}),
      ...(output ? { output } : {}),
      ...(name ? { name } : {}),
    }));
    if (output) setOutputTouched(true);
    if (artifact && !input) {
      void (async () => {
        try {
          const arts = await api.listArtifacts();
          const a = arts.find((x) => x.id === artifact);
          if (a?.path) {
            setForm((f) => ({
              ...f,
              input: a.path,
              name: f.name || a.label || '',
            }));
          }
        } catch {
          /* ignore */
        }
      })();
    }
  }, [searchParams]);

  useEffect(() => {
    if (outputTouched) return;
    const suggested = suggestOutputPath(form.input, defaultOutputRoot, '_process');
    if (suggested && suggested !== form.output) {
      setForm((f) => ({ ...f, output: suggested }));
    }
  }, [form.input, defaultOutputRoot, outputTouched]);

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  function handleReset() {
    const next = { ...defaults };
    if (op === 'rebuild') next.rebuildTop = true;
    if (op === 'texture' && ktx2Etc1sEnabled(caps, true)) {
      next.textureMode = 'ktx2-etc1s';
    }
    setForm(next);
    setOutputTouched(false);
    setMessage(null);
    setError(null);
    try {
      localStorage.removeItem(CONFIG_KEY);
      localStorage.removeItem(OUTPUT_TOUCHED_KEY);
    } catch {
      /* ignore */
    }
  }

  async function handleSubmit() {
    setError(null);
    setMessage(null);
    if (!form.input.trim() || !form.output.trim()) {
      setError('请填写输入 tileset 目录与输出路径。');
      return;
    }
    if (pathsEqual(form.input, form.output)) {
      setError('输出目录不能与输入目录相同，请更换输出位置。');
      return;
    }
    if (!form.rebuildTop && form.textureMode === 'keep') {
      setError('请启用顶层重建，或选择非保留的纹理模式。');
      return;
    }
    const pt = caps?.textureModes?.find(
      (m) => m.mode === (form.textureMode === 'ktx2' ? 'ktx2-etc1s' : form.textureMode),
    );
    if (form.textureMode !== 'keep' && pt?.processTileset?.supported === false) {
      setError(pt.processTileset.reason || '当前无法做纹理压缩，请检查运行环境。');
      return;
    }
    setSubmitting(true);
    try {
      const preset = rebuildQualityOptions(form.quality, ktx2Etc1sEnabled(caps, true));
      const res = await api.createTask({
        operation: 'process-tileset',
        input: { path: form.input.trim() },
        output: { path: form.output.trim() },
        taskName: form.name.trim() || undefined,
        options: {
          rebuildTop: {
            enabled: form.rebuildTop,
            levels: form.rebuildLevels,
            simplify: 0.5,
            textureScale: 0.5,
            l1MaxTriangles: preset.l1MaxTriangles,
            l2MaxTriangles: preset.l2MaxTriangles,
          },
          texture: { mode: form.textureMode },
        },
      });
      const id = res.task?.id || res.id;
      setMessage(
        id ? (
          <>
            任务已创建{' '}
            <Link to={`/processing?task=${encodeURIComponent(id)}`}>查看任务</Link>
          </>
        ) : (
          '任务已创建'
        ),
      );
      setOutputTouched(false);
      const nextOut = suggestOutputPath(form.input, defaultOutputRoot, `_process_${Date.now().toString(36)}`);
      if (nextOut) setForm((f) => ({ ...f, output: nextOut }));
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setSubmitting(false);
    }
  }

  const ktxOk = ktx2Etc1sEnabled(caps, true);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <div className="page-crumb">
            <Link to="/">← 返回工具</Link>
            <span>/</span>
            <span>{title}</span>
          </div>
          <h1>{title}</h1>
        </div>
      </div>

      <div className="page-form">
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
        {!ktxOk && (op === 'texture' || form.textureMode !== 'keep') ? (
          <div style={{ marginBottom: 12 }}>
            <Alert kind="warn">
              当前环境无法做 KTX2 纹理压缩。请检查 basisu 或转换能力。{' '}
              <Link to="/settings">打开设置</Link>
            </Alert>
          </div>
        ) : null}

        <FormSection title="输入">
          <PathField
            label="Tileset 目录"
            value={form.input}
            placeholder="含 tileset.json"
            onChange={(v) => update('input', v)}
            onPick={
              isTauri()
                ? () =>
                    void selectInputDirectory().then((p) => {
                      if (p) update('input', p);
                    })
                : undefined
            }
          />
          {isTauri() ? (
            <div className="field">
              <button
                className="btn"
                type="button"
                onClick={() =>
                  void selectTilesetFile().then((p) => {
                    if (!p) return;
                    const norm = p.replace(/\\/g, '/');
                    const dir = norm.endsWith('/tileset.json')
                      ? norm.slice(0, -'/tileset.json'.length)
                      : norm.includes('/')
                        ? norm.slice(0, norm.lastIndexOf('/'))
                        : p;
                    update('input', dir || p);
                  })
                }
              >
                选择 tileset.json
              </button>
            </div>
          ) : null}
        </FormSection>

        <FormSection title="处理选项">
          <div className="field">
            <Switch checked={form.rebuildTop} onChange={(v) => update('rebuildTop', v)}>
              顶层重建
            </Switch>
          </div>
          <div className="field">
            <label>纹理处理</label>
            <select
              className="select"
              value={form.textureMode}
              onChange={(e) => update('textureMode', e.target.value as TextureMode)}
            >
              <option value="keep">保留原纹理</option>
              <option value="ktx2-etc1s" disabled={!ktxOk}>
                KTX2 ETC1S{ktxOk ? '' : '（不可用）'}
              </option>
              <option value="ktx2" disabled={!ktxOk}>
                KTX2{ktxOk ? '' : '（不可用）'}
              </option>
              <option value="ktx2-uastc" disabled={!ktx2UastcEnabled(caps, true)}>
                KTX2 UASTC{ktx2UastcEnabled(caps, true) ? '' : '（不可用）'}
              </option>
            </select>
          </div>

          <AdvancedBlock open={advancedOpen} onToggle={setAdvancedOpen}>
            <div className="field">
              <label>重建质量</label>
              <select
                className="select"
                disabled={!form.rebuildTop}
                value={form.quality}
                onChange={(e) => {
                  const quality = e.target.value as RebuildQuality;
                  const preset = rebuildQualityOptions(quality, ktxOk);
                  setForm((f) => ({
                    ...f,
                    quality,
                    rebuildLevels: preset.levels,
                  }));
                }}
              >
                <option value="quality">质量优先</option>
                <option value="balanced">均衡</option>
                <option value="speed">性能优先</option>
              </select>
            </div>
            <div className="field">
              <label>重建层数</label>
              <select
                className="select"
                disabled={!form.rebuildTop}
                value={form.rebuildLevels}
                onChange={(e) => update('rebuildLevels', Number(e.target.value))}
              >
                <option value={0}>自动到根</option>
                <option value={1}>1</option>
                <option value={2}>2</option>
              </select>
              <div className="field-hint">
                {form.rebuildTop ? rebuildLevelsLabel(form.rebuildLevels) : '未启用顶层重建'}
              </div>
            </div>
            <details>
              <summary className="muted" style={{ cursor: 'pointer', fontSize: 13 }}>
                任务信息
              </summary>
              <div className="field" style={{ marginTop: 8 }}>
                <label>任务名</label>
                <input
                  className="input"
                  placeholder="可选"
                  value={form.name}
                  onChange={(e) => update('name', e.target.value)}
                />
              </div>
            </details>
          </AdvancedBlock>
        </FormSection>

        <FormSection title="输出">
          <PathField
            label="输出目录"
            value={form.output}
            onChange={(v) => {
              setOutputTouched(true);
              update('output', v);
            }}
            onPick={
              isTauri()
                ? () =>
                    void selectOutputDirectory().then((p) => {
                      if (p) {
                        setOutputTouched(true);
                        update('output', p);
                      }
                    })
                : undefined
            }
          />
        </FormSection>

        <SubmitBar
          onReset={handleReset}
          primaryLabel={submitting ? '提交中…' : '开始处理'}
          onPrimary={() => void handleSubmit()}
          primaryDisabled={submitting}
        />
      </div>
    </div>
  );
}
