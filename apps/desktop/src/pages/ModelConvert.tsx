import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { Drawer } from '../components/Drawer';
import { PageHeader } from '../components/PageHeader';
import { api, friendlyError, isTauri } from '../api/desktop';
import type { ModelScanResult } from '../api/types';
import type { CapabilitiesResponse } from '../api/types';
import { Alert } from '../components/Alert';
import { FormSection } from '../components/FormSection';
import { PathField } from '../components/PathField';
import { SubmitBar } from '../components/SubmitBar';
import { pathsEqual, useDebouncedValue } from '../lib/formUtils';
import { canSubmitModelScan, modelScanMatchesCurrentInput } from '../lib/modelScanGate';
import {
  buildModelOutputPath,
  modelConvertValidationError,
  modelOutputPathError,
} from '../lib/modelConvertValidation';

type ModelFormat = 'fbx' | 'obj';
type FormState = {
  input: string;
  outputParent: string;
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
  input: '',
  outputParent: '',
  name: '',
  format: 'fbx',
  unit: 'fromMetadata',
  axes: 'fromMetadata',
  longitude: '',
  latitude: '',
  height: '',
  georeferenceMode: 'local',
  sourceCrs: '',
  axisMapping: 'eastNorthHeight',
  originX: '0',
  originY: '0',
  originZ: '0',
};

function inferredFormat(path: string): ModelFormat {
  return path.toLowerCase().endsWith('.obj') ? 'obj' : 'fbx';
}

function createOutputId(): string {
  return `${Date.now().toString(36)}-${crypto.randomUUID().slice(0, 8)}`;
}

