# 独立预览示例

这些页面用于手工查看模型与转换结果。桌面应用的正式预览页面在 `apps/desktop/public/cesium-preview.html`。

| 页面 | 用途 |
| --- | --- |
| `preview/index.html` | 原有转换结果预览，配合 `scripts/convert_sample.sh` 使用 |
| `preview/osgb.html` | 原有 OSGB 转 GLB 示例，配合 `scripts/prepare_osgb_preview.sh` 使用 |
| `preview/rebuild.html` | 重建结果预览示例 |
| `preview/indoor.html` | 原根目录 `index.html`，默认读取根目录 `output/tileset.json` |

从仓库根目录启动静态服务后，打开对应页面：

```bash
python -m http.server 8080 --directory .
# http://localhost:8080/examples/preview/indoor.html
```

示例仍可能依赖在线 Cesium 资源和手工准备的数据。运行方式还可参考 [旧示例说明](../docs/RUN.md)。
