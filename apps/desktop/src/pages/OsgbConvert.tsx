import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api, friendlyError } from '../api/desktop';
import type { CapabilitiesResponse, OsgbScanResult, TextureMode } from '../api/types';
import { AdvancedBlock } from '../components/AdvancedBlock';
import { Alert } from '../components/Alert';
import { FormSection } from '../components/FormSection';
import { PathField } from '../components/PathField';
import { SubmitBar } from '../components/SubmitBar';
import { Switch } from '../components/Switch';
import { convertReady } from '../lib/convertReady';
import { pathsEqual, suggestOutputPath, useDebouncedValue } from '../lib/formUtils';
import {
  inferRebuildQuality,
  rebuildLevelsLabel,
  rebuildQualityOptions,
  type RebuildQuality,
} from '../lib/rebuildQuality';
import { ktx2Etc1sEnabled, ktx2UastcEnabled } from '../lib/textureCaps';
import { isTauri, selectInputDirectory, selectOutputDirectory } from '../lib/tauri';

const CONFIG_KEY = 'geoforge.osgb.convert.config';
const OUTPUT_TOUCHED_KEY = 'geoforge.osgb.convert.outputTouched';

type FormState = {
  input: string;
  output: string;
  name: string;
  rebuildTop: boolean;
  quality: RebuildQuality;
  rebuildLevels: number;
  textureMode: TextureMode;
  crsOverride: string;
  originX: string;
  originY: string;
  originZ: string;
  geographicExport: boolean;
};

const defaults: FormState = {
  input: '',
  output: '',
  name: '',
  rebuildTop: true,
  quality: 'balanced',
  rebuildLevels: 0,
  textureMode: 'keep',
  crsOverride: '',
  originX: '',
  originY: '',
  originZ: '',
  geographicExport: false,
};

function loadConfig(): FormState {
  try {
    const raw = localStorage.getItem(CONFIG_KEY);
    if (!raw) return defaults;
    const parsed = JSON.parse(raw) as Partial<FormState> & { originOverride?: string };
    const next: FormState = { ...defaults, ...parsed };
    const savedQuality = (parsed as { quality?: RebuildQuality }).quality;
    next.quality = savedQuality ?? inferRebuildQuality(next.rebuildLevels, next.textureMode);
    if (savedQuality) {
      next.rebuildLevels = [0, 1, 2].includes(next.rebuildLevels) ? next.rebuildLevels : 0;
    } else {
      next.rebuildLevels = 0;
    }
    if ((!next.originX || !next.originY || !next.originZ) && parsed.originOverride) {
      const parts = String(parsed.originOverride)
        .split(/[,;\s]+/)
        .filter(Boolean);
      if (parts.length >= 3) {
        next.originX = parts[0];
        next.originY = parts[1];
        next.originZ = parts[2];
      }
    }
    return next;
  } catch {
    return defaults;
  }
}

function scanSrs(scan: OsgbScanResult | null): string | null {
  if (!scan) return null;
  return scan.geo?.effectiveCrs || scan.summary?.srs || scan.metadata?.srs || null;
}

function scanOriginText(scan: OsgbScanResult | null): string | null {
  if (!scan) return null;
  return (
    scan.geo?.effectiveOrigin?.text ||
    scan.summary?.srsOrigin ||
    scan.metadata?.srsOrigin ||
    null
  );
}

