import type { Icon } from '@phosphor-icons/react';
import {
  Cube,
  Eye,
  Image,
  Stack,
} from '@phosphor-icons/react';

export type ToolId = 'osgb-convert' | 'tiles-preview' | 'tiles-rebuild' | 'tiles-texture';

export type ToolDef = {
  id: ToolId;
  title: string;
  desc: string;
  to: string;
  icon: Icon;
};

export type ToolGroup = {
  id: string;
  title: string;
  tools: ToolDef[];
};

export const toolGroups: ToolGroup[] = [
  {
    id: 'oblique',
    title: '倾斜摄影',
    tools: [
      {
        id: 'osgb-convert',
        title: 'OSGB 转换',
        desc: '将 OSGB 数据转换为 3D Tiles',
        to: '/osgb/convert',
        icon: Stack,
      },
    ],
  },
  {
    id: 'tiles',
    title: '3D Tiles',
    tools: [
      {
        id: 'tiles-preview',
        title: '预览',
        desc: '浏览和检查 3D Tiles 数据',
        to: '/preview/tiles',
        icon: Eye,
      },
      {
        id: 'tiles-rebuild',
        title: '顶层重建',
        desc: '重建 3D Tiles 的顶层结构',
        to: '/tiles/process?op=rebuild',
        icon: Cube,
      },
      {
        id: 'tiles-texture',
        title: '纹理压缩',
        desc: '压缩 3D Tiles 纹理以减小体积',
        to: '/tiles/process?op=texture',
        icon: Image,
      },
    ],
  },
];

export function filterToolGroups(query: string): ToolGroup[] {
  const q = query.trim().toLowerCase();
  if (!q) return toolGroups;
  return toolGroups
    .map((g) => ({
      ...g,
      tools: g.tools.filter(
        (t) =>
          t.title.toLowerCase().includes(q) ||
          t.desc.toLowerCase().includes(q),
      ),
    }))
    .filter((g) => g.tools.length > 0);
}
