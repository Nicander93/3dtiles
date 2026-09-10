import type {
  ApiTask,
  Artifact,
  CapabilitiesResponse,
  CreateTaskRequest,
  CreateTaskResponse,
  HealthResponse,
  NativePreviewResponse,
  OsgbScanResult,
  PrepareOsgbResponse,
  PreviewUrlResponse,
  Task,
  TaskStageInfo,
} from './types';

function resolveApiBase(): string {
  const fromEnv = (import.meta.env.VITE_API_BASE ?? '').replace(/\/$/, '');
  if (fromEnv) return fromEnv;
  // Browser / HTTP fallback: Python desktop_server. Tauri uses api/desktop.ts commands.
  if (
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
  ) {
    return 'http://127.0.0.1:8787';
  }
  return '';
}

const API_BASE = resolveApiBase();

/** Prefix relative API/artifact paths for Tauri WebView (no Vite proxy). */
export function absolutizeLocalUrl(url: string): string {
  if (!url) return url;
  if (/^https?:\/\//i.test(url) || url.startsWith('blob:') || url.startsWith('data:')) {
    return url;
  }
  if (url.startsWith('/') && API_BASE) {
    return `${API_BASE}${url}`;
  }
  return url;
}

export class ApiError extends Error {
  status: number;
  body?: unknown;

  constructor(message: string, status: number, body?: unknown) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.body = body;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const url = `${API_BASE}${path}`;
  let res: Response;
  try {
    res = await fetch(url, {
      ...init,
      headers: {
        Accept: 'application/json',
        ...(init?.body ? { 'Content-Type': 'application/json' } : {}),
        ...init?.headers,
      },
    });
  } catch {
    throw new ApiError('无法连接后端服务，请确认本地 API（8787）已启动。', 0);
  }

  const text = await res.text();
  let data: unknown = undefined;
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = text;
    }
  }

  if (!res.ok) {
    let msg: string | null = null;
    if (data && typeof data === 'object') {
      const d = data as Record<string, unknown>;
      if (typeof d.message === 'string') msg = d.message;
      else if (typeof d.detail === 'string') msg = d.detail;
      else if (d.detail && typeof d.detail === 'object' && d.detail !== null) {
        const detail = d.detail as Record<string, unknown>;
        if (typeof detail.message === 'string') msg = detail.message;
      }
    }
    if (!msg && typeof data === 'string' && data.trim()) msg = data;
    throw new ApiError(msg || `请求失败（HTTP ${res.status}）`, res.status, data);
  }

  return data as T;
}

function pathOf(v: { path?: string } | string | undefined): string {
  if (!v) return '';
  if (typeof v === 'string') return v;
  return v.path || '';
}

const STAGE_DEFS: Array<{ id: string; label: string }> = [
  { id: 'scan', label: '扫描' },
  { id: 'convert', label: '转换' },
  { id: 'rebuild', label: '顶层重建' },
  { id: 'texture', label: '纹理' },
  { id: 'check', label: '检查' },
];

function textureIsKeep(opts: Record<string, unknown> | undefined): boolean {
  const mode = ((opts?.texture as { mode?: string } | undefined)?.mode || 'keep').toLowerCase();
  return mode === 'keep' || mode === 'none' || mode === '' || mode === 'passthrough';
}

