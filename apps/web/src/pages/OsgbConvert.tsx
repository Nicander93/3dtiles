import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { api, friendlyError } from '../api/client';
import type { CapabilitiesResponse, OsgbScanResult, TextureMode } from '../api/types';
import { Alert } from '../components/Alert';
import { ktx2Etc1sEnabled, ktx2UastcEnabled } from '../lib/textureCaps';

const CONFIG_KEY = 'geoforge.osgb.convert.config';
const SAMPLE_INPUT = '/workspace/data/OSGBny/OSGBny';
const SAMPLE_OUTPUT = '/workspace/data/geoforge_outputs/osgbny_ui';

type FormState = {
  input: string;
  output: string;
  name: string;
  rebuildTop: boolean;
  rebuildLevels: number;
  textureMode: TextureMode;
  crsOverride: string;
  originX: string;
  originY: string;
  originZ: string;
  geographicExport: boolean;
};

const defaults: FormState = {
  input: SAMPLE_INPUT,
  output: SAMPLE_OUTPUT,
  name: 'OSGBny转换',
  rebuildTop: true,
  rebuildLevels: 1,
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
    next.rebuildLevels = next.rebuildLevels === 2 ? 2 : 1;
    // migrate old comma originOverride → x/y/z
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
  return (
    scan.geo?.effectiveCrs ||
    scan.summary?.srs ||
    scan.metadata?.srs ||
    null
  );
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
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [form, setForm] = useState<FormState>(() => loadConfig());
  const [scan, setScan] = useState<OsgbScanResult | null>(null);
  const [scanning, setScanning] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [caps, setCaps] = useState<CapabilitiesResponse | null>(null);

  useEffect(() => {
    void api.capabilities().then(setCaps).catch(() => setCaps(null));
  }, []);

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
  }, [searchParams]);

  const checklist = useMemo(() => {
    const hasMeta = scan
      ? Boolean(scan.metadata?.path || scan.hasMetadata || (scan.valid && !(scan.errors || []).some((e) => e.includes('metadata'))))
      : null;
    const hasData = scan
      ? Boolean(scan.hasDataDir ?? !(scan.errors || []).some((e) => e.includes('Data/')))
      : null;
    const tileCount = scan?.tileCount ?? scan?.summary?.tileCount ?? scan?.tiles?.length;
    const hasTiles = scan ? (tileCount ?? 0) > 0 : null;
    return [
      { key: 'metadata', label: 'metadata.xml', ok: hasMeta },
      { key: 'data', label: 'Data 目录', ok: hasData },
      { key: 'tiles', label: 'Tile_* 瓦片', ok: hasTiles, detail: tileCount != null ? `${tileCount} 个` : undefined },
    ];
  }, [scan]);

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

  const unitHint = useMemo(() => {
    if (scan?.unitHint && !form.crsOverride.trim()) return scan.unitHint;
    if (scan?.geo?.unitHint && !form.crsOverride.trim()) return scan.geo.unitHint;
    const crs = effectiveCrs || '';
    if (!crs) return '未知单位（缺 SRS）';
    if (/^\s*ENU\s*:/i.test(crs)) return 'ENU 局部坐标：米（东/北/天）；地理原点为经纬度（度）';
    if (/^\s*EPSG\s*:/i.test(crs)) return 'EPSG：请确认投影米或地理度；高程基准独立';
    return '自定义 CRS：请确认单位与轴序；高程基准独立';
  }, [scan, form.crsOverride, effectiveCrs]);

  const missingCrsForGeo = form.geographicExport && !effectiveCrs;

  const summary = useMemo(
    () => ({
      input: form.input || '（未填写）',
      output: form.output || '（未填写）',
      name: form.name || '自动命名',
      options: [
        form.rebuildTop ? `顶层重建 ×${form.rebuildLevels}` : '不重建顶层',
        form.textureMode === 'keep'
          ? '纹理 keep（保留原图）'
          : `纹理 ${form.textureMode}`,
        form.geographicExport ? '地理导出' : '本地/非强制地理',
      ].join(' · '),
      crs: effectiveCrs || '（缺 CRS）',
      origin: effectiveOrigin || '（缺原点）',
    }),
    [form, effectiveCrs, effectiveOrigin],
  );

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  async function handleScan() {
    setError(null);
    setMessage(null);
    if (!form.input.trim()) {
      setError('请先填写 OSGB 输入目录路径。');
      return;
    }
    setScanning(true);
    try {
      const result = await api.scanOsgb(form.input.trim());
      setScan(result);
      if (!result.valid) {
        setError(result.message || (result.errors || []).join('; ') || '输入目录校验未通过。');
      } else {
        const srs = scanSrs(result) || '（缺失）';
        const origin = scanOriginText(result) || '（缺失）';
        setMessage(
          `${result.message || '输入目录校验通过。'} 实际 CRS：${srs}；原点：${origin}`,
        );
        if (result.path && result.path !== form.input) {
          update('input', result.path);
        }
      }
    } catch (e) {
      setScan(null);
      setError(friendlyError(e));
    } finally {
      setScanning(false);
    }
  }

  function handleSave() {
    localStorage.setItem(CONFIG_KEY, JSON.stringify(form));
    setMessage('配置已保存到本地浏览器。');
    setError(null);
  }

  async function handleSubmit() {
    setError(null);
    setMessage(null);
    if (!form.input.trim() || !form.output.trim()) {
      setError('请填写输入目录与输出路径。');
      return;
    }
    if (missingCrsForGeo) {
      setError(
        '缺参数阻止地理导出：扫描未得到 SRS，且未填写 CRS 覆盖。请先扫描或填写 CRS，或关闭「地理导出」。',
      );
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
          },
          texture: { mode: form.textureMode },
          geo,
          geographicExport: form.geographicExport,
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

  const checksPassed = checklist.every((c) => c.ok === true);

  return (
    <div className="page">
      <div className="page-header">
        <div>
          <h1>OSGB转换</h1>
          <p>三步配置输入、处理参数与输出，提交本地转换任务</p>
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
            <label>OSGB 根目录</label>
            <div className="row">
              <input
                className="input"
                placeholder="含 Data/ + metadata.xml"
                value={form.input}
                onChange={(e) => update('input', e.target.value)}
              />
              <button className="btn" type="button" onClick={() => void handleScan()} disabled={scanning}>
                {scanning ? '扫描中…' : '扫描'}
              </button>
            </div>
            <div className="field-hint">默认样例：{SAMPLE_INPUT}</div>
          </div>

          <div className="section-title" style={{ marginBottom: 8 }}>
            校验清单
          </div>
          <div className="check-cards">
            {checklist.map((c) => {
              const state = c.ok == null ? 'pending' : c.ok ? 'ok' : 'bad';
              const mark = c.ok == null ? '○' : c.ok ? '✓' : '✗';
              const status =
                c.ok == null ? '待扫描' : c.ok ? (c.detail ? `已找到 · ${c.detail}` : '已找到') : '缺失';
              return (
                <div key={c.key} className={`check-card ${state}`}>
                  <div className="check-mark">{mark}</div>
                  <div className="check-body">
                    <strong>{c.label}</strong>
                    <span>{status}</span>
                  </div>
                </div>
              );
            })}
          </div>

          <div className="summary-box" style={{ marginTop: 12, borderColor: 'var(--accent, #2563eb)' }}>
            <div className="section-title" style={{ marginBottom: 8 }}>
              实际采用的 CRS / 原点
            </div>
            {scan || form.crsOverride.trim() || (form.originX && form.originY && form.originZ) ? (
              <dl>
                <dt>CRS</dt>
                <dd style={{ fontWeight: 600 }}>{effectiveCrs || '（缺失）'}</dd>
                <dt>原点 (SRSOrigin)</dt>
                <dd style={{ fontWeight: 600 }}>{effectiveOrigin || '（缺失）'}</dd>
                <dt>单位提示</dt>
                <dd>{unitHint}</dd>
                <dt>扫描 SRS</dt>
                <dd>{scanSrs(scan) || '（未扫描或缺失）'}</dd>
                <dt>扫描原点</dt>
                <dd>{scanOriginText(scan) || '（未扫描或缺失）'}</dd>
                <dt>OSGB 文件</dt>
                <dd>{scan?.summary?.osgbFileCount ?? '—'}</dd>
              </dl>
            ) : (
              <div className="muted">点击「扫描」后显示实际采用的 CRS、原点与单位提示。</div>
            )}
            {scan && !scanSrs(scan) && !form.crsOverride.trim() ? (
              <div className="field-hint" style={{ marginTop: 8, color: 'var(--danger, #b45309)' }}>
                缺少 SRS：可继续本地转换/预览；若勾选地理导出，须填写 CRS 覆盖。
              </div>
            ) : null}
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
              顶层重建（rebuild-top）
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
              <option value="keep">keep — 保留原纹理（推荐）</option>
              <option
                value="ktx2-etc1s"
                disabled={!ktx2Etc1sEnabled(caps)}
              >
                ktx2-etc1s — Basis ETC1S（体积优先）
                {!ktx2Etc1sEnabled(caps)
                  ? ' — 当前不可用'
                  : caps?.postprocessBasisu?.available || caps?.textureModes?.find((m) => m.mode === 'ktx2-etc1s')?.postprocess
                    ? ' — basisu 后处理'
                    : ''}
              </option>
              <option value="ktx2" disabled={!ktx2Etc1sEnabled(caps)}>
                ktx2 — 同 ETC1S 别名
              </option>
              <option
                value="ktx2-uastc"
                disabled={!ktx2UastcEnabled(caps)}
              >
                ktx2-uastc — Basis UASTC（质量优先）
                {!ktx2UastcEnabled(caps)
                  ? ' — 当前不可用'
                  : caps?.postprocessBasisu?.available
                    ? ' — basisu 后处理'
                    : ''}
              </option>
            </select>
            <div className="field-hint">
              {form.textureMode === 'keep'
                ? '默认 keep：不压缩，已在样例验证。'
                : form.textureMode === 'ktx2-uastc'
                  ? caps?.textureModes?.find((m) => m.mode === 'ktx2-uastc')?.supported
                    ? '将在转换后用 basisu -uastc 后处理为 KTX2（KHR_texture_basisu），并校验证据。'
                    : 'UASTC 需要 basisu 后处理工具；当前不可用。'
                  : caps?.textureModes?.find((m) => m.mode === 'ktx2-etc1s')?.supported
                    ? caps?.textureModes?.find((m) => m.mode === 'ktx2-etc1s')?.postprocess
                      ? '转换保持原纹理，随后用 basisu 后处理为 KTX2 ETC1S（KHR_texture_basisu），并校验证据。'
                      : '将向 3dtile 传递 --enable-texture-compress（ETC1S / KHR_texture_basisu），并校验输出证据。'
                    : caps?.textureModes?.find((m) => m.mode === 'ktx2-etc1s')?.reason ||
                      '当前无 KTX2 能力（二进制 flag 与 basisu 后处理均不可用）。'}
            </div>
          </div>

          <div className="field">
            <label className="switch">
              <input
                type="checkbox"
                checked={form.geographicExport}
                onChange={(e) => update('geographicExport', e.target.checked)}
              />
              地理导出 / 强制要求 CRS
            </label>
            <div className="field-hint">
              勾选后：若扫描无 SRS 且未填写 CRS 覆盖，提交将被阻止（缺参数阻止地理导出）。
            </div>
          </div>

          <div className="field">
            <label>CRS 覆盖（可选）</label>
            <input
              className="input"
              placeholder="例如 ENU:35.9,117.1 或 EPSG:4547 / WKT 文本"
              value={form.crsOverride}
              onChange={(e) => update('crsOverride', e.target.value)}
            />
            <div className="field-hint">
              ENU:lat,lon 会映射到 3dtile <code>-c</code> 的 x/y（经度/纬度）。EPSG/WKT
              写入任务选项；当前运行时二进制仍以 metadata.xml 为准（无独立 CRS CLI 标志）。
            </div>
          </div>
          <div className="field">
            <label>原点覆盖 X / Y / Z（可选，对应 SRSOrigin）</label>
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
            <div className="field-hint">
              写入任务选项；Z 可映射为 <code>-c offset</code>。局部东/北偏移 CLI 无独立标志，仍读
              metadata.xml。高程基准独立，首版不自动猜测。
            </div>
          </div>
          {missingCrsForGeo ? (
            <Alert kind="warn">
              已启用地理导出，但尚无有效 CRS。请扫描含 SRS 的数据，或填写 CRS 覆盖后再提交。
            </Alert>
          ) : null}
        </div>

        <div className="card card-pad step-card">
          <h2>
            <span className="step-num">3</span> 输出
          </h2>
          <div className="field">
            <label>输出路径</label>
            <input
              className="input"
              placeholder={SAMPLE_OUTPUT}
              value={form.output}
              onChange={(e) => update('output', e.target.value)}
            />
            <div className="field-hint">启用重建时成果目录为 输出路径_rebuild</div>
          </div>
          <div className="field">
            <label>任务名</label>
            <input
              className="input"
              placeholder="可选"
              value={form.name}
              onChange={(e) => update('name', e.target.value)}
            />
          </div>
          <div className="summary-box">
            <div className="section-title" style={{ marginBottom: 8 }}>
              任务摘要
            </div>
            <dl>
              <dt>输入</dt>
              <dd>{summary.input}</dd>
              <dt>输出</dt>
              <dd>{summary.output}</dd>
              <dt>名称</dt>
              <dd>{summary.name}</dd>
              <dt>选项</dt>
              <dd>{summary.options}</dd>
              <dt>实际 CRS</dt>
              <dd>{summary.crs}</dd>
              <dt>实际原点</dt>
              <dd>{summary.origin}</dd>
            </dl>
          </div>
        </div>
      </div>

      <div className="footer-actions">
        <div className="muted" style={{ flex: 1 }}>
          {missingCrsForGeo
            ? '地理导出缺 CRS，无法提交'
            : scan
              ? checksPassed
                ? '检查通过，可以提交任务'
                : '校验未通过，请修正输入后再提交'
              : '建议先扫描再提交'}
        </div>
        <button className="btn" type="button" onClick={handleSave}>
          保存配置
        </button>
        <button
          className="btn btn-primary"
          type="button"
          onClick={() => void handleSubmit()}
          disabled={submitting || missingCrsForGeo}
        >
          {submitting ? '提交中…' : '提交任务'}
        </button>
      </div>
    </div>
  );
}
