/**
 * Desktop API adapter (Phase 2).
 * Pages must import from here — do not call invoke() directly.
 * Prefer Tauri commands when window.__TAURI__ / __TAURI_INTERNALS__ is present
 * (task submit/list/preview/scan/health — no Python HTTP on the happy path);
 * otherwise fall back to Python desktop_server HTTP (browser-only mode).
 */
import { invoke } from '@tauri-apps/api/core';
import {
  ApiError,
  absolutizeLocalUrl,
  friendlyError,
  isActiveStatus,
  isDoneStatus,
  normalizeTask,
  normalizeTaskList,
  api as httpApi,
} from './client';
import type {
  ApiTask,
  Artifact,
  CapabilitiesResponse,
  CreateTaskRequest,
  CreateTaskResponse,
  HealthResponse,
  OsgbScanResult,
  ModelScanResult,
  PreviewUrlResponse,
  Task,
} from './types';

export { ApiError, friendlyError, isActiveStatus, isDoneStatus, normalizeTask, normalizeTaskList, absolutizeLocalUrl };

export function isTauri(): boolean {
  return (
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
  );
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(cmd, args);
}

export type ResourceMode = 'auto' | 'custom';

export interface ExecutionSettings {
  resourceMode: ResourceMode;
  cpuWorkers?: number;
  memoryBudgetMiB?: number;
  ioWorkers?: number;
}

export interface DesktopSettings {
  defaultOutputRoot: string;
  defaultRebuildTop: boolean;
  defaultRebuildLevels: number;
  defaultTextureCompress: boolean;
  defaultConvertThreads?: number;
  pythonServerUrl: string;
  resourceServerPort: number;
  execution?: ExecutionSettings;
  settingsVersion?: number;
}

const SETTINGS_VERSION = 2;

const settingsDefaults: DesktopSettings = {
  defaultOutputRoot: '',
  defaultRebuildTop: true,
  defaultRebuildLevels: 0,
  defaultTextureCompress: false,
  defaultConvertThreads: 0,
  pythonServerUrl: 'http://127.0.0.1:8787',
  resourceServerPort: 0,
  execution: {
    resourceMode: 'auto',
  },
};

// Before version 2 the UI defaulted to one convert worker, so a saved 1 cannot be told apart
// from an untouched default. Move it to the automatic default; later saves keep 1 as chosen.
function migrateSettings(saved: Partial<DesktopSettings>): DesktopSettings {
  const settings = { ...settingsDefaults, ...saved, settingsVersion: SETTINGS_VERSION };
  if ((saved.settingsVersion ?? 0) < SETTINGS_VERSION && saved.defaultConvertThreads === 1) {
    settings.defaultConvertThreads = settingsDefaults.defaultConvertThreads;
  }
  return settings;
}

