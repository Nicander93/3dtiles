# texture_ktx2

产品路径的 3D Tiles 纹理 KTX2 后处理（BasisU / `KHR_texture_basisu`）。

```bash
python tools/texture_ktx2/run.py -i TILESET_DIR --mode ktx2-etc1s --basisu /path/to/basisu
python tools/texture_ktx2/run.py -i TILESET_DIR -o OUT --mode ktx2-uastc --report report.json
```

打包：用 PyInstaller onedir 生成 `geoforge-texture.exe`，与 BasisU 一并放入 `resources/runtime/texture/`。

```powershell
# 构建机示例（需已安装 pyinstaller）
cd tools/texture_ktx2
pyinstaller --onedir --name geoforge-texture run.py
```

环境变量：`GEOFORGE_BASISU`、`GEOFORGE_TEXTURE`（正式包指向 geoforge-texture.exe）。
