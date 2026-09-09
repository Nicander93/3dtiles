# 可运行初版 / Runnable v0

## 一键：下载公开 OSGB → 转 3D Tiles → 预览

```bash
./scripts/convert_sample.sh
python3 -m http.server 8080 --directory examples/preview
# 浏览器打开 http://127.0.0.1:8080/
```

公开样例来源：GitHub 用户上传附件（fanvanzh/3dtiles issue #336 讨论用的 OSGBny.zip）。

## 运行时从哪来

本机无 `scripts/setup_runtime.sh` 从 Docker Hub 镜像 `winner1/3dtiles:1.0` **导出** `_3dtile` 与依赖库（不需要本机 Docker daemon，使用 `crane`）。

也可用已安装的 Docker：

```bash
docker run --rm -v "$PWD/samples:/data" winner1/3dtiles:1.0 \
  /bin/bash -c 'cd /3dtiles && export LD_LIBRARY_PATH=./lib && ./target/release/_3dtile -f osgb -i /data/OSGBny -o /data/OSGBny_3dtiles'
```

## OSGB 预览

完整倾斜摄影 OSGB 浏览需要原生 OpenSceneGraph 查看器（见 `docs/UI_PLAN.md`）。  
初版先保证 **转换结果** 用 Cesium 网页预览；源数据目录结构可用文件浏览器查看 `samples/OSGBny/Data`。

## 校验

```bash
test -f samples/OSGBny_3dtiles/tileset.json
find samples/OSGBny_3dtiles -name '*.b3dm' | head
```

## OSGB 三维预览（公开倾斜样例）

```bash
./scripts/prepare_osgb_preview.sh
python3 -m http.server 8080 --directory examples/preview
# 打开 http://127.0.0.1:8080/osgb.html
```

将各 `Tile_*/Tile_*.osgb` 转为 GLB 后用 Three.js 浏览（公开样例 OSGBny）。

## 顶层重建 rebuild-top

```bash
./scripts/convert_sample.sh
./scripts/rebuild_sample.sh
# 预览合并结果：把 examples/preview/index.html 里 tileset 路径改成 ./tiles_rebuild/tileset.json
# 或:
python tools/rebuild_top/rebuild_top.py -i samples/OSGBny_3dtiles -o samples/OSGBny_3dtiles_rebuild -v
```

依赖：`tools/rebuild_top/requirements.txt`（脚本会自动建 `.venv`）。
