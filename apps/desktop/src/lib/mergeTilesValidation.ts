export const MAX_MERGE_INPUTS = 64;

export interface MergeForm {
  inputs: string[];
  output: string;
  name: string;
}

function normalizedPath(path: string): string {
  // The desktop runs on Windows. Backend canonicalization is authoritative.
  return path.trim().replace(/\\/g, '/').replace(/\/+$/, '').replace(/\/tileset\.json$/i, '').toLowerCase();
}

export function validateMergeForm(form: MergeForm): string | null {
  if (form.inputs.length < 2 || form.inputs.length > MAX_MERGE_INPUTS) return '请选择 2 到 64 份 Tileset。';
  if (form.inputs.some((path) => !path.trim())) return '请填写每份 Tileset 的路径。';
  const paths = form.inputs.map(normalizedPath);
  if (new Set(paths).size !== paths.length) return '输入列表包含重复的 Tileset。';
  if (!form.output.trim()) return '请填写成果目录。';
  const output = normalizedPath(form.output);
  if (paths.some((input) => output === input || output.startsWith(`${input}/`) || input.startsWith(`${output}/`))) {
    return '成果目录不能与任何输入目录重叠。';
  }
  return null;
}

export function mergeTaskRequest(form: MergeForm) {
  return {
    operation: 'merge-tilesets',
    input: { path: form.inputs[0].trim() },
    output: { path: form.output.trim() },
    taskName: form.name.trim() || undefined,
    options: { merge: { additionalInputs: form.inputs.slice(1).map((path) => path.trim()) } },
  };
}
