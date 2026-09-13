import type { CapabilitiesResponse } from '../api/types';

export function convertReady(caps: CapabilitiesResponse | null): {
  ok: boolean;
  via: 'bin' | 'docker' | 'unknown' | 'none';
  message: string;
} {
  const c = caps?.convert;
  if (!c) {
    return { ok: true, via: 'unknown', message: '' };
  }
  if (c.exists) {
    return { ok: true, via: 'bin', message: '本机已找到 _3dtile' };
  }
  if (c.docker) {
    const image = c.image || 'winner1/3dtiles:1.0';
    return {
      ok: true,
      via: 'docker',
      message: `本机没有 _3dtile，转换会走 Docker（${image}）。第一次会拉镜像，请保持 Docker Desktop 已启动。`,
    };
  }
  return {
    ok: false,
    via: 'none',
    message:
      '找不到 _3dtile，也没有可用的 Docker。请先安装并启动 Docker Desktop，或设置环境变量 GEOFORGE_3DTILE 指向转换器。',
  };
}