export const desktop = {
  isTauri,

  selectInputDirectory: async (): Promise<string | null> => {
    if (!isTauri()) return null;
    return tauriInvoke<string | null>('select_input_directory');
  },

  selectOutputDirectory: async (): Promise<string | null> => {
    if (!isTauri()) return null;
    return tauriInvoke<string | null>('select_output_directory');
  },

  selectTilesetFile: async (): Promise<string | null> => {
    if (!isTauri()) return null;
    return tauriInvoke<string | null>('select_tileset_file');
  },

  /** Health / capabilities / scan: Tauri local commands when available (Phase 3). */
  health: async (): Promise<HealthResponse> => {
    if (isTauri()) {
      return tauriInvoke<HealthResponse>('health');
    }
    return httpApi.health();
  },

  capabilities: async (): Promise<CapabilitiesResponse> => {
    if (isTauri()) {
      return tauriInvoke<CapabilitiesResponse>('capabilities');
    }
    return httpApi.capabilities();
  },

  scanOsgb: async (path: string): Promise<OsgbScanResult> => {
    if (isTauri()) {
      return tauriInvoke<OsgbScanResult>('scan_osgb', { path });
    }
    return httpApi.scanOsgb(path);
  },

  selectModelFile: async (): Promise<string | null> => {
    if (!isTauri()) return null;
    return tauriInvoke<string | null>('select_model_file');
  },

  selectTextureRoot: async (): Promise<string | null> => {
    if (!isTauri()) return null;
    return tauriInvoke<string | null>('select_texture_root');
  },

  scanModel: async (path: string, textureRoots: string[] = []): Promise<ModelScanResult> => {
    if (!isTauri()) {
      throw new ApiError('model preflight requires Tauri desktop', 0);
    }
    return tauriInvoke<ModelScanResult>('scan_model', { path, textureRoots });
  },

  listTasks: async (): Promise<Task[]> => {
    if (isTauri()) {
      const data = await tauriInvoke<{ ok?: boolean; tasks: ApiTask[] }>('list_tasks', {
        filter: null,
      });
      return (data.tasks || []).map(normalizeTask);
    }
    return httpApi.listTasks();
  },

  getTask: async (id: string): Promise<Task> => {
    if (isTauri()) {
      const data = await tauriInvoke<{ ok?: boolean; task: ApiTask }>('get_task', { taskId: id });
      return normalizeTask(data.task);
    }
    return httpApi.getTask(id);
  },

  createTask: async (body: CreateTaskRequest): Promise<CreateTaskResponse> => {
    if (isTauri()) {
      const input = typeof body.input === 'string' ? body.input : body.input.path;
      const output = typeof body.output === 'string' ? body.output : body.output.path;
      return tauriInvoke<CreateTaskResponse>('submit_task', {
        config: {
          operation: body.operation,
          input,
          output,
          options: body.options ?? {},
          taskName: body.taskName || body.name,
        },
      });
    }
    return httpApi.createTask(body);
  },

  cancelTask: async (id: string) => {
    if (isTauri()) {
      return tauriInvoke<{ ok?: boolean; task?: ApiTask }>('cancel_task', { taskId: id });
    }
    return httpApi.cancelTask(id);
  },

  getTaskLogs: async (id: string, tail = 500) => {
    if (isTauri()) {
      return tauriInvoke<{
        ok?: boolean;
        taskId: string;
        logPath?: string;
        lines: string[];
        log: string;
      }>('get_task_logs', { taskId: id, tail });
    }
    return httpApi.getTaskLogs(id, tail);
  },

  listArtifacts: async (): Promise<Artifact[]> => {
    if (isTauri()) {
      const data = await tauriInvoke<{ ok?: boolean; artifacts: Artifact[] }>('list_artifacts');
      return data.artifacts || [];
    }
    return httpApi.listArtifacts();
  },

  registerArtifact: async (path: string, opts?: { taskId?: string; label?: string; kind?: string }) => {
    if (!isTauri()) {
      throw new ApiError('registerArtifact requires Tauri desktop', 0);
    }
    return tauriInvoke<{ ok: boolean; artifact: Artifact }>('register_artifact', {
      args: {
        path,
        taskId: opts?.taskId,
        label: opts?.label,
        kind: opts?.kind,
      },
    });
  },

  previewUrl: async (id: string): Promise<PreviewUrlResponse> => {
    if (isTauri()) {
      const res = await tauriInvoke<PreviewUrlResponse>('get_preview_url', { artifactId: id });
      // Already absolute http://127.0.0.1:<rust-port>/...
      return res;
    }
    return httpApi.previewUrl(id);
  },

  openArtifactDirectory: async (artifactId: string) => {
    if (!isTauri()) {
      throw new ApiError('openArtifactDirectory requires Tauri desktop', 0);
    }
    return tauriInvoke<{ ok: boolean; path: string }>('open_artifact_directory', { artifactId });
  },

  getSettings: async (): Promise<DesktopSettings> => {
    if (isTauri()) {
      const s = await tauriInvoke<DesktopSettings>('get_settings');
      return migrateSettings(s);
    }
    try {
      const raw = localStorage.getItem('geoforge.settings');
      if (!raw) return { ...settingsDefaults };
      return migrateSettings(JSON.parse(raw));
    } catch {
      return { ...settingsDefaults };
    }
  },

  updateSettings: async (settings: DesktopSettings): Promise<DesktopSettings> => {
    if (isTauri()) {
      return tauriInvoke<DesktopSettings>('update_settings', { settings });
    }
    const saved = { ...settings, settingsVersion: SETTINGS_VERSION };
    localStorage.setItem('geoforge.settings', JSON.stringify(saved));
    return saved;
  },

  getResourceServerInfo: async () => {
    if (!isTauri()) return null;
    return tauriInvoke<{ ok: boolean; port: number; baseUrl: string; dataDir: string }>(
      'get_resource_server_info',
    );
  },

  /** Absolute API base for Settings display. */
  get baseUrl(): string {
    if (isTauri()) return '(Tauri commands + Rust artifact server)';
    return httpApi.baseUrl;
  },
};

/** Drop-in replacement for legacy `api` import. */
export const api = desktop;
