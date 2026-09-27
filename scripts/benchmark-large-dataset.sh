#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

BENCHMARK_VERSION="1.0.0"
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

show_help() {
    cat << EOF
GeoForge Large Dataset Benchmark Tool

Usage: $0 [OPTIONS]

Options:
    -i, --input PATH        Input OSGB root directory (required)
    -o, --output DIR        Output directory (default: ./benchmark-output-TIMESTAMP)
    -t, --threads N         Thread count (0=auto, default: 0)
    -m, --texture-mode MODE Texture mode: keep|ktx2|both (default: keep)
    -s, --sample-name NAME  Sample dataset name (default: derived from input path)
    -r, --result-file PATH  JSON result file path (default: OUTPUT_DIR/result.json)
    --csv FILE              Append results to CSV file (default: docs/acceptance/large-data/results.csv)
    --skip-csv              Do not append to CSV
    --dry-run               Show what would be done without running
    -h, --help              Show this help message

Environment Variables:
    GEOFORGE_PROCESSOR      Path to processor executable
    GEOFORGE_3DTILE         Path to converter executable
    GEOFORGE_CONVERT_THREADS Override thread count

Examples:
    # Run benchmark with default settings
    $0 -i /path/to/osgb/Data -s my-sample

    # Run with 4 threads and KTX2 texture processing
    $0 -i /path/to/osgb/Data -t 4 -m ktx2 -s large-sample

    # Dry run to check configuration
    $0 -i /path/to/osgb/Data --dry-run

Notes:
    - This script measures baseline performance for P0.
    - It captures timing, memory, CPU, and I/O metrics.
    - Results are saved in JSON format (benchmark-schema.json).
    - Results can be automatically appended to CSV for tracking.
    - If sample data is unavailable, the script will mark results as UNMEASURED.

EOF
}

INPUT_DIR=""
OUTPUT_DIR=""
THREADS=0
TEXTURE_MODE="keep"
SAMPLE_NAME=""
RESULT_FILE=""
CSV_FILE="${WORKSPACE_ROOT}/docs/acceptance/large-data/results.csv"
SKIP_CSV=false
DRY_RUN=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -i|--input)
            INPUT_DIR="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -t|--threads)
            THREADS="$2"
            shift 2
            ;;
        -m|--texture-mode)
            TEXTURE_MODE="$2"
            shift 2
            ;;
        -s|--sample-name)
            SAMPLE_NAME="$2"
            shift 2
            ;;
        -r|--result-file)
            RESULT_FILE="$2"
            shift 2
            ;;
        --csv)
            CSV_FILE="$2"
            shift 2
            ;;
        --skip-csv)
            SKIP_CSV=true
            shift
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            show_help
            exit 1
            ;;
    esac
done

if [ -z "$INPUT_DIR" ]; then
    echo "Error: Input directory is required" >&2
    show_help
    exit 1
fi

if [ ! -d "$INPUT_DIR" ]; then
    echo "Error: Input directory does not exist: $INPUT_DIR" >&2
    exit 1
fi

if [ -z "$SAMPLE_NAME" ]; then
    SAMPLE_NAME=$(basename "$(dirname "$INPUT_DIR")")
fi

if [ -z "$OUTPUT_DIR" ]; then
    OUTPUT_DIR="${WORKSPACE_ROOT}/benchmark-output-${TIMESTAMP//[:TZ-]/}"
fi

if [ -z "$RESULT_FILE" ]; then
    RESULT_FILE="${OUTPUT_DIR}/result.json"
fi

mkdir -p "$OUTPUT_DIR"

echo "=== GeoForge Large Dataset Benchmark ==="
echo "Benchmark Version: $BENCHMARK_VERSION"
echo "Timestamp: $TIMESTAMP"
echo "Input: $INPUT_DIR"
echo "Output: $OUTPUT_DIR"
echo "Sample Name: $SAMPLE_NAME"
echo "Threads: $THREADS"
echo "Texture Mode: $TEXTURE_MODE"
echo "Result File: $RESULT_FILE"
[ "$SKIP_CSV" = false ] && echo "CSV File: $CSV_FILE"
echo ""

