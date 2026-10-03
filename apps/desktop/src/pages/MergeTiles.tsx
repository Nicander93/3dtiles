import { useEffect, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { api, friendlyError, isTauri } from '../api/desktop';
import { Alert } from '../components/Alert';
import { FormSection } from '../components/FormSection';
import { SubmitBar } from '../components/SubmitBar';
import { suggestOutputPath } from '../lib/formUtils';
import { MAX_MERGE_INPUTS, mergeTaskRequest, validateMergeForm, type MergeForm } from '../lib/mergeTilesValidation';

const CONFIG_KEY = 'geoforge.tiles.merge.config';
const defaults: MergeForm = { inputs: ['', ''], output: '', name: '' };

function loadConfig(): MergeForm {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(CONFIG_KEY) || 'null');
    if (saved && typeof saved === 'object') {
      const form = saved as Partial<MergeForm>;
      if (Array.isArray(form.inputs) && form.inputs.length >= 2 && form.inputs.length <= MAX_MERGE_INPUTS && form.inputs.every((p) => typeof p === 'string')) {
        return { inputs: form.inputs, output: typeof form.output === 'string' ? form.output : '', name: typeof form.name === 'string' ? form.name : '' };
      }
    }
  } catch { /* Ignore unavailable or invalid local storage. */ }
  return defaults;
}

export function MergeTiles() {
  const [params] = useSearchParams();
  const [form, setForm] = useState<MergeForm>(loadConfig);
  const [error, setError] = useState<string | null>(null);
  const [taskId, setTaskId] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    try { localStorage.setItem(CONFIG_KEY, JSON.stringify(form)); } catch { /* Storage is optional. */ }
  }, [form]);

  useEffect(() => {
    const inputs = params.getAll('input');
    if (inputs.length >= 2) setForm({ inputs, output: params.get('output') || '', name: params.get('name') || '' });
  }, [params]);

  function updateInput(index: number, path: string) {
    setForm((current) => ({ ...current, inputs: current.inputs.map((p, i) => i === index ? path : p) }));
    setError(null); setTaskId(null);
  }

  async function pickInput(index: number) {
    try {
      const path = await api.selectTilesetFile();
      if (path) updateInput(index, path);
    } catch (e) { setError(friendlyError(e)); }
  }

  async function pickOutput() {
    try {
      const parent = await api.selectOutputDirectory();
      if (parent) setForm((current) => ({ ...current, output: suggestOutputPath('merged', parent, '_tiles') }));
    } catch (e) { setError(friendlyError(e)); }
  }

  async function submit() {
    const problem = validateMergeForm(form);
    setError(problem); setTaskId(null);
    if (problem) return;
    setSubmitting(true);
    try {
      const response = await api.createTask(mergeTaskRequest(form));
      const id = response.id || response.task?.id;
      if (!id) throw new Error('后端未返回任务编号，请检查处理记录。');
      setTaskId(id);
    } catch (e) { setError(friendlyError(e)); }
    finally { setSubmitting(false); }
  }

  return <div className="page">
    <div className="page-header"><h1>3D Tiles 合并</h1><p className="muted">将多份本地 Tileset 汇总为一个可独立移动的成果目录。</p></div>
    <div className="tool-form">
      {error ? <Alert kind="error">{error}</Alert> : null}
      {taskId ? <Alert kind="success">合并任务已创建。<Link to={`/processing?task=${encodeURIComponent(taskId)}`}>查看任务</Link></Alert> : null}
      <FormSection title="输入">
        <p className="field-hint">输入应使用同一坐标参考系；合并保留原坐标，不自动定位或转换坐标系。支持本地显式 Tileset，源目录中的资源将一并复制。</p>
        {form.inputs.map((path, index) => <div className="field" key={index}>
          <label htmlFor={`merge-input-${index}`}>Tileset {index + 1}</label>
          <div className="row">
            <input id={`merge-input-${index}`} className="input" value={path} placeholder="目录或 tileset.json 路径" disabled={submitting} onChange={(e) => updateInput(index, e.target.value)} />
            {isTauri() ? <button className="btn" type="button" disabled={submitting} onClick={() => void pickInput(index)}>选择文件</button> : null}
            <button className="btn" type="button" aria-label={`移除 Tileset ${index + 1}`} disabled={submitting || form.inputs.length <= 2} onClick={() => setForm((current) => ({ ...current, inputs: current.inputs.filter((_, i) => i !== index) }))}>移除</button>
          </div>
        </div>)}
        <button className="btn" type="button" disabled={submitting || form.inputs.length >= MAX_MERGE_INPUTS} onClick={() => setForm((current) => ({ ...current, inputs: [...current.inputs, ''] }))}>添加 Tileset</button>
      </FormSection>
      <FormSection title="输出">
        <div className="field"><label htmlFor="merge-output">成果目录</label><div className="row">
          <input id="merge-output" className="input" value={form.output} disabled={submitting} onChange={(e) => setForm({ ...form, output: e.target.value })} />
          {isTauri() ? <button className="btn" type="button" disabled={submitting} onClick={() => void pickOutput()}>选择父目录</button> : null}
        </div><p className="field-hint">请选择尚不存在的目录；复制的模型、纹理和原有层级保留在各自子目录中。</p></div>
        <div className="field"><label htmlFor="merge-name">任务名</label><input id="merge-name" className="input" value={form.name} disabled={submitting} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="可选" /></div>
      </FormSection>
      <SubmitBar primaryLabel={submitting ? '提交中…' : '开始合并'} primaryDisabled={submitting} onPrimary={() => void submit()} onReset={() => { setForm(defaults); setError(null); setTaskId(null); }} />
    </div>
  </div>;
}
