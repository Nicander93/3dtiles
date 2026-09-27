export type ModelInputFormat = 'fbx' | 'obj';

export interface ModelScanGateInput {
  scannedPath: string | null;
  currentPath: string;
  debouncedPath: string;
  scannedTextureRoots: string[];
  textureRoots: string[];
  scanPending: boolean;
  scanValid: boolean;
  scannedFormat?: string;
  selectedFormat: ModelInputFormat;
}

function samePath(left: string, right: string): boolean {
  const normalize = (path: string) => path.trim().replace(/[/\\]+$/, '').replace(/\\/g, '/').toLowerCase();
  return Boolean(left.trim()) && normalize(left) === normalize(right);
}

function samePathList(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((path, index) => samePath(path, right[index]));
}

export function modelScanMatchesCurrentInput(input: ModelScanGateInput): boolean {
  return Boolean(
    input.scannedPath
      && samePath(input.scannedPath, input.currentPath)
      && samePath(input.debouncedPath, input.currentPath)
      && samePathList(input.scannedTextureRoots, input.textureRoots),
  );
}

export function canSubmitModelScan(input: ModelScanGateInput): boolean {
  return modelScanMatchesCurrentInput(input)
    && !input.scanPending
    && input.scanValid
    && (!input.scannedFormat || input.scannedFormat === input.selectedFormat);
}