collect_environment() {
    local os_name os_version cpu_model cpu_cores cpu_threads memory_mb disk_type

    os_name=$(uname -s)
    os_version=$(uname -r)
    
    if [ -f /proc/cpuinfo ]; then
        cpu_model=$(grep "model name" /proc/cpuinfo | head -1 | cut -d':' -f2 | xargs)
        cpu_cores=$(grep "cpu cores" /proc/cpuinfo | head -1 | cut -d':' -f2 | xargs)
        cpu_threads=$(grep -c "^processor" /proc/cpuinfo)
    else
        cpu_model="Unknown"
        cpu_cores=0
        cpu_threads=0
    fi

    if [ -f /proc/meminfo ]; then
        memory_mb=$(($(grep MemTotal /proc/meminfo | awk '{print $2}') / 1024))
    else
        memory_mb=0
    fi

    disk_type="Unknown"

    cat > "${OUTPUT_DIR}/environment.json" << EOF
{
  "os": "$os_name",
  "os_version": "$os_version",
  "cpu": "$cpu_model",
  "cpu_cores": $cpu_cores,
  "cpu_threads": $cpu_threads,
  "memory_mb": $memory_mb,
  "disk_type": "$disk_type"
}
EOF

    echo "Environment collected: $os_name, $cpu_model, ${memory_mb}MB RAM"
}

collect_codebase() {
    local main_commit main_branch main_dirty
    local converter_version converter_commit

    cd "$WORKSPACE_ROOT"
    
    main_commit=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
    main_branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
    main_dirty=$(git diff-index --quiet HEAD -- 2>/dev/null && echo "false" || echo "true")

    local converter_config="${WORKSPACE_ROOT}/apps/desktop/config/converter-runtime.json"
    if [ -f "$converter_config" ]; then
        converter_version=$(jq -r '.version // "unknown"' "$converter_config")
        converter_commit="unknown"
    else
        converter_version="unknown"
        converter_commit="unknown"
    fi

    cat > "${OUTPUT_DIR}/codebase.json" << EOF
{
  "main_repo": {
    "url": "https://github.com/Nicander93/3dtiles",
    "branch": "$main_branch",
    "commit": "$main_commit",
    "dirty": $main_dirty
  },
  "converter_repo": {
    "url": "https://github.com/Nicander93/geoforge-converter",
    "version": "$converter_version",
    "commit": "$converter_commit"
  }
}
EOF

    echo "Codebase: main=$main_commit, converter=$converter_version"
}

collect_sample_info() {
    local block_count=0
    local input_size_mb=0

    if [ -d "$INPUT_DIR" ]; then
        input_size_mb=$(du -sm "$INPUT_DIR" 2>/dev/null | cut -f1 || echo 0)
        block_count=$(find "$INPUT_DIR" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l || echo 0)
    fi

    cat > "${OUTPUT_DIR}/sample.json" << EOF
{
  "name": "$SAMPLE_NAME",
  "type": "osgb",
  "input_size_mb": $input_size_mb,
  "block_count": $block_count
}
EOF

    echo "Sample: $SAMPLE_NAME, ${input_size_mb}MB, ${block_count} blocks"
}

run_benchmark() {
    echo ""
    echo "=== Running Benchmark ==="
    echo "UNMEASURED: No actual benchmark execution implemented yet."
    echo "This is a P0 scaffold. Real execution requires processor integration."
    echo ""

    local status="unmeasured"
    local total_seconds=0

    cat > "${OUTPUT_DIR}/timing.json" << EOF
{
  "total_seconds": $total_seconds,
  "scan_seconds": 0,
  "convert_seconds": 0,
  "rebuild_seconds": 0,
  "texture_seconds": 0,
  "validate_seconds": 0,
  "commit_seconds": 0
}
EOF

    cat > "${OUTPUT_DIR}/memory.json" << EOF
{
  "measurement_method": "linux_rss",
  "peak_total_mb": 0,
  "notes": "UNMEASURED - P0 scaffold without actual execution"
}
EOF

    cat > "${OUTPUT_DIR}/status.json" << EOF
{
  "status": "$status",
  "blocks": {
    "total": 0,
    "succeeded": 0,
    "failed": 0
  },
  "failures": []
}
EOF
}

