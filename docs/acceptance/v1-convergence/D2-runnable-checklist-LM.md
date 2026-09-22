# D2 Runnable Sample Path - LM Checklist

**目标:** 将 D2 inventory 转换为可运行的 sample path，供 parent 在 LM (machineId) 上执行

**前提:** 无法从 cloud agent 访问 LM，需要 parent 手动在 LM 上执行

**LM 环境信息:**
- **Machine ID:** LM
- **Repository Path:** `D:\code\3dtiles`
- **Expected Data Locations:** (待 parent 确认)
  - LandsD: 可能在 `D:\data\` 或其他数据盘
  - PlanD: 可能在 `D:\data\` 或其他数据盘

---

## Checklist for Parent (Run on LM)

### Step 1: Locate Data

在 LM 上找到 LandsD / PlanD 数据路径：

```powershell
# 搜索可能的数据位置
Get-ChildItem -Path D:\ -Filter "*.json" -Recurse -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "*tileset*" }

# 或者手动检查已知路径
dir D:\data\
dir E:\data\
dir F:\data\
```

**Expected Output:**
- LandsD tileset.json 路径: `_______________` (parent fill in)
- PlanD tileset.json 路径: `_______________` (parent fill in)

### Step 2: Inspect Data Size

确定数据集大小，决定是否需要 subset：

```powershell
# 检查目录大小
$path = "D:\data\LandsD"  # adjust path
$size = (Get-ChildItem -Path $path -Recurse | Measure-Object -Property Length -Sum).Sum
"{0:N2} GB" -f ($size / 1GB)

# 检查 tile 数量
(Get-ChildItem -Path $path -Filter "*.b3dm" -Recurse).Count
(Get-ChildItem -Path $path -Filter "Tile_*" -Recurse -Directory).Count
```

**Decision Criteria:**
- **< 500 MB:** 可以直接运行完整数据集
- **500 MB - 2 GB:** 考虑 subset (例如只选一个区域的 blocks)
- **> 2 GB:** 必须 subset

**Parent Record:**
- LandsD size: `_______________`
- PlanD size: `_______________`
- Use subset? YES / NO

### Step 3: Define Minimal Sample

如果需要 subset，选择最小可代表性样本：

**Subset Strategy A: Single Block**
```powershell
# 找到一个包含多 LOD 的 block
$blockPath = "D:\data\LandsD\Tile_+XXX_+YYY"  # choose one

# 复制到临时 sample 目录
$samplePath = "D:\code\3dtiles\acceptance-samples\d2-landsd-sample"
New-Item -ItemType Directory -Force -Path $samplePath
Copy-Item -Recurse "$blockPath" "$samplePath\"

# 创建最小的 root tileset.json 指向这个 block
@"
{
  \"asset\": {\"version\": \"1.0\"},
  \"geometricError\": 1000,
  \"root\": {
    \"geometricError\": 1000,
    \"boundingVolume\": {\"region\": [...]},
    \"refine\": \"REPLACE\",
    \"children\": [
      {\"content\": {\"uri\": \"Tile_+XXX_+YYY/tileset.json\"}}
    ]
  }
}
"@ | Out-File -Encoding UTF8 "$samplePath\tileset.json"
```

**Subset Strategy B: 2x2 Block Grid**
```powershell
# 选择 4 个相邻 blocks
$blocks = @("Tile_+000_+000", "Tile_+001_+000", "Tile_+000_+001", "Tile_+001_+001")

# 复制所有 blocks
foreach ($block in $blocks) {
    Copy-Item -Recurse "D:\data\LandsD\$block" "$samplePath\"
}

# 创建 root tileset.json with 4 children
```

**Parent Decision:**
- Subset strategy: A (single block) / B (2x2 grid) / NONE (full data)
- Selected block(s): `_______________`
- Sample path: `_______________`

### Step 4: Test Sample Path

验证 sample 可以被 processor 读取：

```powershell
cd D:\code\3dtiles

# Build processor (if not already built)
cargo build --release -p processor