export function deriveStages(api: ApiTask): TaskStageInfo[] {
  const stage = (api.stage || '').toLowerCase();
  const status = (api.status || '').toLowerCase();
  const opts = api.options || {};
  const op = (api.operation || '').toLowerCase();
  const rebuildOpts = opts.rebuildTop as { enabled?: boolean; levels?: number } | undefined;
  const rebuildOn = rebuildOpts?.enabled === true;
  const rebuildLevels = rebuildOpts?.levels === 2 ? 2 : 1;
  const keepTexture = textureIsKeep(opts);
  const skipConvert = op === 'process-tileset' || op === 'rebuild-top';
  const skipRebuild = !rebuildOn && op !== 'rebuild-top';
  // rebuild-top always rebuilds; process-tileset uses rebuildTop.enabled

  const order = STAGE_DEFS.map((d) => d.id);
  const effective = stage === 'done' || stage === 'cancelled' ? 'check' : stage;
  let idx = order.indexOf(effective);
  if (stage === 'done') idx = order.length; // all past check

  function baseSkip(defId: string): TaskStageInfo | null {
    if (defId === 'convert' && skipConvert) {
      return { id: defId, label: '转换', status: 'skipped', message: '本操作无需转换' };
    }
    if (defId === 'rebuild' && skipRebuild && op !== 'rebuild-top') {
      return { id: defId, label: '顶层重建', status: 'skipped', message: '未启用' };
    }
    if (defId === 'texture' && keepTexture) {
      return { id: defId, label: '纹理', status: 'skipped', message: 'keep（跳过）' };
    }
    if (defId === 'texture' && !keepTexture) {
      // Will show deferred message when reached / done
      return null;
    }
    return null;
  }

  return STAGE_DEFS.map((def, i) => {
    const label =
      def.id === 'rebuild' && (rebuildOn || op === 'rebuild-top')
        ? `顶层重建 · L${rebuildLevels}`
        : def.label;

    if (status === 'failed' && stage === def.id) {
      return { id: def.id, label, status: 'failed', message: api.error || '失败' };
    }

    // Terminal success: mark skips + done
    if (status === 'succeeded' || status === 'completed' || stage === 'done') {
      const sk = baseSkip(def.id);
      if (sk) return sk;
      if (def.id === 'texture' && !keepTexture) {
        const prog = typeof api.progress === 'object' && api.progress ? api.progress : {};
        const mode = ((opts?.texture as { mode?: string } | undefined)?.mode || 'ktx2').toLowerCase();
        if (status === 'failed' || api.error) {
          return { id: def.id, label, status: 'failed', message: api.error || 'KTX2 失败' };
        }
        return {
          id: def.id,
          label,
          status: 'done',
          message: typeof prog.textureMode === 'string' ? `KTX2 ${String(prog.textureMode)}` : `KTX2 ${mode}`,
        };
      }
      return { id: def.id, label, status: 'done' };
    }

    // Cancelled / interrupted
    if (status === 'cancelled' || status === 'interrupted') {
      const sk = baseSkip(def.id);
      if (sk && (idx < 0 || i < idx || def.id === 'convert' || def.id === 'rebuild' || (def.id === 'texture' && keepTexture))) {
        // still show skip for convert/rebuild/texture-keep even if not reached
        if (def.id === 'convert' || def.id === 'rebuild' || (def.id === 'texture' && keepTexture && idx >= order.indexOf('texture'))) {
          return sk;
        }
      }
      if (def.id === 'convert' && skipConvert) return { id: def.id, label, status: 'skipped', message: '本操作无需转换' };
      if (def.id === 'rebuild' && skipRebuild) return { id: def.id, label, status: 'skipped', message: '未启用' };
      const cancelIdx = order.indexOf(stage === 'cancelled' ? 'scan' : stage);
      const cidx = cancelIdx >= 0 ? cancelIdx : idx;
      if (cidx >= 0 && i < cidx) {
        const sk2 = baseSkip(def.id);
        return sk2 || { id: def.id, label, status: 'done' };
      }
      if (i === cidx || (stage === 'cancelled' && i === 0 && status === 'cancelled' && !api.startedAt)) {
        return { id: def.id, label, status: 'failed', message: status };
      }
      return { id: def.id, label, status: 'pending' };
    }

    // Queued
    if (status === 'queued' || idx < 0) {
      if (def.id === 'convert' && skipConvert) return { id: def.id, label, status: 'skipped', message: '本操作无需转换' };
      if (def.id === 'rebuild' && skipRebuild) return { id: def.id, label, status: 'skipped', message: '未启用' };
      if (def.id === 'texture' && keepTexture) return { id: def.id, label, status: 'pending', message: '将跳过 (keep)' };
      if (status === 'running' && i === 0) return { id: def.id, label, status: 'running' };
      return { id: def.id, label, status: 'pending' };
    }

    // Active progression
    if (i < idx) {
      const sk = baseSkip(def.id);
      if (sk) return sk;
      return { id: def.id, label, status: 'done' };
    }
    if (i === idx) {
      if (def.id === 'texture' && keepTexture) {
        return { id: def.id, label, status: 'skipped', message: 'keep（跳过）' };
      }
      return {
        id: def.id,
        label,
        status: status === 'running' || status === 'cancelling' ? 'running' : 'pending',
        message: def.id === 'texture' && !keepTexture ? 'KTX2 压缩中…' : undefined,
      };
    }
    // Future stages
    if (def.id === 'convert' && skipConvert) return { id: def.id, label, status: 'skipped', message: '本操作无需转换' };
    if (def.id === 'rebuild' && skipRebuild) return { id: def.id, label, status: 'skipped', message: '未启用' };
    if (def.id === 'texture' && keepTexture) return { id: def.id, label, status: 'pending', message: '将跳过 (keep)' };
    return { id: def.id, label, status: 'pending' };
  });
}

