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
import { canSubmitModelScan, modelScanMatchesCurrentInput } from '../lib/modelScanGate';
import { modelConvertValidationError } from '../lib/modelConvertValidation';

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
  georeferenceMode: 'local' | 'anchor' | 'projected';
  sourceCrs: string;
  axisMapping: 'eastNorthHeight' | 'northEastHeight';
  originX: string;
  originY: string;
  originZ: string;
};

const defaults: FormState = {
  input: '', output: '', name: '', format: 'fbx', unit: 'fromMetadata', axes: 'fromMetadata', longitude: '', latitude: '', height: '',
  georeferenceMode: 'local', sourceCrs: '', axisMapping: 'eastNorthHeight', originX: '0', originY: '0', originZ: '0',
};

function inferredFormat(path: string): ModelFormat {
  return path.toLowerCase().endsWith('.obj') ? 'obj' : 'fbx';
}

export function ModelConvert() {
  const [form, setForm] = useState<FormState>(defaults);
  const [scan, setScan] = useState<ModelScanResult | null>(null);
  const [scannedTextureRoots, setScannedTextureRoots] = useState<string[]>([]);
  const [scanPending, setScanPending] = useState(false);
  const [textureRoots, setTextureRoots] = useState<string[]>([]);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [capabilities, setCapabilities] = useState<CapabilitiesResponse | null>(null);
  const input = useDebouncedValue(form.input, 500);
  const scanGate = {
    scannedPath: scan?.path ?? null,
    currentPath: form.input,
    debouncedPath: input,
    scannedTextureRoots,
    textureRoots,
    scanPending,
    scanValid: scan?.valid === true,
    scannedFormat: scan?.format,
    selectedFormat: form.format,
  };
  const scanMatchesCurrentInput = modelScanMatchesCurrentInput(scanGate);
  const canSubmit = canSubmitModelScan(scanGate);
  const formError = modelConvertValidationError({
    ...form,
    projectedGeoreferenceSupported: capabilities?.model?.projectedGeoreference === true,
  });

  useEffect(() => {
    const path = input.trim();
    if (!path) {
      setScan(null);
      setScannedTextureRoots([]);
      setScanPending(false);
      return;
    }

    let active = true;
    setScan(null);
    setScanPending(true);
    void api.scanModel(path, textureRoots)
      .then((result) => { if (active) { setScan(result); setScannedTextureRoots(textureRoots); } })
      .catch((reason) => { if (active) { setScan(null); setScannedTextureRoots([]); setError(friendlyError(reason)); } })
      .finally(() => { if (active) setScanPending(false); });
    return () => { active = false; };
  }, [input, textureRoots]);

  useEffect(() => {
    void api.capabilities().then(setCapabilities).catch(() => setCapabilities(null));
  }, []);

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((current) => ({ ...current, [key]: value }));
    if (key === 'input') {
      setScan(null);
      setScanPending(Boolean(String(value).trim()));
      setError(null);
    }
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

  async function addTextureRoot() {
    try {
      const path = await api.selectTextureRoot();
      if (path && !textureRoots.some((root) => pathsEqual(root, path))) {
        setTextureRoots((roots) => [...roots, path]);
      }
    } catch (reason) {
      setError(friendlyError(reason));
    }
  }

  async function submit() {
    setError(null);
    if (capabilities?.model?.ready === false) return setError(capabilities.model.reason || '当前转换器不支持 FBX/OBJ 模型转换。');
    if (scanPending || !scanMatchesCurrentInput) return setError('请先等待当前模型文件预检完成。');
    if (scan && !scan.valid) return setError((scan.errors || []).join('；') || '模型预检未通过。');
    if (formError) return setError(formError);
    setSubmitting(true);
    try {
      const georeference = form.georeferenceMode === 'anchor'
        ? { mode: 'anchor', longitudeDeg: Number(form.longitude), latitudeDeg: Number(form.latitude), ellipsoidHeightM: Number(form.height) }
        : form.georeferenceMode === 'projected'
          ? { mode: 'projected', sourceCrs: form.sourceCrs.trim(), axisMapping: form.axisMapping, originOffset: [Number(form.originX), Number(form.originY), Number(form.originZ)] }
          : { mode: 'local' };
      const result = await api.createTask({
        operation: 'convert-model', input: { path: form.input.trim() }, output: { path: form.output.trim() }, taskName: form.name || undefined,
        options: { model: { format: form.format, unit: form.unit, axes: form.axes, missingTexturePolicy: 'error', textureRoots }, georeference, texture: { mode: 'keep' }, modelOutput: { format: '3dtiles-1.0', tiling: 'single', lod: false } },
      });
      const id = result.task?.id || result.id;
      if (id) window.location.assign(`/processing?task=${encodeURIComponent(id)}`);
    } catch (reason) { setError(friendlyError(reason)); }
    finally { setSubmitting(false); }
  }

  return <div className="page"><div className="page-header"><div><div className="page-crumb"><Link to="/">← 返回工具</Link><span>/</span><span>通用模型转换</span></div><h1>通用模型转换</h1></div></div><div className="page-form">
    {error ? <Alert kind="error">{error}</Alert> : null}
    {capabilities?.model?.ready === false ? <Alert kind="warn">{capabilities.model.reason || '当前转换器缺少模型转换能力，请更新运行组件。'}</Alert> : null}
    <FormSection title="输入"><PathField label="FBX / OBJ 文件" value={form.input} placeholder="例如 D:\\models\\building.fbx" onChange={(value) => { update('input', value); update('format', inferredFormat(value)); }} onPick={isTauri() ? () => void pickModelFile() : undefined} feedback={scanPending ? <div className="input-feedback">正在扫描模型及其材质资源…</div> : scanMatchesCurrentInput && scan ? <div className={scan.valid ? 'input-feedback ok' : 'input-feedback bad'}>{scan.valid ? <><span>已识别 {scan.format?.toUpperCase()} 模型</span>{scan.materials?.length ? <button className="btn-ghost btn-sm" type="button" onClick={() => setDetailsOpen((open) => !open)}>{detailsOpen ? '收起详情' : '资源详情'}</button> : null}</> : (scan.errors || []).join('；')}</div> : null} />
      {scan?.warnings?.map((warning) => <Alert key={warning} kind="warn">{warning}</Alert>)}</FormSection>
    {detailsOpen && scan?.materials?.length ? <div className="summary-box"><dl><dt>MTL 文件</dt><dd>{scan.summary?.materialLibraryCount ?? scan.materials.length}</dd>{scan.materials.map((material) => <><dt key={`${material.path}-label`}>{material.reference}</dt><dd key={material.path}>{material.exists ? `${material.textures?.filter((texture) => texture.exists).length ?? 0} 个贴图可用` : '缺失'}{material.textures?.filter((texture) => !texture.exists).map((texture) => <div key={texture.path} className="field-error">缺失贴图：{texture.reference}</div>)}</dd></>)}</dl></div> : null}
    <FormSection title="模型设置"><div className="field"><label>格式</label><select className="select" value={form.format} onChange={(event) => update('format', event.target.value as ModelFormat)}><option value="fbx">FBX</option><option value="obj">OBJ</option></select></div><div className="field"><label>单位</label><select className="select" value={form.unit} onChange={(event) => update('unit', event.target.value as FormState['unit'])}><option value="fromMetadata">从文件元数据读取</option><option value="meters">米</option><option value="centimeters">厘米</option><option value="millimeters">毫米</option><option value="feet">英尺</option></select></div><div className="field"><label>轴向</label><select className="select" value={form.axes} onChange={(event) => update('axes', event.target.value as FormState['axes'])}><option value="fromMetadata">从文件元数据读取</option><option value="yUpRightHanded">右手 Y 向上</option><option value="zUpRightHanded">右手 Z 向上</option></select></div></FormSection>
    <FormSection title="外部贴图目录"><p className="muted">当 FBX 或 OBJ 的材质引用不在模型同目录时，添加包含贴图的目录。</p><button className="btn-secondary" type="button" onClick={() => void addTextureRoot()} disabled={!isTauri()}>添加目录</button>{textureRoots.map((root) => <div className="summary-box" key={root}><span>{root}</span><button className="btn-ghost btn-sm" type="button" onClick={() => setTextureRoots((roots) => roots.filter((item) => !pathsEqual(item, root)))}>移除</button></div>)}</FormSection>
    <FormSection title="地理定位"><div className="field"><label>定位模式</label><select className="select" value={form.georeferenceMode} onChange={(event) => update('georeferenceMode', event.target.value as FormState['georeferenceMode'])}><option value="local">保留本地坐标</option><option value="anchor">WGS84 锚点放置</option><option value="projected" disabled={capabilities?.model?.projectedGeoreference === false}>已有投影坐标</option></select></div>
      {form.georeferenceMode === 'anchor' ? <div className="row"><input className="input" placeholder="经度" value={form.longitude} onChange={(event) => update('longitude', event.target.value)} /><input className="input" placeholder="纬度" value={form.latitude} onChange={(event) => update('latitude', event.target.value)} /><input className="input" placeholder="椭球高（米）" value={form.height} onChange={(event) => update('height', event.target.value)} /></div> : null}
      {form.georeferenceMode === 'projected' ? <><div className="field"><label>源 CRS</label><input className="input" placeholder="EPSG:4547 或完整 WKT2" value={form.sourceCrs} onChange={(event) => update('sourceCrs', event.target.value)} /></div><div className="field"><label>源坐标轴</label><select className="select" value={form.axisMapping} onChange={(event) => update('axisMapping', event.target.value as FormState['axisMapping'])}><option value="eastNorthHeight">E / N / H</option><option value="northEastHeight">N / E / H</option></select></div><div className="row"><input className="input" placeholder="原点偏移 E/N 或 N/E" value={form.originX} onChange={(event) => update('originX', event.target.value)} /><input className="input" placeholder="原点偏移 N/E 或 E/N" value={form.originY} onChange={(event) => update('originY', event.target.value)} /><input className="input" placeholder="原点偏移 H" value={form.originZ} onChange={(event) => update('originZ', event.target.value)} /></div><Alert kind="warn">投影转换会逐顶点重投影；请确认 CRS、轴顺序和原点偏移均与源模型一致。</Alert></> : null}</FormSection>
    <FormSection title="输出"><PathField label="成果目录" value={form.output} onChange={(value) => update('output', value)} /></FormSection>
    <SubmitBar onReset={() => { setForm(defaults); setScan(null); setScannedTextureRoots([]); setScanPending(false); setTextureRoots([]); setError(null); }} primaryLabel={submitting ? '提交中…' : '开始转换'} onPrimary={() => void submit()} primaryDisabled={submitting || scanPending || !canSubmit || Boolean(formError) || capabilities?.model?.ready === false} />
  </div></div>;
}
