export type RegionPoint = [number, number];
export type DrawMode = 'rectangle' | 'polygon';

/** Validate in the same WGS84 surface / local ENU frame used by the processor. */
export function drawnClipRegion(mode: DrawMode, points: RegionPoint[]) {
  if ((mode === 'rectangle' && points.length !== 2) || points.length < (mode === 'rectangle' ? 2 : 3) || points.length > 256) throw new Error('请完成区域绘制；多边形需要 3 到 256 个顶点。');
  if (points.some((p) => p.length !== 2 || !p.every(Number.isFinite) || Math.abs(p[0]) > 180 || Math.abs(p[1]) > 80)) throw new Error('区域经度须在 ±180°、纬度须在 ±80° 内。');
  const ring = mode === 'rectangle' ? rectangleRing(points) : points;
  const lon = ring.reduce((sum, p) => sum + p[0], 0) / ring.length;
  const lat = ring.reduce((sum, p) => sum + p[1], 0) / ring.length;
  if (ring.some((p) => Math.abs(p[0] - lon) > 1)) throw new Error('区域跨度过大或跨日期变更线。');
  const radians = Math.PI / 180;
  const sl = Math.sin(lon * radians), cl = Math.cos(lon * radians), s = Math.sin(lat * radians), c = Math.cos(lat * radians);
  const surface = ([x, y]: RegionPoint) => {
    const sy = Math.sin(y * radians), cy = Math.cos(y * radians);
    const n = 6378137 / Math.sqrt(1 - 6.6943799901413165e-3 * sy * sy);
    return [n * cy * Math.cos(x * radians), n * cy * Math.sin(x * radians), n * (1 - 6.6943799901413165e-3) * sy];
  };
  const origin = surface([lon, lat]);
  const planar = ring.map((p) => {
    const delta = surface(p).map((v, i) => v - origin[i]);
    return [-sl * delta[0] + cl * delta[1], -s * cl * delta[0] - s * sl * delta[1] + c * delta[2]];
  });
  if (planar.some(([x, y]) => Math.hypot(x, y) > 10000)) throw new Error('区域距中心不能超过 10 公里。');
  const area = planar.reduce((sum, p, i) => {
    const next = planar[(i + 1) % planar.length]; return sum + p[0] * next[1] - next[0] * p[1];
  }, 0);
  if (Math.abs(area) < 1e-4) throw new Error('区域面积为零，请重新绘制。');
  const cross = (a: number[], b: number[], p: number[]) => (b[0]-a[0])*(p[1]-a[1])-(b[1]-a[1])*(p[0]-a[0]);
  const onSegment = (a: number[], b: number[], p: number[]) => Math.abs(cross(a,b,p)) <= 1e-6 && [0,1].every((i) => p[i] >= Math.min(a[i],b[i])-1e-6 && p[i] <= Math.max(a[i],b[i])+1e-6);
  if (planar.some((a,i) => planar.slice(i+1).some((b) => Math.hypot(b[0]-a[0],b[1]-a[1]) < 1e-6))) throw new Error('区域存在重复顶点。');
  for (let i = 0; i < planar.length; i++) {
    const a = planar[i], b = planar[(i + 1) % planar.length];
    if (Math.hypot(b[0] - a[0], b[1] - a[1]) < 1e-6) throw new Error('区域存在重复顶点。');
    for (let j=i+1; j<planar.length; j++) {
      const c=planar[j], d=planar[(j+1)%planar.length];
      if (Math.hypot(c[0]-a[0],c[1]-a[1]) < 1e-6) throw new Error('区域存在重复顶点。');
      if (j === i+1 || (i === 0 && j === planar.length-1)) continue;
      if ((cross(a,b,c)*cross(a,b,d) < 0 && cross(c,d,a)*cross(c,d,b) < 0) || onSegment(a,b,c) || onSegment(a,b,d) || onSegment(c,d,a) || onSegment(c,d,b)) throw new Error('多边形不能自交或自接触，请调整顶点。');
    }
  }
  return mode === 'rectangle'
    ? { type: 'rectangle', bounds: [ring[0][0], ring[0][1], ring[2][0], ring[2][1]] }
    : { type: 'Polygon', coordinates: [[...points, points[0]]] };
}

function rectangleRing(points: RegionPoint[]): RegionPoint[] {
  const [a, b] = points;
  const west = Math.min(a[0], b[0]), east = Math.max(a[0], b[0]);
  const south = Math.min(a[1], b[1]), north = Math.max(a[1], b[1]);
  return [[west, south], [east, south], [east, north], [west, north]];
}

export function readDrawingMessage(value: unknown): { mode: DrawMode; points: RegionPoint[]; heights?: number[]; complete: boolean; active: boolean } | null {
  if (!value || typeof value !== 'object') return null;
  const data = value as Record<string, unknown>;
  if (data.type !== 'geoforge-region' || !['rectangle', 'polygon'].includes(String(data.mode)) || typeof data.complete !== 'boolean' || typeof data.active !== 'boolean' || !Array.isArray(data.points) || data.points.length > 256) return null;
  if (!data.points.every((p) => Array.isArray(p) && p.length === 2 && p.every((n) => typeof n === 'number' && Number.isFinite(n)))) return null;
  if (data.heights !== undefined && (!Array.isArray(data.heights) || data.heights.length !== data.points.length || !data.heights.every((h) => typeof h === 'number' && Number.isFinite(h)))) return null;
  return { mode: data.mode as DrawMode, points: data.points as RegionPoint[], heights: data.heights as number[] | undefined, complete: data.complete, active: data.active };
}

export function readFlattenMessage(value: unknown): { regionKey: string; heightMeters: number; initialHeightMeters: number; hasUndo: boolean } | null {
  if (!value || typeof value !== 'object') return null;
  const data = value as Record<string, unknown>;
  if (data.type !== 'geoforge-flatten-plane' || typeof data.regionKey !== 'string' || typeof data.hasUndo !== 'boolean' ||
    ![data.heightMeters,data.initialHeightMeters].every((h) => typeof h === 'number' && Number.isFinite(h) && Math.abs(h) <= 10000)) return null;
  return data as unknown as { regionKey: string; heightMeters: number; initialHeightMeters: number; hasUndo: boolean };
}