export function ModelConvert() {
  const navigate = useNavigate();
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [form, setForm] = useState<FormState>(defaults);
  const [outputId, setOutputId] = useState(createOutputId);
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
  const outputPath = buildModelOutputPath(form.input, form.outputParent, outputId);
  const outputError = modelOutputPathError(form.input, outputPath);
  const formError = modelConvertValidationError({
    ...form,
    output: outputPath,
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
    void api
      .scanModel(path, textureRoots)
      .then((result) => {
        if (active) {
          setScan(result);
          setScannedTextureRoots(textureRoots);
        }
      })
      .catch((reason) => {
        if (active) {
          setScan(null);
          setScannedTextureRoots([]);
          setError(friendlyError(reason));
        }
      })
      .finally(() => {
        if (active) setScanPending(false);
      });
    return () => {
      active = false;
    };
  }, [input, textureRoots]);

  useEffect(() => {
    void api
      .capabilities()
      .then(setCapabilities)
      .catch(() => setCapabilities(null));
  }, []);

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((current) => ({ ...current, [key]: value }));
    if (key === 'input') {
      setScan(null);
      setScanPending(Boolean(String(value).trim()));
      setError(null);
    }
    if (key === 'outputParent') setError(null);
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

  async function pickOutputParent() {
    try {
      const path = await api.selectOutputDirectory();
      if (!path) return;
      update('outputParent', path);
      setOutputId(createOutputId());
    } catch (reason) {
      setError(friendlyError(reason));
    }
  }

  async function submit() {
    setError(null);
    if (capabilities?.model?.ready === false)
      return setError(capabilities.model.reason || '当前转换器不支持 FBX/OBJ 模型转换。');
    if (scanPending || !scanMatchesCurrentInput) return setError('请先等待当前模型文件预检完成。');
    if (scan && !scan.valid) return setError((scan.errors || []).join('；') || '模型预检未通过。');
    if (formError) return setError(formError);
    setSubmitting(true);
    try {
      const georeference =
        form.georeferenceMode === 'anchor'
          ? {
              mode: 'anchor',
              longitudeDeg: Number(form.longitude),
              latitudeDeg: Number(form.latitude),
              ellipsoidHeightM: Number(form.height),
            }
          : form.georeferenceMode === 'projected'
            ? {
                mode: 'projected',
                sourceCrs: form.sourceCrs.trim(),
                axisMapping: form.axisMapping,
                originOffset: [Number(form.originX), Number(form.originY), Number(form.originZ)],
              }
            : { mode: 'local' };
      const result = await api.createTask({
        operation: 'convert-model',
        input: { path: form.input.trim() },
        output: { path: outputPath },
        taskName: form.name || undefined,
        options: {
          model: {
            format: form.format,
            unit: form.unit,
            axes: form.axes,
            missingTexturePolicy: 'error',
            textureRoots,
          },
          georeference,
          texture: { mode: 'keep' },
          modelOutput: { format: '3dtiles-1.0', tiling: 'single', lod: false },
        },
      });
      const id = result.task?.id || result.id;
      if (id) navigate(`/processing?task=${encodeURIComponent(id)}`);
    } catch (reason) {
      setError(friendlyError(reason));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="page">
      <PageHeader
        title="通用模型转换"
        description="将 FBX / OBJ 转换为 3D Tiles。"
        actions={
          <button className="btn" type="button" onClick={() => setAdvancedOpen(true)}>
            高级设置
          </button>
        }
      />
      <div className="page-form">
        {error ? <Alert kind="error">{error}</Alert> : null}
        {capabilities?.model?.ready === false ? (
          <Alert kind="warn">
            {capabilities.model.reason || '当前转换器缺少模型转换能力，请更新运行组件。'}
          </Alert>
        ) : null}
        <FormSection title="数据" description="选择 FBX 或 OBJ 文件。">
          <PathField
            label="模型文件"
            value={form.input}
            placeholder="例如 D:\\models\\building.fbx"
            onChange={(value) => {
              update('input', value);
              update('format', inferredFormat(value));
            }}
            onPick={isTauri() ? () => void pickModelFile() : undefined}
            pickLabel="选择文件"
            feedback={
              scanPending ? (
                <div className="input-feedback">正在扫描模型及其材质资源…</div>
              ) : scanMatchesCurrentInput && scan ? (
                <div className={scan.valid ? 'input-feedback ok' : 'input-feedback bad'}>
                  {scan.valid ? (
                    <>
                      <span>已识别 {scan.format?.toUpperCase()} 模型</span>
                      {scan.materials?.length ? (
                        <button
                          className="btn btn-ghost btn-sm"
                          type="button"
                          onClick={() => setDetailsOpen((open) => !open)}
                        >
                          {detailsOpen ? '收起详情' : '资源详情'}
                        </button>
                      ) : null}
                    </>
                  ) : (
                    (scan.errors || []).join('；')
                  )}
                </div>
              ) : null
            }
          />
          {scan?.warnings?.map((warning) => (
            <Alert key={warning} kind="warn">
              {warning}
            </Alert>
          ))}
          {detailsOpen && scan?.materials?.length ? (
            <div className="summary-box">
              <dl>
                <dt>MTL 文件</dt>
                <dd>{scan.summary?.materialLibraryCount ?? scan.materials.length}</dd>
                {scan.materials.map((material) => (
                  <dd className="material-detail" key={material.path}>
                    <strong>{material.reference}</strong> ·{' '}
                    {material.exists
                      ? `${material.textures?.filter((texture) => texture.exists).length ?? 0} 个贴图可用`
                      : '缺失'}
                    {material.textures
                      ?.filter((texture) => !texture.exists)
                      .map((texture) => (
                        <div key={texture.path} className="field-error">
                          缺失贴图：{texture.reference}
                        </div>
                      ))}
                  </dd>
                ))}
              </dl>
            </div>
          ) : null}
        </FormSection>
        <FormSection title="处理" description="格式、单位与定位" columns={3}>
          <div className="field">
            <label htmlFor="model-format">格式</label>
            <select
              id="model-format"
              className="select"
              value={form.format}
              onChange={(event) => update('format', event.target.value as ModelFormat)}
            >
              <option value="fbx">FBX</option>
              <option value="obj">OBJ</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="model-unit">单位</label>
            <select
              id="model-unit"
              className="select"
              value={form.unit}
              onChange={(event) => update('unit', event.target.value as FormState['unit'])}
            >
              <option value="fromMetadata">从文件元数据读取</option>
              <option value="meters">米</option>
              <option value="centimeters">厘米</option>
              <option value="millimeters">毫米</option>
              <option value="feet">英尺</option>
            </select>
          </div>
          <div className="field">
            <label htmlFor="model-placement">定位模式</label>
            <select
              id="model-placement"
              className="select"
              value={form.georeferenceMode}
              onChange={(event) =>
                update('georeferenceMode', event.target.value as FormState['georeferenceMode'])
              }
            >
              <option value="local">保留本地坐标</option>
              <option value="anchor">WGS84 锚点放置</option>
              <option
                value="projected"
                disabled={capabilities?.model?.projectedGeoreference === false}
              >
                已有投影坐标
              </option>
            </select>
          </div>
          {form.georeferenceMode === 'anchor' ? (
            <div className="form-grid cols-3 span-all">
              <div className="field">
                <label htmlFor="model-lon">经度</label>
                <input
                  id="model-lon"
                  className="input"
                  placeholder="经度"
                  value={form.longitude}
                  onChange={(event) => update('longitude', event.target.value)}
                />
              </div>
              <div className="field">
                <label htmlFor="model-lat">纬度</label>
                <input
                  id="model-lat"
                  className="input"
                  placeholder="纬度"
                  value={form.latitude}
                  onChange={(event) => update('latitude', event.target.value)}
                />
              </div>
              <div className="field">
                <label htmlFor="model-height">椭球高（米）</label>
                <input
                  id="model-height"
                  className="input"
                  placeholder="椭球高（米）"
                  value={form.height}
                  onChange={(event) => update('height', event.target.value)}
                />
              </div>
            </div>
          ) : null}
          {form.georeferenceMode === 'projected' ? (
            <div className="form-grid cols-2 span-all">
              <div className="field">
                <label htmlFor="model-crs">源 CRS</label>
                <input
                  id="model-crs"
                  className="input"
                  placeholder="EPSG:4547 或完整 WKT2"
                  value={form.sourceCrs}
                  onChange={(event) => update('sourceCrs', event.target.value)}
                />
              </div>
              <div className="field">
                <label htmlFor="model-mapping">源坐标轴</label>
                <select
                  id="model-mapping"
                  className="select"
                  value={form.axisMapping}
                  onChange={(event) =>
                    update('axisMapping', event.target.value as FormState['axisMapping'])
                  }
                >
                  <option value="eastNorthHeight">E / N / H</option>
                  <option value="northEastHeight">N / E / H</option>
                </select>
              </div>
              <div className="field span-all">
                <label>原点偏移</label>
                <div className="row">
                  <input
                    aria-label="原点偏移 X"
                    className="input"
                    placeholder="原点偏移 E/N 或 N/E"
                    value={form.originX}
                    onChange={(event) => update('originX', event.target.value)}
                  />
                  <input
                    aria-label="原点偏移 Y"
                    className="input"
                    placeholder="原点偏移 N/E 或 E/N"
                    value={form.originY}
                    onChange={(event) => update('originY', event.target.value)}
                  />
                  <input
                    aria-label="原点偏移 Z"
                    className="input"
                    placeholder="原点偏移 H"
                    value={form.originZ}
                    onChange={(event) => update('originZ', event.target.value)}
                  />
                </div>
              </div>
              <div className="span-all">
                <Alert kind="warn">
                  投影转换会逐顶点重投影；请确认 CRS、轴顺序和原点偏移均与源模型一致。
                </Alert>
              </div>
            </div>
          ) : null}
        </FormSection>
        <FormSection title="输出" description="自动生成新的成果目录。">
          <PathField
            label="保存位置（已有文件夹）"
            value={form.outputParent}
            onChange={(value) => update('outputParent', value)}
            onPick={isTauri() ? () => void pickOutputParent() : undefined}
            pickLabel="选择文件夹"
            error={outputError}
            hint={
              outputPath
                ? `将新建成果目录：${outputPath}。不会覆盖已有目录。`
                : '请选择保存位置，应用会在其中新建成果目录。'
            }
          />
        </FormSection>
        <Drawer title="高级设置" open={advancedOpen} onClose={() => setAdvancedOpen(false)}>
          <div className="field">
            <label htmlFor="model-axes">轴向</label>
            <select
              id="model-axes"
              className="select"
              value={form.axes}
              onChange={(event) => update('axes', event.target.value as FormState['axes'])}
            >
              <option value="fromMetadata">从文件元数据读取</option>
              <option value="yUpRightHanded">右手 Y 向上</option>
              <option value="zUpRightHanded">右手 Z 向上</option>
            </select>
          </div>
          <section className="drawer-group">
            <h3>外部贴图目录</h3>
            <p className="field-hint">材质引用不在模型同目录时，添加包含贴图的目录。</p>
            <button
              className="btn"
              type="button"
              onClick={() => void addTextureRoot()}
              disabled={!isTauri()}
            >
              添加目录
            </button>
            {textureRoots.map((root) => (
              <div className="texture-root" key={root}>
                <span title={root}>{root}</span>
                <button
                  className="btn btn-ghost btn-sm"
                  type="button"
                  onClick={() =>
                    setTextureRoots((roots) => roots.filter((item) => !pathsEqual(item, root)))
                  }
                >
                  移除
                </button>
              </div>
            ))}
            {!isTauri() ? <p className="field-hint">请在桌面应用中选择贴图目录。</p> : null}
          </section>
          <div className="field">
            <label htmlFor="model-name">任务名</label>
            <input
              id="model-name"
              className="input"
              placeholder="可选，自动命名"
              value={form.name}
              onChange={(event) => update('name', event.target.value)}
            />
          </div>
        </Drawer>
        <SubmitBar
          onReset={() => {
            setForm(defaults);
            setOutputId(createOutputId());
            setScan(null);
            setScannedTextureRoots([]);
            setScanPending(false);
            setTextureRoots([]);
            setError(null);
          }}
          primaryLabel={submitting ? '提交中…' : '开始转换'}
          onPrimary={() => void submit()}
          primaryDisabled={
            submitting ||
            scanPending ||
            !canSubmit ||
            Boolean(formError) ||
            capabilities?.model?.ready === false
          }
          hint={
            formError ||
            (scanPending
              ? '等待模型预检完成'
              : !canSubmit
                ? '选择模型并完成预检后即可转换'
                : '3D Tiles · 保留原纹理')
          }
        />
      </div>
    </div>
  );
}