export function OsgbConvert() {
  const [searchParams] = useSearchParams();
  const [form, setForm] = useState<FormState>(() => loadConfig());
  const [outputTouched, setOutputTouched] = useState(() => {
    try {
      return localStorage.getItem(OUTPUT_TOUCHED_KEY) === '1';
    } catch {
      return false;
    }
  });
  const [scan, setScan] = useState<OsgbScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState<ReactNode>(null);
  const [error, setError] = useState<string | null>(null);
  const [caps, setCaps] = useState<CapabilitiesResponse | null>(null);
  const [defaultOutputRoot, setDefaultOutputRoot] = useState('');
  const scanSeq = useRef(0);
  const debouncedInput = useDebouncedValue(form.input, 500);

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

  useEffect(() => {
    const input = searchParams.get('input') || searchParams.get('path');
    const output = searchParams.get('output');
    const name = searchParams.get('name');
    if (!input && !output && !name) return;
    setForm((f) => ({
      ...f,
      ...(input ? { input } : {}),
      ...(output ? { output } : {}),
      ...(name ? { name } : {}),
    }));
    if (output) setOutputTouched(true);
  }, [searchParams]);

  // Auto-suggest output when input changes and user hasn't touched output
  useEffect(() => {
    if (outputTouched) return;
    const suggested = suggestOutputPath(form.input, defaultOutputRoot, '_tiles');
    if (suggested && suggested !== form.output) {
      setForm((f) => ({ ...f, output: suggested }));
    }
  }, [form.input, defaultOutputRoot, outputTouched]);

  async function runScan(path: string) {
    const trimmed = path.trim();
    if (!trimmed) {
      setScan(null);
      setScanError(null);
      return;
    }
    const seq = ++scanSeq.current;
    setScanning(true);
    setScanError(null);
    try {
      const result = await api.scanOsgb(trimmed);
      if (seq !== scanSeq.current) return;
      setScan(result);
      if (!result.valid) {
        setScanError(result.message || (result.errors || []).join('; ') || '输入目录校验未通过。');
      } else {
        setScanError(null);
        if (result.path && result.path !== form.input) {
          setForm((f) => ({ ...f, input: result.path }));
        }
        if (!scanSrs(result) && form.geographicExport) {
          setAdvancedOpen(true);
        }
      }
    } catch (e) {
      if (seq !== scanSeq.current) return;
      setScan(null);
      setScanError(friendlyError(e));
    } finally {
      if (seq === scanSeq.current) setScanning(false);
    }
  }

  useEffect(() => {
    void runScan(debouncedInput);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debouncedInput]);

  const effectiveCrs = useMemo(() => {
    const override = form.crsOverride.trim();
    if (override) return override;
    return scanSrs(scan);
  }, [form.crsOverride, scan]);

  const effectiveOrigin = useMemo(() => {
    if (form.originX.trim() && form.originY.trim() && form.originZ.trim()) {
      return `${form.originX.trim()},${form.originY.trim()},${form.originZ.trim()}`;
    }
    return scanOriginText(scan);
  }, [form.originX, form.originY, form.originZ, scan]);

  const missingCrsForGeo = form.geographicExport && !effectiveCrs;
  const convertHint = convertReady(caps);

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((f) => ({ ...f, [key]: value }));
    if (key === 'input') {
      setScan(null);
      setScanError(null);
    }
  }

  function handleReset() {
    setForm({ ...defaults });
    setOutputTouched(false);
    setScan(null);
    setScanError(null);
    setMessage(null);
    setError(null);
    setAdvancedOpen(false);
    try {
      localStorage.removeItem(CONFIG_KEY);
      localStorage.removeItem(OUTPUT_TOUCHED_KEY);
    } catch {
      /* ignore */
    }
  }

  async function pickInputDirectory() {
    try {
      const path = await selectInputDirectory();
      if (path) update('input', path);
    } catch (error) {
      setError(friendlyError(error));
    }
  }

  async function pickOutputParent() {
    try {
      const parent = await selectOutputDirectory();
      if (!parent) return;
      const output = suggestOutputPath(form.input, parent, '_tiles');
      if (!output) {
        setError('请先选择输入目录，再选择输出位置。');
        return;
      }
      setOutputTouched(true);
      update('output', output);
    } catch (error) {
      setError(friendlyError(error));
    }
  }

  async function handleSubmit() {
    setError(null);
    setMessage(null);
    if (!form.input.trim() || !form.output.trim()) {
      setError('请填写输入目录与输出路径。');
      return;
    }
    if (pathsEqual(form.input, form.output)) {
      setError('输出目录不能与输入目录相同，请更换输出位置。');
      return;
    }
    if (missingCrsForGeo) {
      setAdvancedOpen(true);
      setError('缺少坐标信息，请补充坐标系，或关闭地理导出。');
      return;
    }
    if (scan && !scan.valid) {
      setError(scanError || '输入校验未通过，请修正后再提交。');
      return;
    }
    setSubmitting(true);
    try {
      const geo: Record<string, unknown> = {
        geographicExport: form.geographicExport,
      };
      if (form.crsOverride.trim()) geo.crs = form.crsOverride.trim();
      if (form.originX.trim() && form.originY.trim() && form.originZ.trim()) {
        geo.originX = Number(form.originX.trim());
        geo.originY = Number(form.originY.trim());
        geo.originZ = Number(form.originZ.trim());
        geo.origin = `${form.originX.trim()},${form.originY.trim()},${form.originZ.trim()}`;
      }
      const preset = rebuildQualityOptions(form.quality, ktx2Etc1sEnabled(caps));
      const res = await api.createTask({
        operation: 'convert-osgb',
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
          geo,
          geographicExport: form.geographicExport,
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
      // Keep the fresh unique path fixed. Clearing this flag would let the
      // input auto-suggest effect immediately replace it with the old `_tiles`
      // path after the successful submission.
      setOutputTouched(true);
      const nextOut = suggestOutputPath(form.input, defaultOutputRoot, `_tiles_${Date.now().toString(36)}`);
      if (nextOut) setForm((f) => ({ ...f, output: nextOut }));
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setSubmitting(false);
    }
  }

  const inputFeedback = scanning ? (
    <div className="input-feedback muted">正在识别…</div>
  ) : scanError ? (
    <div className="input-feedback bad">{scanError}</div>
  ) : scan?.valid ? (
    <div className="input-feedback ok">
      <span>已识别 OSGB 数据</span>
      <button className="btn-ghost btn-sm" type="button" onClick={() => setDetailsOpen((v) => !v)}>
        {detailsOpen ? '收起' : '详情'}
      </button>
    </div>
  ) : null;

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <div className="page-crumb">
            <Link to="/">← 返回工具</Link>
            <span>/</span>
            <span>OSGB 转换</span>
          </div>
          <h1>OSGB 转换</h1>
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
        {convertHint.via === 'docker' ? (
          <div style={{ marginBottom: 12 }}>
            <Alert kind="warn">
              {convertHint.message}{' '}
              <Link to="/settings">打开设置</Link>
            </Alert>
          </div>
        ) : null}
        {convertHint.via === 'none' ? (
          <div style={{ marginBottom: 12 }}>
            <Alert kind="error">
              {convertHint.message}{' '}
              <Link to="/settings">打开设置</Link>
            </Alert>
          </div>
        ) : null}

        <FormSection title="输入">
          <PathField
            label="数据目录"
            value={form.input}
            placeholder="含 Data/ 与 metadata.xml"
            onChange={(v) => update('input', v)}
            onBlur={() => void runScan(form.input)}
            onPick={
              isTauri() ? () => void pickInputDirectory() : undefined
            }
            feedback={inputFeedback}
          />
          {detailsOpen && scan ? (
            <div className="summary-box" style={{ marginTop: 8 }}>
              <dl>
                <dt>CRS</dt>
                <dd>{effectiveCrs || '（缺失）'}</dd>
                <dt>原点</dt>
                <dd>{effectiveOrigin || '（缺失）'}</dd>
                <dt>瓦片</dt>
                <dd>{scan.tileCount ?? scan.summary?.tileCount ?? '—'}</dd>
                <dt>OSGB 文件</dt>
                <dd>{scan.summary?.osgbFileCount ?? '—'}</dd>
              </dl>
              {!scanSrs(scan) && !form.crsOverride.trim() ? (
                <div className="field-hint" style={{ marginTop: 8, color: 'var(--warning)' }}>
                  缺少坐标信息，请补充坐标系。可继续本地转换；若需要地理定位，请在高级设置中填写 CRS。
                </div>
              ) : null}
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
              <option value="ktx2-etc1s" disabled={!ktx2Etc1sEnabled(caps)}>
                KTX2 ETC1S{ktx2Etc1sEnabled(caps) ? '' : '（不可用）'}
              </option>
              <option value="ktx2" disabled={!ktx2Etc1sEnabled(caps)}>
                KTX2{ktx2Etc1sEnabled(caps) ? '' : '（不可用）'}
              </option>
              <option value="ktx2-uastc" disabled={!ktx2UastcEnabled(caps)}>
                KTX2 UASTC{ktx2UastcEnabled(caps) ? '' : '（不可用）'}
              </option>
            </select>
            <div className="field-hint">
              {form.textureMode === 'keep'
                ? '保留原始纹理，体积较大。'
                : form.textureMode === 'ktx2-uastc'
                  ? 'UASTC 质量优先，体积介于原图与 ETC1S 之间。'
                  : 'ETC1S / KTX2 可明显减小体积，适合网络分发。'}
            </div>
          </div>

          <AdvancedBlock
            open={advancedOpen}
            onToggle={setAdvancedOpen}
            title="高级设置"
          >
            <div className="field">
              <label>重建质量</label>
              <select
                className="select"
                disabled={!form.rebuildTop}
                value={form.quality}
                onChange={(e) => {
                  const quality = e.target.value as RebuildQuality;
                  const preset = rebuildQualityOptions(quality, ktx2Etc1sEnabled(caps));
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
            <div className="field">
              <Switch
                checked={form.geographicExport}
                onChange={(v) => {
                  update('geographicExport', v);
                  if (v && !effectiveCrs) setAdvancedOpen(true);
                }}
              >
                地理导出（要求坐标系）
              </Switch>
            </div>
            <div className="field">
              <label>CRS 覆盖</label>
              <input
                className="input"
                placeholder="例如 ENU:35.9,117.1 或 EPSG:4547"
                value={form.crsOverride}
                onChange={(e) => update('crsOverride', e.target.value)}
              />
              {!effectiveCrs ? (
                <div className="field-error">缺少坐标信息，请补充坐标系</div>
              ) : null}
            </div>
            <div className="field">
              <label>原点覆盖 X / Y / Z</label>
              <div className="row">
                <input
                  className="input"
                  placeholder="X"
                  value={form.originX}
                  onChange={(e) => update('originX', e.target.value)}
                />
                <input
                  className="input"
                  placeholder="Y"
                  value={form.originY}
                  onChange={(e) => update('originY', e.target.value)}
                />
                <input
                  className="input"
                  placeholder="Z"
                  value={form.originZ}
                  onChange={(e) => update('originZ', e.target.value)}
                />
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
                  placeholder="可选，自动命名"
                  value={form.name}
                  onChange={(e) => update('name', e.target.value)}
                />
              </div>
            </details>
          </AdvancedBlock>
        </FormSection>

        <FormSection title="输出">
          <PathField
            label="成果目录（选择父目录后自动生成）"
            value={form.output}
            onChange={(v) => {
              setOutputTouched(true);
              update('output', v);
            }}
            onPick={
              isTauri() ? () => void pickOutputParent() : undefined
            }
          />
        </FormSection>

        <SubmitBar
          onReset={handleReset}
          primaryLabel={submitting ? '提交中…' : '开始转换'}
          onPrimary={() => void handleSubmit()}
          primaryDisabled={submitting || convertHint.via === 'none'}
        />
      </div>
    </div>
  );
}