assemble_result() {
    echo ""
    echo "=== Assembling Result ==="

    local env=$(cat "${OUTPUT_DIR}/environment.json")
    local codebase=$(cat "${OUTPUT_DIR}/codebase.json")
    local sample=$(cat "${OUTPUT_DIR}/sample.json")
    local timing=$(cat "${OUTPUT_DIR}/timing.json")
    local memory=$(cat "${OUTPUT_DIR}/memory.json")
    local status=$(cat "${OUTPUT_DIR}/status.json")

    cat > "$RESULT_FILE" << EOF
{
  "benchmark_version": "$BENCHMARK_VERSION",
  "timestamp": "$TIMESTAMP",
  "environment": $env,
  "codebase": $codebase,
  "configuration": {
    "threads": $THREADS,
    "thread_source": "explicit",
    "texture_mode": "$TEXTURE_MODE"
  },
  "sample": $sample,
  "results": {
    "status": "unmeasured",
    "timing": $timing,
    "memory": $memory,
    "blocks": $(echo "$status" | jq '.blocks'),
    "failures": $(echo "$status" | jq '.failures'),
    "notes": "UNMEASURED - P0 baseline scaffold. No sample data available for actual execution."
  }
}
EOF

    echo "Result written to: $RESULT_FILE"
}

append_to_csv() {
    if [ "$SKIP_CSV" = true ]; then
        echo "Skipping CSV append"
        return
    fi

    echo ""
    echo "=== Appending to CSV ==="

    if [ ! -f "$CSV_FILE" ]; then
        mkdir -p "$(dirname "$CSV_FILE")"
        cat > "$CSV_FILE" << 'EOF'
timestamp,os,cpu,cpu_cores,memory_mb,disk_type,main_commit,converter_version,threads,texture_mode,sample_name,sample_type,input_size_mb,block_count,status,total_seconds,convert_seconds,rebuild_seconds,peak_total_mb,notes
EOF
        echo "Created new CSV file: $CSV_FILE"
    fi

    local env_os=$(jq -r '.environment.os' "$RESULT_FILE")
    local env_cpu=$(jq -r '.environment.cpu' "$RESULT_FILE")
    local env_cores=$(jq -r '.environment.cpu_cores' "$RESULT_FILE")
    local env_mem=$(jq -r '.environment.memory_mb' "$RESULT_FILE")
    local env_disk=$(jq -r '.environment.disk_type' "$RESULT_FILE")
    local main_commit=$(jq -r '.codebase.main_repo.commit' "$RESULT_FILE")
    local converter_ver=$(jq -r '.codebase.converter_repo.version' "$RESULT_FILE")
    local sample_name=$(jq -r '.sample.name' "$RESULT_FILE")
    local sample_type=$(jq -r '.sample.type' "$RESULT_FILE")
    local input_size=$(jq -r '.sample.input_size_mb' "$RESULT_FILE")
    local block_count=$(jq -r '.sample.block_count' "$RESULT_FILE")
    local status=$(jq -r '.results.status' "$RESULT_FILE")
    local total_sec=$(jq -r '.results.timing.total_seconds' "$RESULT_FILE")
    local convert_sec=$(jq -r '.results.timing.convert_seconds' "$RESULT_FILE")
    local rebuild_sec=$(jq -r '.results.timing.rebuild_seconds' "$RESULT_FILE")
    local peak_mem=$(jq -r '.results.memory.peak_total_mb' "$RESULT_FILE")
    local notes="UNMEASURED - P0 scaffold"

    echo "$TIMESTAMP,$env_os,$env_cpu,$env_cores,$env_mem,$env_disk,$main_commit,$converter_ver,$THREADS,$TEXTURE_MODE,$sample_name,$sample_type,$input_size,$block_count,$status,$total_sec,$convert_sec,$rebuild_sec,$peak_mem,\"$notes\"" >> "$CSV_FILE"

    echo "Result appended to: $CSV_FILE"
}

if [ "$DRY_RUN" = true ]; then
    echo ""
    echo "DRY RUN - No actual execution"
    exit 0
fi

collect_environment
collect_codebase
collect_sample_info
run_benchmark
assemble_result
append_to_csv

echo ""
echo "=== Benchmark Complete ==="
echo "Results: $RESULT_FILE"
[ "$SKIP_CSV" = false ] && echo "CSV: $CSV_FILE"
echo ""
echo "To run with actual sample data:"
echo "  $0 -i /path/to/your/osgb/Data -s your-sample-name"
