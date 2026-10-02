import { useEffect, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api, friendlyError, isTauri } from '../api/desktop';
import { Alert } from '../components/Alert';
import { FormSection } from '../components/FormSection';
import { SubmitBar } from '../components/SubmitBar';
import { suggestOutputPath } from '../lib/formUtils';
import { clipTaskRequest, restoreClipRegion, type ClipForm } from '../lib/clipTilesValidation';

const CONFIG_KEY = 'geoforge.tiles.clip.config';
const defaults: ClipForm = { input: '', output: '', name: '', mode: 'rectangle', bounds: ['', '', '', ''], geojson: '' };
function loadConfig(): ClipForm {
  try {
    const value = JSON.parse(localStorage.getItem(CONFIG_KEY) || 'null');
    if (value && ['input', 'output', 'name', 'geojson'].every((k) => typeof value[k] === 'string') && ['rectangle', 'polygon'].includes(value.mode) &&
      Array.isArray(value.bounds) && value.bounds.length === 4 && value.bounds.every((b: unknown) => typeof b === 'string')) return value;
  } catch { /* Optional storage. */ }
  return defaults;
}
export function ClipTiles() {
  const [params] = useSearchParams();
  const [form, setForm] = useState<ClipForm>(loadConfig);
  const [error, setError] = useState<string | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  useEffect(() => { try { localStorage.setItem(CONFIG_KEY, JSON.stringify(form)); } catch { /* Optional storage. */ } }, [form]);
  useEffect(() => {
    if (!params.get('input')) return;
    try { const region = params.get('region'); setForm({ ...defaults, input: params.get('input') || '', output: params.get('output') || '', name: params.get('name') || '', ...(region ? restoreClipRegion(JSON.parse(region)) : {}) }); }
    catch { setError('重新处理的区域参数无效，请重新填写。'); }
  }, [params]);
  function update(patch: Partial<ClipForm>) { setForm((current) => ({ ...current, ...patch })); setError(null); setTaskId(null); }
  async function submit() {
    setError(null); setTaskId(null);
    try { const request = clipTaskRequest(form); setSubmitting(true); const response = await api.createTask(request); const id = response.id || response.task?.id;
      if (!id) throw new Error('后端未返回任务编号，请检查处理记录。'); setTaskId(id);
    } catch (e) { setError(friendlyError(e)); } finally { setSubmitting(false); }
  }
  async function pick(output: boolean) {
    try { const path = output ? await api.selectOutputDirectory() : await api.selectTilesetFile(); if (path) update(output ? { output: suggestOutputPath('clipped', path, '_tiles') } : { input: path }); }
    catch (e) { setError(friendlyError(e)); }
  }
  return <div className="page">
    <div className="page-header"><h1>3D Tiles 范围裁剪</h1><p className="muted">精确保留范围内的几何，处理全部 LOD，并导出独立成果。</p></div>
    <div className="tool-form">
      {error ? <Alert kind="error">{error}</Alert> : null}
      {taskId ? <Alert kind="success">裁剪任务已创建。<Link to={`/processing?task=${encodeURIComponent(taskId)}`}>查看任务</Link></Alert> : null}
      <FormSection title="输入与输出">
        <div className="field"><label htmlFor="clip-input">Tileset 路径</label><div className="row">
          <input id="clip-input" className="input" disabled={submitting} value={form.input} onChange={(e) => update({ input: e.target.value })} placeholder="目录或 tileset.json" />
          {isTauri() ? <button type="button" className="btn" disabled={submitting} onClick={() => void pick(false)}>选择文件</button> : null}
        </div></div>
        <div className="field"><label htmlFor="clip-output">成果目录</label><div className="row">
          <input id="clip-output" className="input" disabled={submitting} value={form.output} onChange={(e) => update({ output: e.target.value })} />
          {isTauri() ? <button type="button" className="btn" disabled={submitting} onClick={() => void pick(true)}>选择父目录</button> : null}
        </div><p className="field-hint">成果目录须尚不存在且不与输入重叠；空结果会报告失败并清理临时文件。</p></div>
        <p className="field-hint">支持已有 ECEF 地理定位的显式静态 GLB / 无批次 B3DM 网格。保留纹理、材质、UV 与法线，不自动封口。动画、实例化、压缩网格和批次属性暂不支持。</p>
      </FormSection>
      <FormSection title="保留区域">
        <div className="field"><label htmlFor="clip-mode">区域输入方式</label><select id="clip-mode" className="input" value={form.mode} disabled={submitting} onChange={(e) => update({ mode: e.target.value as ClipForm['mode'] })}>
          <option value="rectangle">经纬度矩形</option><option value="polygon">凸多边形 GeoJSON</option>
        </select></div>
        {form.mode === 'rectangle' ? ['西经度', '南纬度', '东经度', '北纬度'].map((label, i) => <div className="field" key={label}><label htmlFor={`clip-bound-${i}`}>{label}（度）</label>
          <input id={`clip-bound-${i}`} className="input" type="number" step="any" disabled={submitting} value={form.bounds[i]} onChange={(e) => update({ bounds: form.bounds.map((v, n) => n === i ? e.target.value : v) })} />
        </div>) : <>
          <div className="field"><label htmlFor="clip-geojson">GeoJSON 区域</label><textarea id="clip-geojson" className="input" rows={10} disabled={submitting} value={form.geojson} onChange={(e) => update({ geojson: e.target.value })} placeholder='{"type":"Polygon","coordinates":[[[经度,纬度],…]]}' /></div>
          <div className="field"><label htmlFor="clip-import">导入 GeoJSON 文件</label><input id="clip-import" type="file" accept=".json,.geojson,application/json" disabled={submitting} onChange={(e) => {
            const file = e.target.files?.[0]; if (file) void file.text().then((geojson) => update({ geojson })).catch((e) => setError(friendlyError(e)));
          }} /></div>
        </>}
        <p className="field-hint">WGS84 经纬度，凸多边形须闭合且无孔洞。区域距中心不超过 10 公里，纬度在 ±80° 内，不跨日期变更线。边界按局部 ENU 竖直平面裁剪，不限制高度。</p>
      </FormSection>
      <FormSection title="任务"><div className="field"><label htmlFor="clip-name">任务名</label><input id="clip-name" className="input" disabled={submitting} value={form.name} onChange={(e) => update({ name: e.target.value })} placeholder="可选" /></div></FormSection>
      <SubmitBar primaryLabel={submitting ? '提交中…' : '开始裁剪'} primaryDisabled={submitting} onPrimary={() => void submit()} onReset={() => update(defaults)} />
    </div>
  </div>;
}