# Test validate on sample
.\target\release\processor.exe process-tileset `
    --input "D:\code\3dtiles\acceptance-samples\d2-landsd-sample" `
    --output "D:\code\3dtiles\acceptance-results\test-d2" `
    --rebuild-top

# Expected: Should complete without crash
# Check output size
(Get-ChildItem -Path "D:\code\3dtiles\acceptance-results\test-d2" -Recurse | Measure-Object -Property Length -Sum).Sum / 1MB
```

**Parent Record:**
- Test run: SUCCESS / FAILED
- Error (if failed): `_______________`
- Output size: `_______________` MB

### Step 5: Document Sample Metadata

记录 sample 的元数据，供 run-acceptance.sh 使用：

```powershell
# 生成 sample metadata
@"
{
  \"sampleId\": \"d2-landsd-sample-1\",
  \"sourceData\": \"LandsD city 2024\",
  \"sampleType\": \"single-block\",
  \"blocks\": [\"Tile_+XXX_+YYY\"],
  \"inputPath\": \"D:\\\\code\\\\3dtiles\\\\acceptance-samples\\\\d2-landsd-sample\",
  \"inputSize\": \"XXX MB\",
  \"tileCount\": \"YYY\",
  \"lodLevels\": \"Z\",
  \"coverage\": \"Partial (single block from full city)\",
  \"license\": \"Internal use only\",
  \"notes\": \"Minimal representative sample for acceptance testing\"
}
"@ | Out-File -Encoding UTF8 "D:\code\3dtiles\acceptance-samples\d2-landsd-sample\sample-metadata.json"
```

**Parent Record:**
- Metadata created: YES / NO
- Sample ID: `_______________`

### Step 6: Update D2 Inventory Doc

更新 `docs/acceptance/v1-convergence/R08-4-ci-d2-inventory.md` with actual paths：

**Parent Action:**
```powershell
# Edit R08-4-ci-d2-inventory.md on LM
# Add section:
```

```markdown
## D2 LandsD Sample (LM-only)

**Path on LM:** `D:\code\3dtiles\acceptance-samples\d2-landsd-sample`

**Sample Metadata:**
- Source: LandsD city 2024
- Sample type: Single block / 2x2 grid
- Input size: XXX MB
- Tile count: YYY
- LOD levels: Z

**How to Run:**
```powershell
cd D:\code\3dtiles\docs\acceptance\v1-convergence

# Adjust run-acceptance.sh for Windows (or use WSL)
.\scripts\run-acceptance.sh --level d2-sample --input "D:\code\3dtiles\acceptance-samples\d2-landsd-sample"
```
```

### Step 7: Extend run-acceptance.sh for D2

在 LM 上修改 `run-acceptance.sh` 支持 D2 external path：

**Parent Action (on LM, in WSL or Git Bash):**

```bash
cd /d/code/3dtiles/docs/acceptance/v1-convergence

# Edit scripts/run-acceptance.sh
# Add d2-sample case:

case "$LEVEL" in
    d0)
        # existing D0 logic
        ;;
    d1)
        # existing D1 logic
        ;;
    d2-sample)
        log "Running D2 sample (LM-only, external path)..."
        
        # Parent provides input path via --input flag
        if [[ -z "$INPUT_PATH" ]]; then
            error "D2 sample requires --input <path>"
        fi
        
        D2_OUTPUT="$OUTPUT_DIR/d2-sample"
        mkdir -p "$D2_OUTPUT"
        
        FIXTURE="sample"
        FIXTURE_OUTPUT="$D2_OUTPUT/$FIXTURE"
        
        log "Processing D2 sample from: $INPUT_PATH"
        mkdir -p "$FIXTURE_OUTPUT"
        
        # Run processor with --rebuild-top
        START_TIME=$(date +%s)
        if "$PROCESSOR_PATH" process-tileset \
            --input "$INPUT_PATH" \
            --output "$FIXTURE_OUTPUT/output" \
            --rebuild-top \
            > "$FIXTURE_OUTPUT/logs.txt" 2>&1; then
            PASSED=true
            EXIT_CODE=0
        else
            PASSED=false
            EXIT_CODE=$?
        fi
        END_TIME=$(date +%s)
        DURATION=$((END_TIME - START_TIME))
        
        # Run all checks (GE, frontier, subtree)
        # (similar to D1 logic)
        
        log "D2 sample: $([[ $PASSED == true ]] && echo "✅ PASS" || echo "❌ FAIL") ($DURATION s)"
        ;;
esac
```

