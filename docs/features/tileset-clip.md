# 3D Tiles 范围裁剪

`clip-tileset` 导出区域内的真实几何。第一版支持已有 WGS84/ECEF 地理定位的显式静态三角网格，包含 GLB 与无批次 B3DM。桌面入口为工作区 → **范围裁剪**；用户选择输入、尚不存在的成果目录，以及经纬度矩形或多边形 GeoJSON。

## 区域定义与限制

矩形使用度单位的 `[west,south,east,north]`。GeoJSON 接受 Polygon 或包含 Polygon 的 Feature，要求一个闭合、简单的环，3–256 个不同顶点，无孔洞。顺逆时针都支持。区域顶点距平均经纬度中心不得超过 10 公里，纬度限 ±80°，不支持跨日期变更线。

区域边界定义为：把 WGS84 顶点投影到中心附近的局部 ENU，连接为直线，再沿 ENU up 方向无限延伸形成竖直半平面。这是小范围的平面裁剪语义；边界不是长距离地表曲线，也不限制高度。f64 用于坐标链和交点判断；保留网格局部坐标后写回 f32。写出后的顶点若超过边界 2 厘米容差，任务明确失败，要求先把模型顶点局部化。2 厘米检查仅针对数值写出误差，不是地表投影精度承诺。

## 处理与保留

- 遍历每个 LOD、子节点和外部 Tileset，累计 transform；内容依次应用 glTF 节点变换、Y-up → Z-up、B3DM RTC_CENTER、累计 tile transform。
- 从解码 POSITION 计算保守 ENU 包围盒，排除完全在区域外的 primitive；完全包含的 primitive 跳过边界求交；相交三角形逐半平面裁剪并三角化。第一版仍重写被保留的 primitive，不承诺内侧内容字节原样复制。
- 保留 POSITION、NORMAL、TANGENT、TEXCOORD_0/1、COLOR_0；插值 UV/颜色，归一化法线，并对切线做正交化。不同材质 primitive 保持分离；共享 mesh 的不同节点按各自变换分别裁剪。
- 材质、sampler 和贴图编码保留，内嵌图片重定位到新 BIN，外部图片复制为相对引用。几何 accessor/bufferView 重新生成，输出采用无索引三角形；不压缩、简化或重新打包贴图，不生成封口。
- 保留原树的有效分支与 refine，移除空内容和空分支，按实际写出的 f32 顶点重算 box；父盒包含子树。原 geometricError 不降低，必要时提高父级误差以保持包含子级误差的保守关系。此策略不重新估计裁剪后的 HLOD 误差。
- 输出不复制未引用的原始模型或旧几何缓冲。资源命名独立；成果可脱离输入目录移动使用。`clip-report.json` 记录原区域、处理/移除内容数、前后三角形数和未封口策略。
- 入口与外部 Tileset 均使用 asset.version 1.1，使直接 GLB 引用符合核心格式；原 B3DM 内容类型仍保留。

## 明确不支持

隐式切片、多 contents、结构化元数据与 Tileset 扩展（包括旧版元数据扩展）；点云、I3DM、CMPT；动画、蒙皮、morph、GPU 实例化、Draco/meshopt/量化、feature ID 与自定义顶点属性。B3DM 须使用现代 28 字节头，BATCH_LENGTH=0，允许 JSON RTC_CENTER，不支持 feature/batch binary table 或 batch metadata。

GLB 须为 glTF 2、一个内嵌 BIN、一个 scene；节点必须均可达且不构成共享节点 DAG/循环。普通节点复用 mesh 支持。接受 KHR_materials_unlit、KHR_texture_transform，其他 glTF 扩展明确失败。支持 float 属性及归一化颜色/UV，常规整数索引和 interleaved byteStride，不支持 sparse accessor。

输入资源须使用输入根目录内的普通相对 URI；远程、绝对、查询、fragment、百分号编码不支持。每个读取文件及输出 BIN 上限 128 MiB，GLB JSON 上限 16 MiB，每个 accessor 和每个生成 primitive 上限 100 万顶点，递归深度上限 128。大数据集可由许多小内容组成，但第一版不是无限内存的流式网格内核。

输入原样保留；预检在创建输出父目录前检查重叠。沿用取消、临时目录归属及 no-replace 提交。全部裁空会报告 `clip removed all geometry`，不提交成果；任何失败或取消均清理本任务临时目录。

## CLI 和任务协议

```powershell
rtk proxy cargo run -p processor -- clip-tileset -i D:/data/tiles -o D:/results/cropped --region D:/data/region.json
```

```json
{
  "schemaVersion":1,
  "taskId":"clip-example",
  "operation":"clip-tileset",
  "input":{"path":"D:/data/tiles"},
  "output":{"path":"D:/results/cropped"},
  "options":{"clip":{"region":{"type":"rectangle","bounds":[0,0,0.0003,0.0003]}}}
}
```

## 测试和人工验收

算法单测验证面积、交点属性插值、完全内外和非法区域。八项真实 CLI 测试覆盖边界顶点与面积、UV/法线/材质、归一化颜色与索引/stride、多级 LOD、外部树、RTC 与节点变换、共享 mesh、外部图片、取消、循环/越界/缺资源及输入保护。

三项裁剪 Playwright 测试使用真实 processor，验证工作区入口、输出 GLB 边界、报告、任务阶段、重新处理区域恢复、GeoJSON 文件导入与持久化，以及已有输出和空结果。传输层替换为浏览器测试适配器，原生 Tauri 对话框、WebView 渲染和安装包仍需人工验收。Product Core CI 运行所有合并与裁剪 e2e。

在 `apps/desktop` 执行 `rtk proxy node scripts/create-clip-fixtures.mjs`，生成带定位、可渲染的合成三角形及区域文件。在本分支开发桌面中先预览原模型，再执行裁剪并预览成果，应看到模型缩小为区域内的部分。再把成果移动后预览，导入 region.geojson 重做，确认重试恢复区域；指定已有目录、无交集区域和复制/裁剪时取消，确认输入不变。当前发布安装包不包含此功能。合成样例用于流程与几何检查，不替代真实城市数据性能验证。

在 `apps/desktop` 执行 `rtk proxy npm run tauri:dev` 启动开发桌面；这个入口会编译并指定本分支的 processor，避免误用旧安装包中的处理器。

标准依据：[3D Tiles 坐标与层级](https://github.com/CesiumGS/3d-tiles/blob/main/specification/README.adoc)、[B3DM RTC](https://github.com/CesiumGS/3d-tiles/blob/main/specification/TileFormats/Batched3DModel/README.adoc)、[glTF 属性和变换](https://github.com/KhronosGroup/glTF/blob/main/specification/2.0/Specification.adoc)。
