export interface ModelConvertValidationInput {
  input: string;
  output: string;
  format: 'fbx' | 'obj';
  unit: string;
  axes: string;
  georeferenceMode: 'local' | 'anchor' | 'projected';
  longitude: string;
  latitude: string;
  height: string;
  sourceCrs: string;
  originX: string;
  originY: string;
  originZ: string;
  projectedGeoreferenceSupported: boolean;
}

function samePath(left: string, right: string): boolean {
  const normalize = (path: string) => path.trim().replace(/[/\\]+$/, '').replace(/\\/g, '/').toLowerCase();
  return Boolean(left.trim()) && normalize(left) === normalize(right);
}

function finiteNumber(value: string): number | null {
  if (!value.trim()) return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function modelConvertValidationError(input: ModelConvertValidationInput): string | null {
  if (!input.input.trim() || !input.output.trim()) return '请填写模型文件和输出目录。';
  if (samePath(input.input, input.output)) return '输出目录不能与模型文件相同。';
  if (input.format === 'obj' && (input.unit === 'fromMetadata' || input.axes === 'fromMetadata')) {
    return 'OBJ 不可靠地声明单位和轴向，请明确选择。';
  }

  if (input.georeferenceMode === 'anchor') {
    const longitude = finiteNumber(input.longitude);
    const latitude = finiteNumber(input.latitude);
    const height = finiteNumber(input.height);
    if (longitude === null || latitude === null || height === null) {
      return '锚点定位需要有效的经度、纬度和椭球高数值。';
    }
    if (longitude < -180 || longitude > 180 || latitude < -90 || latitude > 90) {
      return '锚点经度范围为 -180 至 180，纬度范围为 -90 至 90。';
    }
  }

  if (input.georeferenceMode === 'projected') {
    if (!input.sourceCrs.trim()) return '已有投影坐标模式需要完整的源 CRS（例如 EPSG:4547 或 WKT2）。';
    if (!input.projectedGeoreferenceSupported) return '当前转换器不支持已有投影坐标模式，请更新运行组件。';
    if ([input.originX, input.originY, input.originZ].some((value) => finiteNumber(value) === null)) {
      return '投影坐标原点偏移必须是有限数值。';
    }
  }

  return null;
}
