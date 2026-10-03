# 3D Tiles 合并

第一版提供 `merge-tilesets` 操作：把 2–64 份本地显式 Tileset 复制到独立成果目录，通过外部 Tileset 引用生成统一入口。它保留各输入的几何、纹理、LOD、transform、refine 和普通 extras；不做几何焊接、去重、重建或坐标转换。输入必须已经使用相同坐标参考系。

## 使用

工作区 → **3D Tiles 合并**，添加目录或入口 JSON，选择尚不存在的成果目录后提交。任务列表显示检查输入、合并、检查成果、提交成果四个阶段。重新处理会带回全部输入，并生成新的输出路径。

CLI 示例：

```powershell
rtk proxy cargo run -p processor -- merge-tilesets -i D:/data/a D:/data/b/tileset.json -o D:/results/merged
```

任务协议沿用 schemaVersion 1，`input.path` 为第一份输入，其他输入在 options 中：

```json
{
  "schemaVersion": 1,
  "taskId": "merge-example",
  "operation": "merge-tilesets",
  "input": {"path": "D:/data/a"},
  "output": {"path": "D:/results/merged"},
  "options": {"merge": {"additionalInputs": ["D:/data/b"]}}
}
```

输出结构：

```text
merged/
  tileset.json
  sources/source-001/<原入口 JSON 和完整源目录>
  sources/source-002/<原入口 JSON 和完整源目录>
```

入口的父级包围盒使用统一坐标下的保守 AABB。box 变换八个角点，sphere 使用仿射矩阵各行范数计算范围；WGS84 region 用区间运算转成 ECEF 范围，并遵循 region 不受 tile transform 影响的规则。包装节点不重复设置源 transform；父级误差使用输入误差与最大缩放的保守上界。跨日期变更线和极区保留保守包围范围，不保证包围盒最紧。

## 边界与保护

- 支持 asset.version 1.0/1.1 的显式树，以及现有 processor 校验器支持的网格内容。根必须声明 refine、boundingVolume 和 geometricError。
- 暂不支持隐式切片、多 contents、required 扩展、结构化元数据 schema/groups/classes。普通 extras 按原样保留。
- 资源必须是输入根内的本地相对引用；远程/绝对 URI、带查询参数或百分号编码的 URI 明确拒绝。GLB/B3DM 内的 glTF 外部 buffer/image 也会检查。GLB JSON 块限 64 MiB。
- 整个源目录会复制，包括未引用文件；请把每份数据放在独立目录。源目录中的符号链接、Windows junction/reparse point 明确拒绝。
- 预检先检查全部输入与输出/临时目录的重叠，再创建输出父目录。路径别名重复、已有输出、缺资源和越界引用均失败。
- 复制按 128 KiB 分块检查取消。校验成功后以现有 no-replace 提交机制生成成果；取消或失败只清理本任务拥有的临时目录。
- 不自动推断 local/ECEF 混用，也不证明输入的包围盒真实覆盖全部几何；这仍是输入数据的正确性要求。

## 自动化验证

```powershell
rtk proxy cargo test -p processor --lib stages::merge
rtk proxy cargo test -p processor --test merge_tilesets
rtk proxy cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib
# 在 apps/desktop 下运行
rtk proxy npm test
rtk proxy npm run test:e2e
```

CLI 测试使用真实 processor，覆盖中文路径、同名资源隔离、嵌套外部 Tileset、输入删除后成果仍有效、重复输入、第二份输入内的输出路径、已有输出、缺资源、复制时 stdin 取消。Unix CI 另外覆盖符号链接拒绝。

Playwright 测试替换桌面传输层，真实运行 processor，覆盖工作区入口→填写→提交→文件检查→任务阶段→重新处理，以及表单防错、配置恢复、后端错误。它不覆盖 Windows 原生文件选择器、安装包 DLL 部署或 WebView 实机渲染。Product Core CI 安装 Chromium 后自动执行；Windows 本地默认用已安装的 Edge，可通过 PLAYWRIGHT_CHANNEL 选择其他浏览器。

## 手动验收

在 apps/desktop 运行 `rtk proxy node scripts/create-merge-fixtures.mjs`，生成两份带地理定位的可渲染三角形样本，并打印输入与输出路径。样本是合成数据，用于检查流程和位置；不会证明城市级数据性能。重复生成时应使用新的目标目录。

使用本功能所在分支的开发桌面或后续包含该功能的安装版，完成：选择两份输入→合并→任务成功→成果预览出现两份模型；将成果移动到新目录后再预览；重新处理确认两份输入都恢复；指定已有输出验证不覆盖；复制大文件时取消，确认输入不变、正式成果不存在。当前旧安装包不包含新功能。

标准依据：[3D Tiles 外部 Tileset、坐标变换与包围盒](https://github.com/CesiumGS/3d-tiles/blob/main/specification/README.adoc)。
