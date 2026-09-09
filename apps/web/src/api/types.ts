export type TaskStatus =
  | 'queued'
  | 'running'
  | 'cancelling'
  | 'succeeded'
  | 'completed'
  | 'failed'
  | 'cancelled'
  | 'interrupted';

export type TaskStage =
  | 'queued'
  | 'scan'
  | 'convert'
  | 'rebuild'
  | 'texture'
  | 'check'
  | 'done'
  | 'cancelled'
  | 'failed'
  | 'interrupted';

export interface TaskStageInfo {
  id: TaskStage | string;
  label: string;
  status: 'pending' | 'running' | 'done' | 'failed' | 'skipped';
  message?: string;
}

export interface Task {
  id: string;
  name: string;
  taskName?: string;
  operation: string;
  status: TaskStatus;
  input: string;
  output: string;
  options?: Record<string, unknown>;
  progress?: number | Record<string, unknown>;
  stage?: string;
  stages?: TaskStageInfo[];
  log?: string;
  logPath?: string;
  createdAt?: string | number;
  updatedAt?: string | number;
  message?: string;
  error?: string;
  artifactId?: string;
  artifactPath?: string;
}

export interface ApiTask {
  id: string;
  operation: string;
  status: string;
  taskName?: string;
  input?: { path?: string } | string;
  output?: { path?: string } | string;
  options?: Record<string, unknown>;
  stage?: string;
  progress?: Record<string, unknown> | number;
  log?: string;
  logPath?: string;
  error?: string | null;
  createdAt?: number;
  updatedAt?: number;
  startedAt?: number | null;
  finishedAt?: number | null;
  pid?: number | null;
  cancelRequested?: boolean;
}

export interface HealthResponse {
  ok?: boolean;
  status?: string;
  product?: string;
  version?: string;
  message?: string;
  convertBin?: string;
  texture?: {
    enableTextureCompress?: boolean;
    ktx2Etc1s?: boolean;
    ktx2Uastc?: boolean;
    processTilesetTexture?: boolean;
    postprocessBasisu?: boolean;
    basisuPath?: string;
    notes?: string[];
  };
}

export type TextureMode = 'keep' | 'ktx2' | 'ktx2-etc1s' | 'ktx2-uastc';

export interface TextureModeInfo {
  mode: string;
  supported: boolean;
  cliFlags?: string[];
  postprocess?: boolean;
  reason?: string;
  processTileset?: { mode: string; supported: boolean; reason?: string };
}

export interface CapabilitiesResponse {
  ok?: boolean;
  convert?: Record<string, unknown>;
  textureModes?: TextureModeInfo[];
  aliases?: Record<string, string>;
  postprocessBasisu?: { available?: boolean; path?: string | null };
}

export interface OsgbScanGeo {
  scanSrs?: string | null;
  scanOrigin?: string | null;
  effectiveCrs?: string | null;
  effectiveOrigin?: {
    x?: number | null;
    y?: number | null;
    z?: number | null;
    source?: string;
    text?: string;
  } | null;
  unitHint?: string;
  geographicExport?: boolean;
  hasCrs?: boolean;
  enuLatLon?: { lat: number; lon: number } | null;
  epsg?: number | null;
}

export interface OsgbScanResult {
  ok?: boolean;
  path: string;
  valid: boolean;
  hasMetadata?: boolean;
  hasDataDir?: boolean;
  tileCount?: number;
  message?: string;
  warnings?: string[];
  errors?: string[];
  unitHint?: string;
  geo?: OsgbScanGeo;
  metadata?: {
    srs?: string | null;
    srsOrigin?: string | null;
    path?: string;
  };
  summary?: {
    root?: string;
    tileCount?: number;
    osgbFileCount?: number;
    totalBytes?: number;
    srs?: string | null;
    srsOrigin?: string | null;
  };
  tiles?: Array<{
    name: string;
    entryExists?: boolean;
    osgbCount?: number;
  }>;
}

export interface CreateTaskRequest {
  operation: string;
  input: { path: string } | string;
  output: { path: string } | string;
  options?: Record<string, unknown>;
  taskName?: string;
  name?: string;
}

export interface CreateTaskResponse {
  ok?: boolean;
  task?: ApiTask;
  id?: string;
}

export interface Artifact {
  id: string;
  taskId?: string;
  path: string;
  kind?: string;
  label?: string;
  createdAt?: number;
  created_at?: number;
  available?: boolean;
  has_tileset?: boolean;
  hasTileset?: boolean;
}

export interface PreviewUrlResponse {
  ok: boolean;
  id?: string;
  url: string;
  previewUrl?: string;
  path?: string;
  has_tileset?: boolean;
}

export interface PrepareOsgbResponse {
  ok: boolean;
  cacheDir?: string;
  models: Array<{ name: string; url: string; path?: string; bytes?: number }>;
  errors?: string[];
  scan?: OsgbScanResult;
}

export interface NativePreviewResponse {
  ok: boolean;
  pid?: number;
  display?: string;
  path?: string;
  error?: string;
}
