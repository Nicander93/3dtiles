import { useEffect, useRef, useState, type RefObject } from 'react';
import { Link } from 'react-router-dom';
import { api, friendlyError, isTauri } from '../api/desktop';
import type { Artifact, Task } from '../api/types';
import { clipTaskRequest, restoreClipRegion } from '../lib/clipTilesValidation';
import { drawnClipRegion, readDrawingMessage, readFlattenMessage, type DrawMode } from '../lib/previewClipRegion';
import { suggestOutputPath } from '../lib/formUtilsCore';
import { Alert } from './Alert';

interface Props {
  input: string;
  name: string;
  frame: RefObject<HTMLIFrameElement>;
  onResult: (artifact: Artifact) => void;
  onClose: () => void;
  operation?: 'clip' | 'flatten';
  sourceArtifactId?: string;
}

export function PreviewClipPanel({ input, name, frame, onResult, onClose, operation = 'clip', sourceArtifactId }: Props) {
  const flatten = operation === 'flatten';
  const title = flatten ? '区域压平' : '范围裁剪';
  const [mode, setMode] = useState<DrawMode>('rectangle');
  const [drawing, setDrawing] = useState<ReturnType<typeof readDrawingMessage>>(null);
  const [output, setOutput] = useState(() => suggestOutputPath(input.replace(/[/\\][^/\\]+\.json$/i, ''), '', `_${flatten ? 'flattened' : 'clipped'}_${Date.now().toString(36)}`));
  const [plane, setPlane] = useState<ReturnType<typeof readFlattenMessage>>(null);
  const regionKey = useRef('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [task, setTask] = useState<Task | null>(null);
  const [result, setResult] = useState<Artifact | null>(null);
  const busy = submitting || Boolean(task && !['succeeded', 'completed', 'failed', 'cancelled', 'interrupted'].includes(task.status));
  let region: unknown = null, regionError = '';
  try { if (drawing?.complete) region = drawnClipRegion(drawing.mode, drawing.points); }
  catch (e) { regionError = friendlyError(e); }
  function command(action: string, drawMode = mode) {
    frame.current?.contentWindow?.postMessage({ type: 'geoforge-draw', action, mode: drawMode }, window.location.origin);
  }
  function planeCommand(action: string) {
    frame.current?.contentWindow?.postMessage({ type: 'geoforge-flatten', action, region,
      regionKey: JSON.stringify(region), samples: drawing?.points.map((p,i) => [...p,drawing.heights?.[i] || 0]) }, window.location.origin);
  }
  useEffect(() => {
    function receive(ev: MessageEvent) {
      if (ev.source !== frame.current?.contentWindow || ev.origin !== window.location.origin) return;
      const value = readDrawingMessage(ev.data);
      if (value) {
        let key = '';
        try { if (value.complete) key = JSON.stringify(drawnClipRegion(value.mode,value.points)); } catch { /* Region error is rendered below. */ }
        if (key !== regionKey.current) {
          regionKey.current = key; setPlane(null);
          frame.current?.contentWindow?.postMessage({ type: 'geoforge-flatten', action: 'clear' }, window.location.origin);
        }
        setDrawing(value); setError('');
      }
      const telemetry = readFlattenMessage(ev.data);
      if (telemetry && telemetry.regionKey === regionKey.current) setPlane(telemetry);
    }
    window.addEventListener('message', receive);
    return () => {
      window.removeEventListener('message', receive);
      frame.current?.contentWindow?.postMessage({ type: 'geoforge-draw', action: 'clear' }, window.location.origin);
      frame.current?.contentWindow?.postMessage({ type: 'geoforge-draw', action: 'stop' }, window.location.origin);
      frame.current?.contentWindow?.postMessage({ type: 'geoforge-flatten', action: 'clear' }, window.location.origin);
    };
  }, [frame]);
  useEffect(() => {
    if (!task?.id || ['failed', 'cancelled', 'interrupted'].includes(task.status)) return;
    let disposed = false, polling = false;
    async function poll() {
      if (polling) return;
      polling = true;
      try {
        const current = await api.getTask(task!.id);
        if (disposed) return;
        setTask(current);
        if (['succeeded', 'completed'].includes(current.status)) {
          const list = await api.listArtifacts();
          const artifact = list.find((a) => a.taskId === current.id || a.id === current.artifactId);
          if (!disposed && artifact) { setResult(artifact); window.clearInterval(timer); }
        } else if (['failed', 'cancelled', 'interrupted'].includes(current.status)) {
          setError(current.error || current.message || '裁剪任务未完成，请查看任务详情。');
          window.clearInterval(timer);
        }
      } catch (e) { if (!disposed) setError(friendlyError(e)); }
      finally { polling = false; }
    }
    void poll();
    const timer = window.setInterval(() => void poll(), 1500);
    return () => { disposed = true; window.clearInterval(timer); };
    // Status updates keep the same polling lifecycle until this panel closes or a new task is submitted.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [task?.id]);
  async function submit() {
    setError('');
    try {
      if (!region) throw new Error(regionError || '请先完成区域绘制。');
      if (flatten && !plane) throw new Error('请先显示压平工具并拖动目标平面。');
      const request = clipTaskRequest({ input, output, name: `${name || '模型'} · ${title}`, ...restoreClipRegion(region) });
      if (flatten && plane) { request.operation = 'flatten-tileset'; (request as { options: Record<string,unknown> }).options = { flatten: { region, heightMeters: plane.heightMeters }, preview: { sourceArtifactId } }; }
      setSubmitting(true);
      const response = await api.createTask(request);
      const id = response.id || response.task?.id;
      if (!id) throw new Error('后端未返回任务编号。');
      command('stop');
      planeCommand('stop');
      setResult(null); setTask({ id, name: request.taskName || '', operation: request.operation, input, output, status: 'queued' });
    } catch (e) { setError(friendlyError(e)); }
    finally { setSubmitting(false); }
  }
  return <aside className="info-panel preview-operation-panel" aria-label="模型操作">
    <div className="row"><h3>{title}</h3><button className="btn btn-sm" type="button" onClick={onClose}>关闭操作</button></div>
    <p className="muted">{flatten ? '画出区域，拖动竖直手柄调整目标平面，导出新模型。' : '绘制要保留的区域，导出新的 3D Tiles 模型。'}</p>
    <div className="field"><label htmlFor="preview-draw-mode">绘制方式</label><select id="preview-draw-mode" className="input" value={mode} disabled={busy} onChange={(e) => { const next = e.target.value as DrawMode; setMode(next); command('clear'); command('stop'); }}>
      <option value="rectangle">矩形</option><option value="polygon">凸多边形</option>
    </select></div>
    <button className="btn" type="button" disabled={busy} onClick={() => { planeCommand('clear'); setPlane(null); setTask(null); setResult(null); setDrawing(null); command('start'); }}>开始绘制</button>
    <p className="field-hint">{mode === 'rectangle' ? '在模型上按住左键拖出矩形。' : '左键点选顶点，右键或点击完成绘制闭合区域。'}完成后可拖动黄色顶点调整。{flatten ? '区域内的全部表面移到目标面，边界补连接侧壁。' : '区域不限高度，不自动封口。'}</p>
    <div className="row">
      <button className="btn btn-sm" type="button" disabled={busy || !drawing?.points.length} onClick={() => command('undo')}>撤销顶点</button>
      <button className="btn btn-sm" type="button" disabled={busy || !drawing?.points.length} onClick={() => command('clear')}>清空区域</button>
      <button className="btn btn-sm" type="button" disabled={busy || !drawing || drawing.complete || drawing.points.length < (mode === 'rectangle' ? 2 : 3)} onClick={() => command('finish')}>完成绘制</button>
    </div>
    <p role="status">{drawing?.complete ? `区域已闭合 · ${drawing.points.length} 个控制点` : drawing?.active ? `已绘制 ${drawing.points.length} 个控制点` : '尚未开始绘制'}</p>
    {regionError ? <Alert kind="error">{regionError}</Alert> : null}
    {region ? <details><summary>区域参数</summary><pre className="preview-region-json">{JSON.stringify(region, null, 2)}</pre></details> : null}
    {flatten ? <>
      <button className="btn" type="button" disabled={busy || !region} onClick={() => { command('stop'); planeCommand('start'); }}>显示压平拖拽工具</button>
      {plane ? <>
        <output aria-label="目标平面高度">目标平面：{plane.heightMeters.toFixed(2)} m</output>
        <p className="field-hint">沿青色箭头拖动；俯视时可抓住圆点上下拖动。拖动时锁定相机。高度相对区域中心的椭球面，沿当地竖直方向计算。</p>
        <div className="row"><button className="btn btn-sm" type="button" disabled={busy || !plane.hasUndo} onClick={() => planeCommand('undo')}>撤销高度调整</button><button className="btn btn-sm" type="button" disabled={busy} onClick={() => planeCommand('reset')}>恢复原高度</button></div>
        <p className="field-hint">青色目标面和示意侧壁实时更新；精确分割、纹理和侧壁以导出模型为准。新侧壁沿用表面材质，纹理按米铺设。</p>
      </> : null}
    </> : null}
    <div className="field"><label htmlFor="preview-clip-output">{flatten ? '压平成果目录' : '裁剪成果目录'}</label><input id="preview-clip-output" className="input" value={output} disabled={busy} onChange={(e) => setOutput(e.target.value)} />
      {isTauri() ? <button className="btn btn-sm" type="button" disabled={busy} onClick={() => void api.selectOutputDirectory().then((path) => { if (path) setOutput(suggestOutputPath('clipped', path, `_${Date.now().toString(36)}`)); }).catch((e) => setError(friendlyError(e)))}>选择父目录</button> : null}
    </div>
    <p className="field-hint">仅支持已有地理定位的静态 GLB / 无批次 B3DM；区域距中心不超过 10 公里。成果目录须尚不存在。</p>
    {error ? <Alert kind="error">{error}</Alert> : null}
    <button className="btn btn-primary" type="button" disabled={busy || !region || (flatten && !plane)} onClick={() => void submit()}>{busy ? '正在处理…' : flatten ? '导出压平模型' : '导出裁剪模型'}</button>
    {task ? <div role="status"><p>{title}任务：{task.status}</p><Link to={`/processing?task=${encodeURIComponent(task.id)}`}>查看{flatten ? '压平' : '裁剪'}任务</Link></div> : null}
    {result ? <button className="btn btn-primary" type="button" onClick={() => onResult(result)}>打开{flatten ? '压平' : '裁剪'}后模型</button> : null}
  </aside>;
}
