import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { api, friendlyError } from '../api/client';
import type { Artifact, CapabilitiesResponse, TextureMode } from '../api/types';
import { Alert } from '../components/Alert';
import { ktx2Etc1sEnabled, ktx2UastcEnabled } from '../lib/textureCaps';

const SAMPLE_INPUT = '/workspace/data/geoforge_outputs/osgbny_v1';
const SAMPLE_OUTPUT = '/workspace/data/geoforge_outputs/osgbny_v1_process';

type FormState = {
  input: string;
  output: string;
  name: string;
  artifactId: string;
  rebuildTop: boolean;
  rebuildLevels: number;
  textureMode: TextureMode;
};

const defaults: FormState = {
  input: SAMPLE_INPUT,
  output: SAMPLE_OUTPUT,
  name: 'Tiles处理',
  artifactId: '',
  rebuildTop: true,
  rebuildLevels: 1,
  textureMode: 'keep',
};

export function ProcessTiles() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [form, setForm] = useState<FormState>(defaults);
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [caps, setCaps] = useState<CapabilitiesResponse | null>(null);

  useEffect(() => {
    void api.capabilities().then(setCaps).catch(() => setCaps(null));
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        setArtifacts(await api.listArtifacts());
      } catch {
        /* ignore — path input still works */
      }
    })();
  }, []);

  useEffect(() => {
    const input = searchParams.get('input') || searchParams.get('path');
    const output = searchParams.get('output');
    const artifact = searchParams.get('artifact');
    const name = searchParams.get('name');
    if (!input && !output && !artifact && !name) return;
    setForm((f) => ({
      ...f,
      ...(input ? { input } : {}),
      ...(output ? { output } : {}),
      ...(artifact ? { artifactId: artifact } : {}),
      ...(name ? { name } : {}),
    }));
  }, [searchParams]);

  // When artifact picked, fill input path
  useEffect(() => {
    if (!form.artifactId) return;
    const a = artifacts.find((x) => x.id === form.artifactId);
    if (a?.path) {
      setForm((f) => ({
        ...f,
        input: a.path,
        output: f.output.includes('_process') ? f.output : `${a.path}_process`,
        name: f.name || a.label || 'Tiles处理',
      }));
    }
  }, [form.artifactId, artifacts]);

  const summary = useMemo(
    () => ({
      input: form.input || '（未填写）',
      output: form.output || '（未填写）',
      options: [
        form.rebuildTop ? `顶层重建 ×${form.rebuildLevels}` : '不重建',
        form.textureMode === 'keep' ? '纹理 keep（跳过）' : `纹理 ${form.textureMode}`,
      ].join(' · '),
    }),
    [form],
  );

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  async function handleSubmit() {
    setError(null);
    setMessage(null);
    if (!form.input.trim() || !form.output.trim()) {
      setError('请填写输入 tileset 目录与输出路径。');
      return;
    }
    if (!form.rebuildTop && form.textureMode === 'keep') {
      setError('process-tileset 需要启用顶层重建，或选择非 keep 纹理模式。');
      return;
    }
    const pt = caps?.textureModes?.find((m) => m.mode === (form.textureMode === 'ktx2' ? 'ktx2-etc1s' : form.textureMode));
    if (form.textureMode !== 'keep' && pt?.processTileset?.supported === false) {
      setError(pt.processTileset.reason || '当前无法在 process-tileset 路径做 KTX2；请用 convert-osgb 或 keep。');
      return;
    }
    setSubmitting(true);
    try {
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
          },
          texture: { mode: form.textureMode },
        },
      });
      const id = res.task?.id || res.id;
      setMessage(id ? `任务已提交：${id}` : '任务已提交。');
      navigate('/processing');
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>处理已有 Tiles</h1>
          <p>对已有 3D Tiles 目录执行顶层重建 / 纹理（process-tileset）</p>
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

      <div className="steps-3">
        <div className="card card-pad step-card">
          <h2>
            <span className="step-num">1</span> 输入
          </h2>
          <div className="field">
            <label>从成果选择（可选）</label>
            <select
              className="select"
              value={form.artifactId}
              onChange={(e) => update('artifactId', e.target.value)}
            >
              <option value="">— 手动填写路径 —</option>
              {artifacts.map((a) => (
                <option key={a.id} value={a.id}>
                  {(a.label || a.id) + (a.has_tileset || a.hasTileset ? '' : '（无 tileset）')} — {a.path}
                </option>
              ))}
            </select>
          </div>
          <div className="field">
            <label>Tileset 目录</label>
            <input
              className="input"
              placeholder={SAMPLE_INPUT}
              value={form.input}
              onChange={(e) => update('input', e.target.value)}
            />
            <div className="field-hint">需包含 tileset.json。样例：{SAMPLE_INPUT}</div>
          </div>
        </div>

        <div className="card card-pad step-card">
          <h2>
            <span className="step-num">2</span> 处理
          </h2>
          <div className="field">
            <label className="switch">
              <input
                type="checkbox"
                checked={form.rebuildTop}
                onChange={(e) => update('rebuildTop', e.target.checked)}
              />
              顶层重建（rebuildTop）
            </label>
          </div>
          <div className="field">
            <label>重建层数（rebuildTop.levels）</label>
            <select
              className="select"
              disabled={!form.rebuildTop}
              value={form.rebuildLevels === 2 ? 2 : 1}
              onChange={(e) => update('rebuildLevels', Number(e.target.value) === 2 ? 2 : 1)}
            >
              <option value={1}>1 — 一层 2×2 合并（默认）</option>
              <option value={2}>2 — 两层金字塔（--levels 2）</option>
            </select>
            <div className="field-hint">对应 rebuild_top.py --levels；仅支持 1 或 2</div>
          </div>
          <div className="field">
            <label>纹理模式</label>
            <select
              className="select"
              value={form.textureMode}
              onChange={(e) => update('textureMode', e.target.value as FormState['textureMode'])}
            >
              <option value="keep">keep — 跳过（推荐）</option>
              <option
                value="ktx2-etc1s"
                disabled={!ktx2Etc1sEnabled(caps, true)}
              >
                ktx2-etc1s — basisu 后处理
              </option>
              <option
                value="ktx2"
                disabled={!ktx2Etc1sEnabled(caps, true)}
              >
                ktx2 — 同 ETC1S 别名
              </option>
              <option
                value="ktx2-uastc"
                disabled={!ktx2UastcEnabled(caps, true)}
              >
                ktx2-uastc — basisu UASTC 后处理
              </option>
            </select>
            <div className="field-hint">
              {caps?.postprocessBasisu?.available
                ? '可用 basisu 对已有 B3DM/GLB 做 KTX2 后处理（KHR_texture_basisu），无需新的 _3dtile 二进制。'
                : '上游 --enable-texture-compress 仅在 OSGB 转换时生效；当前无 basisu 后处理，KTX2 不可用。'}
            </div>
          </div>
        </div>

        <div className="card card-pad step-card">
          <h2>
            <span className="step-num">3</span> 输出
          </h2>
          <div className="field">
            <label>输出路径</label>
            <input
              className="input"
              value={form.output}
              onChange={(e) => update('output', e.target.value)}
            />
          </div>
          <div className="field">
            <label>任务名</label>
            <input
              className="input"
              value={form.name}
              onChange={(e) => update('name', e.target.value)}
            />
          </div>
          <div className="summary-box">
            <div className="section-title" style={{ marginBottom: 8 }}>
              任务摘要
            </div>
            <dl>
              <dt>操作</dt>
              <dd>process-tileset</dd>
              <dt>输入</dt>
              <dd>{summary.input}</dd>
              <dt>输出</dt>
              <dd>{summary.output}</dd>
              <dt>选项</dt>
              <dd>{summary.options}</dd>
            </dl>
          </div>
        </div>
      </div>

      <div className="footer-actions">
        <div className="muted" style={{ flex: 1 }}>
          独立路径：不重新转换 OSGB，只处理已有 Tiles
        </div>
        <button
          className="btn btn-primary"
          type="button"
          onClick={() => void handleSubmit()}
          disabled={submitting}
        >
          {submitting ? '提交中…' : '提交 process-tileset'}
        </button>
      </div>
    </div>
  );
}
