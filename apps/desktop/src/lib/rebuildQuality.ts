import type { TextureMode } from '../api/types';

export type RebuildQuality = 'quality' | 'balanced' | 'speed';

export interface RebuildQualityOptions {
  levels: number;
  textureMode: TextureMode;
  l1MaxTriangles: number;
  l2MaxTriangles: number;
}

export function rebuildQualityOptions(
  quality: RebuildQuality,
  ktx2Available: boolean,
): RebuildQualityOptions {
  if (quality === 'quality') {
    return {
      levels: 0,
      textureMode: 'keep',
      l1MaxTriangles: 8000,
      l2MaxTriangles: 4000,
    };
  }
  if (quality === 'speed') {
    return {
      levels: 0,
      textureMode: ktx2Available ? 'ktx2-etc1s' : 'keep',
      l1MaxTriangles: 2000,
      l2MaxTriangles: 1000,
    };
  }
  return {
    levels: 0,
    textureMode: 'keep',
    l1MaxTriangles: 4000,
    l2MaxTriangles: 2000,
  };
}

export function inferRebuildQuality(
  levels: number,
  textureMode: TextureMode,
): RebuildQuality {
  if (textureMode !== 'keep') return 'speed';
  if (levels === 2) return 'quality';
  return 'balanced';
}

export function rebuildLevelsLabel(levels: number): string {
  return levels > 0 ? `顶层重建 ×${levels}` : '顶层重建到根';
}
