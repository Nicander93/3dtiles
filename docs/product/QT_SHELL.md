# GeoForge Qt 桌面壳（geoforge_shell）

最小 Qt6 主窗口：左侧导航 + 中央内容。优先嵌入 Web UI；当前构建因 WebEngine **链接失败** 回退为系统浏览器 + 清晰状态横幅，并内嵌已有 **OsgbWidget** 加载 OSGBny。

路径：`apps/geoforge_shell/`（复用 `apps/osgb_viewer` 的 `OsgWidget` / `DatasetLoader`）。

## 依赖

与 `osgb_viewer` 相同：conda env `/workspace/conda/envs/osgb_viewer`（Qt6 Widgets + OpenGLWidgets + OSG）。

- **有** `Qt6::WebEngineWidgets` 且能链接：中央 `QWebEngineView` 加载 `http://127.0.0.1:8787/`（按导航切换路由）。
- **无 / 链接失败**（本机当前情况）：中央显示占位页 + 黄色状态横幅 +「在浏览器中打开」；「OSGB预览」切换到原生 `OsgWidget`。

请先启动 Web/API：

```bash
bash /workspace/repos/3dtiles/scripts/run_geoforge.sh
# UI http://127.0.0.1:8787/
```

## P7b：Qt6 WebEngine 尝试（2026-09-09 Asia/Shanghai）

### 安装（成功）

```bash
export MAMBA_ROOT_PREFIX=/workspace/conda
eval "$(/home/box/bin/micromamba shell hook -s bash)"
micromamba activate osgb_viewer
micromamba install -y -n osgb_viewer -c conda-forge qt6-webengine=6.11.2
```

- 包：`qt6-webengine 6.11.2 pl5321h60ec477_0`（+ nss/libudev1 等共 20 包）
- CMake：`find_package(Qt6 COMPONENTS WebEngineWidgets)` → **FOUND**
- 源码已支持 `#ifdef GEOFORGE_HAS_WEBENGINE` + `QWebEngineView`；`main.cpp` 在启用时设置 `AA_ShareOpenGLContexts`

### 链接（失败 — 无法在本机产出 WebEngine 二进制）

`cmake --build` 在链接 `libQt6WebEngineWidgets` 时失败。conda `libudev.so.1` 相对 conda 编译器 sysroot 需要更新的 glibc 符号：

```text
x86_64-conda-linux-gnu-ld: .../lib/libudev.so.1: undefined reference to `fcntl64@GLIBC_2.28'
.../libudev.so.1: undefined reference to `fstat64@GLIBC_2.33'
.../libudev.so.1: undefined reference to `__explicit_bzero_chk@GLIBC_2.25'
.../libudev.so.1: undefined reference to `fstatat64@GLIBC_2.33'
.../libudev.so.1: undefined reference to `stat64@GLIBC_2.33'
.../libudev.so.1: undefined reference to `reallocarray@GLIBC_2.26'
.../libudev.so.1: undefined reference to `gettid@GLIBC_2.30'
.../libudev.so.1: undefined reference to `dlsym@GLIBC_2.34'
.../libudev.so.1: undefined reference to `statx@GLIBC_2.28'
.../libudev.so.1: undefined reference to `pthread_sigmask@GLIBC_2.32'
.../libudev.so.1: undefined reference to `getrandom@GLIBC_2.25'
.../libudev.so.1: undefined reference to `pthread_once@GLIBC_2.34'
collect2: error: ld returned 1 exit status
```

尝试用系统 `/lib/x86_64-linux-gnu` 优先链接会进一步打乱 conda sysroot（`__libc_csu_*` / harfbuzz 符号错误）。结论：包可装，但 **与当前 osgb_viewer conda toolchain 不兼容**，无法嵌入 WebEngine。

### 当前交付构建

```bash
cmake -S . -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$CONDA_PREFIX" \
  -DGEOFORGE_ENABLE_WEBENGINE=OFF
cmake --build build -j
```

CMake 选项 `GEOFORGE_ENABLE_WEBENGINE`（默认 ON）：设 OFF 跳过 WebEngine 链接。回退 UI 含黄色横幅与状态栏「浏览器回退」。

## 构建（通用）

```bash
export MAMBA_ROOT_PREFIX=/workspace/conda
eval "$(/home/box/bin/micromamba shell hook -s bash)"
micromamba activate osgb_viewer

cd /workspace/repos/3dtiles/apps/geoforge_shell
cmake -S . -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$CONDA_PREFIX" \
  -DGEOFORGE_ENABLE_WEBENGINE=OFF
cmake --build build -j
```

产物：`apps/geoforge_shell/build/geoforge_shell`

## 运行

```bash
export LD_LIBRARY_PATH="$CONDA_PREFIX/lib:${LD_LIBRARY_PATH:-}"
export OSG_LIBRARY_PATH="$CONDA_PREFIX/lib/osgPlugins-3.6.5"
export DISPLAY=:2   # 本机桌面

./build/geoforge_shell
```

### 左侧导航

| 项 | 行为 |
| --- | --- |
| 工作区 | Web `/` 或浏览器回退 |
| 转换 | Web `/osgb/convert` |
| 任务 | Web `/processing` |
| OSGB预览 | **内嵌** `OsgWidget`，自动加载 `/workspace/data/OSGBny/OSGBny` |
| Tiles预览 | Web `/preview/tiles` |

底部状态栏显示当前模式（WebEngine / 浏览器回退 / OSGB 原生）。

## 截图

已在 DISPLAY=:2 拍摄：`docs/product/geoforge_shell_shot.png`（工作区回退页+横幅）、`docs/product/geoforge_shell_osgb.png`（OSGB 内嵌页）。也可自行重拍：

```bash
DISPLAY=:2 ffmpeg -y -f x11grab -video_size 1280x800 -i :2.0 -frames:v 1 -update 1 \
  /workspace/repos/3dtiles/docs/product/geoforge_shell_shot.png
```

## 与 osgb_viewer 关系

- `apps/osgb_viewer`：独立原生 OSGB 预览工具（菜单/工具栏）。
- `apps/geoforge_shell`：产品主壳雏形，导航对齐 Web 侧；OSGB 页复用同一套 widget 源码。

不修改 `/workspace/runtime/3dtile-bin`；勿破坏 `runtime/3dtile-bin-ktx2`。
