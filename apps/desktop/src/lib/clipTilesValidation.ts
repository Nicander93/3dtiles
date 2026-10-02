export interface ClipForm {
  input: string; output: string; name: string;
  mode: 'rectangle' | 'polygon'; bounds: string[]; geojson: string;
}
export function clippingRegion(form: ClipForm): unknown {
  if (form.mode === 'polygon') {
    const region = JSON.parse(form.geojson);
    const geometry = region?.type === 'Feature' ? region.geometry : region;
    if (geometry?.type !== 'Polygon' || !Array.isArray(geometry.coordinates) || geometry.coordinates.length !== 1) throw new Error('请输入不含孔洞的 GeoJSON Polygon 或 Feature。');
    return region;
  }
  if (form.bounds.length !== 4 || form.bounds.some((v) => !v.trim())) throw new Error('请填写完整的经纬度范围。');
  const [west, south, east, north] = form.bounds.map(Number);
  if (![west, south, east, north].every(Number.isFinite) || west >= east || south >= north) throw new Error('范围应满足西经度 < 东经度，南纬度 < 北纬度。');
  if (Math.abs(west) > 180 || Math.abs(east) > 180 || Math.abs(south) > 80 || Math.abs(north) > 80) throw new Error('经度应在 ±180° 内，首版纬度应在 ±80° 内。');
  return { type: 'rectangle', bounds: [west, south, east, north] };
}
export function clipTaskRequest(form: ClipForm) {
  if (!form.input.trim()) throw new Error('请填写 Tileset 路径。');
  if (!form.output.trim()) throw new Error('请填写成果目录。');
  const normalized = (p: string) => p.trim().replace(/\\/g, '/').replace(/\/+$/, '').replace(/\/[^/]+\.json$/i, '').toLowerCase();
  const input = normalized(form.input); const output = normalized(form.output);
  if (input === output || input.startsWith(`${output}/`) || output.startsWith(`${input}/`)) throw new Error('成果目录不能与输入目录重叠。');
  return { operation: 'clip-tileset', input: { path: form.input.trim() }, output: { path: form.output.trim() }, taskName: form.name.trim() || undefined,
    options: { clip: { region: clippingRegion(form) } } };
}
export function restoreClipRegion(region: unknown): Pick<ClipForm, 'mode' | 'bounds' | 'geojson'> {
  const value = region as { type?: string; bounds?: number[] } | null;
  if (value?.type === 'rectangle' && Array.isArray(value.bounds) && value.bounds.length === 4) return { mode: 'rectangle', bounds: value.bounds.map(String), geojson: '' };
  return { mode: 'polygon', bounds: ['', '', '', ''], geojson: JSON.stringify(region, null, 2) };
}
