import type { CapabilitiesResponse, TextureMode } from '../api/types';

/** KTX2 ETC1S selectable when basisu post-process or native mode is available. */
export function ktx2Etc1sEnabled(caps: CapabilitiesResponse | null, forProcessTileset = false): boolean {
  if (!caps) return false;
  if (caps.postprocessBasisu?.available) return true;
  const mode = caps.textureModes?.find((m) => m.mode === 'ktx2-etc1s');
  if (!mode) return false;
  if (forProcessTileset && mode.processTileset) return mode.processTileset.supported !== false;
  return mode.supported === true;
}

export function ktx2UastcEnabled(caps: CapabilitiesResponse | null, forProcessTileset = false): boolean {
  if (!caps) return false;
  if (caps.postprocessBasisu?.available) return true;
  const mode = caps.textureModes?.find((m) => m.mode === 'ktx2-uastc');
  if (!mode) return false;
  if (forProcessTileset && mode.processTileset) return mode.processTileset.supported !== false;
  return mode.supported === true;
}

export function textureModeEnabled(
  caps: CapabilitiesResponse | null,
  mode: TextureMode,
  forProcessTileset = false,
): boolean {
  if (mode === 'keep') return true;
  if (mode === 'ktx2-uastc') return ktx2UastcEnabled(caps, forProcessTileset);
  return ktx2Etc1sEnabled(caps, forProcessTileset);
}