**Parent Record:**
- run-acceptance.sh extended: YES / NO

### Step 8: Run Full D2 Acceptance Test

执行完整的 D2 acceptance test：

```bash
cd /d/code/3dtiles/docs/acceptance/v1-convergence

./scripts/run-acceptance.sh --level d2-sample --input "/d/code/3dtiles/acceptance-samples/d2-landsd-sample"
```

**Expected Output:**
- `acceptance-results/v1-convergence/<run-id>/d2-sample/sample/status.json`
- All checks run: GE monotonicity, frontier, subtree
- May PASS or FAIL depending on data quality

**Parent Record:**
- Test run: SUCCESS / FAILED
- Overall result: PASS / FAIL
- Run ID: `_______________`
- SHA recorded in status.json: `_______________`

### Step 9: Archive Results

保存结果供 cloud agent review：

```powershell
# Zip results
Compress-Archive -Path "D:\code\3dtiles\acceptance-results\v1-convergence\<run-id>" `
    -DestinationPath "D:\code\3dtiles\d2-sample-results.zip"

# Parent uploads to shared location or reports back to cloud agent
```

**Parent Action:**
- Results archived: YES / NO
- Uploaded to: `_______________`

---

## Expected Deliverables (Parent → Cloud Agent)

After completing checklist, parent provides:

1. **Data Paths:**
   - LandsD path: `_______________`
   - PlanD path: `_______________`
   - Sample path: `_______________`

2. **Sample Metadata:**
   - Sample type: single-block / 2x2-grid / full
   - Input size: `_______________` MB
   - Tile count: `_______________`
   - LOD levels: `_______________`

3. **Test Results:**
   - Run ID: `_______________`
   - Overall PASS/FAIL: `_______________`
   - GE monotonicity: `_______________`
   - Frontier coverage: `_______________`
   - Subtree retention: `_______________`

4. **Issues Encountered:**
   - `_______________` (if any)

5. **Next Steps:**
   - ☐ D2 sample validated and runnable
   - ☐ run-acceptance.sh supports D2 external path
   - ☐ Results recorded in status.md
   - ☐ Ready for Cesium A/B baseline (once D2 available)

---

## Cloud Agent Cannot Do (Requires Parent on LM)

❌ **Cloud agent 无法执行的操作:**
- 访问 LM 本地磁盘 (`D:\`, `E:\` 等)
- 搜索 LandsD/PlanD 数据路径
- 读取实际 tileset 大小
- 运行 Windows-specific commands (PowerShell)
- 验证 LM 本地 processor build

✅ **Cloud agent 可以准备的:**
- 此 checklist 文档
- run-acceptance.sh 扩展逻辑（示例代码）
- D2 sample metadata schema
- 继续 R09 其他 Layer B slices (不依赖 D2 数据)

---

## Alternative: Use Cloud-Accessible D2 Placeholder

如果 parent 可以上传 D2 sample 到 cloud-accessible location:

```bash
# Parent uploads sample to GitHub LFS or similar
cd /d/code/3dtiles
git lfs track "acceptance-samples/d2-landsd-sample/**"
git add acceptance-samples/
git commit -m "Add D2 LandsD sample for acceptance testing"
git push

# Cloud agent can then access via git pull
```

**Parent Decision:**
- Upload sample to repo? YES / NO (consider size/license restrictions)

---

## Summary

**Current Status:** D2 inventory defined (R08.4), but NOT runnable

**This Checklist Goal:** Make D2 runnable on LM (local-only, no cloud access)

**Parent Action Required:** Execute Steps 1-9 on LM, report back results

**Cloud Agent Next:** Continue R09 Layer B slices (BV tightness / Transform consistency) while waiting for parent D2 execution

**No Fake Data:** All paths/sizes/results must be real from LM execution

**No Fake A/B:** A/B remains blocked until D2 sample validated on LM