export function normalizeTask(api: ApiTask): Task {
  const progressObj = typeof api.progress === 'object' && api.progress ? api.progress : {};
  const progressNum = typeof api.progress === 'number' ? api.progress : undefined;
  return {
    id: api.id,
    name: api.taskName || api.id,
    taskName: api.taskName,
    operation: api.operation,
    status: (api.status as Task['status']) || 'queued',
    input: pathOf(api.input),
    output: pathOf(api.output),
    options: api.options,
    progress: progressNum,
    stage: api.stage,
    stages: deriveStages(api),
    log: api.log,
    logPath: api.logPath,
    createdAt: api.createdAt,
    updatedAt: api.updatedAt,
    error: api.error || undefined,
    message: api.error || undefined,
    artifactId: typeof progressObj.artifactId === 'string' ? progressObj.artifactId : undefined,
    artifactPath: typeof progressObj.path === 'string' ? progressObj.path : undefined,
  };
}

export const api = {
  baseUrl: API_BASE || (typeof window !== 'undefined' ? window.location.origin : ''),

  health: () => request<HealthResponse>('/api/health'),

  capabilities: () => request<CapabilitiesResponse>('/api/capabilities'),

  scanOsgb: (path: string) =>
    request<OsgbScanResult>('/api/osgb/scan', {
      method: 'POST',
      body: JSON.stringify({ path }),
    }).then((r) => {
      const summary = r.summary || {};
      return {
        ...r,
        hasMetadata: r.hasMetadata ?? Boolean(r.metadata?.path || summary.srs || !(r.errors || []).some((e) => e.includes('metadata'))),
        hasDataDir: r.hasDataDir ?? !(r.errors || []).some((e) => e.includes('Data/')),
        tileCount: r.tileCount ?? summary.tileCount ?? (r.tiles ? r.tiles.length : undefined),
        message: r.message || (r.valid ? '输入目录校验通过。' : (r.errors || []).join('; ')),
      };
    }),

  listTasks: async () => {
    const data = await request<{ ok?: boolean; tasks: ApiTask[] } | ApiTask[]>('/api/tasks');
    const list = Array.isArray(data) ? data : data.tasks || [];
    return list.map(normalizeTask);
  },

  getTask: async (id: string) => {
    const data = await request<{ ok?: boolean; task: ApiTask } | ApiTask>(`/api/tasks/${encodeURIComponent(id)}`);
    const task = 'task' in (data as { task?: ApiTask }) ? (data as { task: ApiTask }).task : (data as ApiTask);
    return normalizeTask(task);
  },

  createTask: async (body: CreateTaskRequest) => {
    const input = typeof body.input === 'string' ? { path: body.input } : body.input;
    const output = typeof body.output === 'string' ? { path: body.output } : body.output;
    const payload = {
      operation: body.operation,
      input,
      output,
      options: body.options,
      taskName: body.taskName || body.name,
    };
    const res = await request<CreateTaskResponse>('/api/tasks', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
    return res;
  },

  cancelTask: (id: string) =>
    request<{ ok?: boolean; task?: ApiTask }>(`/api/tasks/${encodeURIComponent(id)}/cancel`, { method: 'POST' }),

  getTaskLogs: (id: string, tail = 500) =>
    request<{ ok?: boolean; taskId: string; logPath?: string; lines: string[]; log: string }>(
      `/api/tasks/${encodeURIComponent(id)}/logs?tail=${tail}`,
    ),

  listArtifacts: async () => {
    const data = await request<{ ok?: boolean; artifacts: Artifact[] }>('/api/artifacts');
    return data.artifacts || [];
  },

  previewUrl: async (id: string) => {
    const res = await request<PreviewUrlResponse>(
      `/api/artifacts/${encodeURIComponent(id)}/preview-url`,
    );
    const url = res.url || res.previewUrl || '';
    const abs = absolutizeLocalUrl(url);
    return { ...res, url: abs, previewUrl: abs || res.previewUrl };
  },

  prepareOsgb: (path: string) =>
    request<PrepareOsgbResponse>('/api/preview/osgb/prepare', {
      method: 'POST',
      body: JSON.stringify({ path }),
    }),

  nativeOsgbPreview: (path: string) =>
    request<NativePreviewResponse>('/api/preview/osgb/native', {
      method: 'POST',
      body: JSON.stringify({ path }),
    }),
};

export function normalizeTaskList(data: Task[] | { tasks: Task[] } | null | undefined): Task[] {
  if (!data) return [];
  if (Array.isArray(data)) return data;
  if (Array.isArray(data.tasks)) return data.tasks;
  return [];
}

export function friendlyError(err: unknown): string {
  if (err instanceof ApiError) return err.message;
  if (err instanceof Error) return err.message;
  return '发生未知错误，请稍后重试。';
}

export function isActiveStatus(status: string): boolean {
  return status === 'running' || status === 'queued' || status === 'cancelling';
}

export function isDoneStatus(status: string): boolean {
  return status === 'succeeded' || status === 'completed' || status === 'failed' || status === 'cancelled' || status === 'interrupted';
}
