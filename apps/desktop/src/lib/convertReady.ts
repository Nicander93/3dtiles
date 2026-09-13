import type { CapabilitiesResponse } from '../api/types';

export function convertReady(caps: CapabilitiesResponse | null): {
  ok: boolean;
  via: 'bin' | 'docker' | 'unknown' | 'none';
  message: string;
} {
  const c = caps?.convert as
    | {
        exists?: boolean;
        docker?: boolean;
        image?: string;
        ready?: boolean;
        native?: boolean;
        dockerFallback?: boolean;
        message?: string;
      }
    | undefined;
  if (!c) {
    return { ok: true, via: 'unknown', message: '' };
  }
  if (c.native || c.exists) {
    return { ok: true, via: 'bin', message: c.message || '本机已找到转换器' };
  }
  if (c.ready && (c.docker || c.dockerFallback)) {
    return {
      ok: true,
      via: 'docker',
      message: c.message || '开发环境可使用 Docker 回退（正式安装包不提供）',
    };
  }
  if (c.docker) {
    const image = c.image || 'winner1/3dtiles:1.0';
    return {
      ok: true,
      via: 'docker',
      message: `本机没有 _3dtile，转换会走 Docker（${image}）。`,
    };
  }
  return {
    ok: false,
    via: 'none',
    message: c.message || '组件缺失，请修复安装（转换器）',
  };
}
