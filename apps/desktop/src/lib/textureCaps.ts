import type { CapabilitiesResponse } from '../api/types';

/** KTX2 ETC1S selectable when basisu post-process or native mode is available. */
export function ktx2Etc1sEnabled(caps: CapabilitiesResponse | null, forProcessTileset = false): boolean {
  if (!caps) return true;
  if (caps.postprocessBasisu?.available) return true;
  const mode = caps.textureModes?.find((m) => m.mode === 'ktx2-etc1s');
  if (forProcessTileset) return mode?.processTileset?.supported !== false;
  return mode?.supported !== false;
}

export function ktx2UastcEnabled(caps: CapabilitiesResponse | null, forProcessTileset = false): boolean {
  if (!caps) return true;
  if (caps.postprocessBasisu?.available) return true;
  const mode = caps.textureModes?.find((m) => m.mode === 'ktx2-uastc');
  if (forProcessTileset) return mode?.processTileset?.supported !== false;
  return mode?.supported !== false;
}
