import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { api, friendlyError, isTauri } from '../api/desktop';
import type { ModelScanResult } from '../api/types';
import type { CapabilitiesResponse } from '../api/types';
import { Alert } from '../components/Alert';
import { FormSection } from '../components/FormSection';
import { PathField } from '../components/PathField';
import { SubmitBar } from '../components/SubmitBar';
import { pathsEqual, useDebouncedValue } from '../lib/formUtils';

type ModelFormat = 'fbx' | 'obj';
type FormState = {
  input: string;
  output: string;
  name: string;
  format: ModelFormat;
  unit: 'fromMetadata' | 'meters' | 'centimeters' | 'millimeters' | 'feet';
  axes: 'fromMetadata' | 'yUpRightHanded' | 'zUpRightHanded';
  longitude: string;
  latitude: string;
  height: string;
};

const defaults: FormState = {
  input: '', output: '', name: '', format: 'fbx', unit: 'fromMetadata', axes: 'fromMetadata', longitude: '', latitude: '', height: '',
};

function inferredFormat(path: string): ModelFormat {
  return path.toLowerCase().endsWith('.obj') ? 'obj' : 'fbx';
}

export function ModelConvert() {
  const [form, setForm] = useState<FormState>(defaults);
  const [scan, setScan] = useState<ModelScanResult | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [capabilities, setCapabilities] = useState<CapabilitiesResponse | null>(null);
  const input = useDebouncedValue(form.input, 500);

  useEffect(() => {
    if (!input.trim()) { setScan(null); return; }
    void api.scanModel(input.trim()).then(setScan).catch((reason) => setError(friendlyError(reason)));
  }, [input]);

  useEffect(() => {
    void api.capabilities().then(setCapabilities).catch(() => setCapabilities(null));
  }, []);

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  async function pickModelFile() {
    try {
      const path = await api.selectModelFile();
      if (!path) return;
      update('input', path);
      update('format', inferredFormat(path));
    } catch (reason) {
      setError(friendlyError(reason));
    }
  }

  async function submit() {
    setError(null);
    if (!form.input.trim() || !form.output.trim()) return setError('请填写模型文件和输出目录。');
    if (capabilities?.model?.ready === false) return setError(capabilities.model.reason || '当前转换器不支持 FBX/OBJ 模型转换。');
    if (pathsEqual(form.input, form.output)) return setError('输出目录不能与模型文件相同。');
    if (scan && !scan.valid) return setError((scan.errors || []).join('；') || '模型预检未通过。');
    if (form.format === 'obj' && (form.unit === 'fromMetadata' || form.axes === 'fromMetadata')) {
      return setError('OBJ 不可靠地声明单位和轴向，请明确选择。');
    }
    const hasAnchor = form.longitude || form.latitude || form.height;
    if (hasAnchor && (!form.longitude || !form.latitude || !form.height)) return setError('锚点定位需要完整的经度、纬度和椭球高。');
    setSubmitting(true);
    try {
      const georeference = hasAnchor
        ? { mode: 'anchor', longitudeDeg: Number(form.longitude), latitudeDeg: Number(form.latitude), ellipsoidHeightM: Number(form.height) }
        : { mode: 'local' };
      const result = await api.createTask({
        operation: 'convert-model', input: { path: form.input.trim() }, output: { path: form.output.trim() }, taskName: form.name || undefined,
        options: { model: { format: form.format, unit: form.unit, axes: form.axes, missingTexturePolicy: 'error', textureRoots: [] }, georeference, texture: { mode: 'keep' }, modelOutput: { format: '3dtiles-1.0', tiling: 'single', lod: false } },
      });
      const id = result.task?.id || result.id;
      if (id) window.location.assign(`/processing?task=${encodeURIComponent(id)}`);
    } catch (reason) { setError(friendlyError(reason)); }
    finally { setSubmitting(false); }
  }

  return <div className="page"><div className="page-header"><div><div className="page-crumb"><Link to="/">← 返回工具</Link><span>/</span><span>通用模型转换</span></div><h1>通用模型转换</h1></div></div><div className="page-form">
    {error ? <Alert kind="error">{error}</Alert> : null}
    {capabilities?.model?.ready === false ? <Alert kind="warn">{capabilities.model.reason || '当前转换器缺少模型转换能力，请更新运行组件。'}</Alert> : null}
    <FormSection title="输入"><PathField label="FBX / OBJ 文件" value={form.input} placeholder="例如 D:\\models\\building.fbx" onChange={(value) => { update('input', value); update('format', inferredFormat(value)); }} onPick={isTauri() ? () => void pickModelFile() : undefined} feedback={scan ? <div className={scan.valid ? 'input-feedback ok' : 'input-feedback bad'}>{scan.valid ? <><span>已识别 {scan.format?.toUpperCase()} 模型</span>{scan.materials?.length ? <button className="btn-ghost btn-sm" type="button" onClick={() => setDetailsOpen((open) => !open)}>{detailsOpen ? '收起详情' : '资源详情'}</button> : null}</> : (scan.errors || []).join('；')}</div> : null} />
      {scan?.warnings?.map((warning) => <Alert key={warning} kind="warn">{warning}</Alert>)}</FormSection>
    {detailsOpen && scan?.materials?.length ? <div className="summary-box"><dl><dt>MTL 文件</dt><dd>{scan.summary?.materialLibraryCount ?? scan.materials.length}</dd>{scan.materials.map((material) => <><dt key={`${material.path}-label`}>{material.reference}</dt><dd key={material.path}>{material.exists ? `${material.textures?.filter((texture) => texture.exists).length ?? 0} 个贴图可用` : '缺失'}{material.textures?.filter((texture) => !texture.exists).map((texture) => <div key={texture.path} className="field-error">缺失贴图：{texture.reference}</div>)}</dd></>)}</dl></div> : null}
    <FormSection title="模型设置"><div className="field"><label>格式</label><select className="select" value={form.format} onChange={(event) => update('format', event.target.value as ModelFormat)}><option value="fbx">FBX</option><option value="obj">OBJ</option></select></div><div className="field"><label>单位</label><select className="select" value={form.unit} onChange={(event) => update('unit', event.target.value as FormState['unit'])}><option value="fromMetadata">从文件元数据读取</option><option value="meters">米</option><option value="centimeters">厘米</option><option value="millimeters">毫米</option><option value="feet">英尺</option></select></div><div className="field"><label>轴向</label><select className="select" value={form.axes} onChange={(event) => update('axes', event.target.value as FormState['axes'])}><option value="fromMetadata">从文件元数据读取</option><option value="yUpRightHanded">右手 Y 向上</option><option value="zUpRightHanded">右手 Z 向上</option></select></div></FormSection>
    <FormSection title="地理定位（可选锚点）"><div className="row"><input className="input" placeholder="经度" value={form.longitude} onChange={(event) => update('longitude', event.target.value)} /><input className="input" placeholder="纬度" value={form.latitude} onChange={(event) => update('latitude', event.target.value)} /><input className="input" placeholder="椭球高（米）" value={form.height} onChange={(event) => update('height', event.target.value)} /></div></FormSection>
    <FormSection title="输出"><PathField label="成果目录" value={form.output} onChange={(value) => update('output', value)} /></FormSection>
    <SubmitBar onReset={() => { setForm(defaults); setScan(null); setError(null); }} primaryLabel={submitting ? '提交中…' : '开始转换'} onPrimary={() => void submit()} primaryDisabled={submitting || capabilities?.model?.ready === false} />
  </div></div>;
}
