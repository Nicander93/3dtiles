#!/usr/bin/env bash
set -euo pipefail

# run-acceptance.sh - R08 Acceptance Test Harness
# Usage: ./run-acceptance.sh --level d0 --output-dir ../../acceptance-results/v1-convergence/<run-id>

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/../fixtures"
DEFAULT_OUTPUT_BASE="$REPO_ROOT/acceptance-results/v1-convergence"

LEVEL="d0"
OUTPUT_DIR=""
PROCESSOR_PATH="$REPO_ROOT/target/release/processor"

usage() {
    cat << EOF
Usage: $0 [options]

Options:
    --level LEVEL           Data gradient level: d0, d1, d2 (default: d0)
    --output-dir DIR        Output directory for results (default: auto-generated timestamp)
    --processor PATH        Path to processor binary (default: target/release/processor)
    -h, --help              Show this help

Examples:
    # Run D0 with auto-generated output dir
    $0 --level d0

    # Run D0 with specific output dir
    $0 --level d0 --output-dir /tmp/my-acceptance-run

    # Use debug build
    $0 --level d0 --processor target/debug/processor
EOF
    exit 1
}

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"
}

error() {
    log "ERROR: $*" >&2
    exit 1
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --level)
            LEVEL="$2"
            shift 2
            ;;
        --output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --processor)
            PROCESSOR_PATH="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            error "Unknown option: $1"
            ;;
    esac
done

# Validate level
if [[ ! "$LEVEL" =~ ^(d0|d1|d2)$ ]]; then
    error "Invalid level: $LEVEL (must be d0, d1, or d2)"
fi

# Auto-generate output dir if not specified
if [[ -z "$OUTPUT_DIR" ]]; then
    RUN_ID="$(date '+%Y%m%d-%H%M%S')"
    OUTPUT_DIR="$DEFAULT_OUTPUT_BASE/$RUN_ID"
fi

# Check processor exists
if [[ ! -x "$PROCESSOR_PATH" ]]; then
    error "Processor not found or not executable: $PROCESSOR_PATH"
fi

log "=== R08 Acceptance Test Harness ==="
log "Level: $LEVEL"
log "Output: $OUTPUT_DIR"
log "Processor: $PROCESSOR_PATH"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Get git info
TIP_SHA=$(cd "$REPO_ROOT" && git rev-parse HEAD)
TIP_SHA_SHORT=$(cd "$REPO_ROOT" && git rev-parse --short HEAD)

# Hash processor binary
PROCESSOR_HASH=$(shasum -a 256 "$PROCESSOR_PATH" | cut -d' ' -f1)

# Get Rust version
RUST_VERSION=$(rustc --version | cut -d' ' -f2)

# Create run-info.json
cat > "$OUTPUT_DIR/run-info.json" << EOF
{
  "runId": "$(basename "$OUTPUT_DIR")",
  "timestamp": "$(date -u '+%Y-%m-%dT%H:%M:%SZ')",
  "tipSha": "$TIP_SHA",
  "tipShaShort": "$TIP_SHA_SHORT",
  "level": "$LEVEL",
  "binaries": {
    "processor": {
      "path": "$PROCESSOR_PATH",
      "sha256": "$PROCESSOR_HASH"
    }
  },
  "parameters": {
    "${LEVEL}Enabled": true,
    "textureMode": "keep"
  },
  "environment": {
    "os": "$(uname -s)",
    "rustVersion": "$RUST_VERSION"
  }
}
EOF

log "Created run-info.json"

# Run tests based on level
case "$LEVEL" in
    d0)
        log "Running D0 tiny fixtures..."
        D0_DIR="$FIXTURES_DIR/d0-tiny"
        D0_OUTPUT="$OUTPUT_DIR/d0-tiny"
        mkdir -p "$D0_OUTPUT"
        
        # Run single-tile fixture
        FIXTURE="single-tile"
        FIXTURE_INPUT="$D0_DIR/$FIXTURE"
        FIXTURE_OUTPUT="$D0_OUTPUT/$FIXTURE"
        
        if [[ ! -d "$FIXTURE_INPUT" ]]; then
            error "D0 fixture not found: $FIXTURE_INPUT"
        fi
        
        log "Processing fixture: $FIXTURE"
        mkdir -p "$FIXTURE_OUTPUT"
        
        # Copy input for reference
        cp -r "$FIXTURE_INPUT" "$FIXTURE_OUTPUT/input"
        
        # Run processor
        START_TIME=$(date +%s)
        if "$PROCESSOR_PATH" process-tileset \
            --input "$FIXTURE_INPUT" \
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
        
        # Count errors/warnings in logs
        ERRORS=$(grep -c "ERROR" "$FIXTURE_OUTPUT/logs.txt" || true)
        WARNINGS=$(grep -c "WARN" "$FIXTURE_OUTPUT/logs.txt" || true)
        
        # Check if output tileset exists
        if [[ -f "$FIXTURE_OUTPUT/output/tileset.json" ]]; then
            OUTPUT_SIZE=$(du -sb "$FIXTURE_OUTPUT/output" | cut -f1)
            LAYER_A="PASS"
        else
            OUTPUT_SIZE=0
            LAYER_A="FAIL"
            PASSED=false
        fi
        
        # Create status.json for fixture
        cat > "$FIXTURE_OUTPUT/status.json" << EOF
{
  "fixture": "d0-tiny/$FIXTURE",
  "passed": $PASSED,
  "exitCode": $EXIT_CODE,
  "duration": "${DURATION}s",
  "validation": {
    "layerA": "$LAYER_A",
    "errors": $ERRORS,
    "warnings": $WARNINGS
  },
  "outputSize": "$OUTPUT_SIZE bytes"
}
EOF
        
        if $PASSED; then
            log "✅ $FIXTURE: PASS ($DURATION s)"
        else
            log "❌ $FIXTURE: FAIL ($DURATION s, exit $EXIT_CODE)"
        fi
        ;;
        
    d1)
        log "D1 medium fixtures: Not yet implemented (R08.2+)"
        error "D1 level not yet supported"
        ;;
        
    d2)
        log "D2 real datasets: Not yet implemented (R08.3+)"
        error "D2 level not yet supported"
        ;;
esac

# Create summary
SUMMARY_FILE="$OUTPUT_DIR/summary.md"
cat > "$SUMMARY_FILE" << EOF
# Acceptance Test Run Summary

**Run ID**: $(basename "$OUTPUT_DIR")  
**Timestamp**: $(date -u '+%Y-%m-%d %H:%M:%S UTC')  
**Git SHA**: $TIP_SHA_SHORT ($TIP_SHA)  
**Level**: $LEVEL

## Results

EOF

if [[ "$LEVEL" == "d0" ]]; then
    FIXTURE_STATUS=$(cat "$OUTPUT_DIR/d0-tiny/single-tile/status.json")
    FIXTURE_PASSED=$(echo "$FIXTURE_STATUS" | grep -o '"passed": [^,]*' | cut -d' ' -f2)
    
    if [[ "$FIXTURE_PASSED" == "true" ]]; then
        STATUS_ICON="✅"
        OVERALL_RESULT="PASS"
    else
        STATUS_ICON="❌"
        OVERALL_RESULT="FAIL"
    fi
    
    cat >> "$SUMMARY_FILE" << EOF
### D0 Tiny Fixtures

| Fixture | Status | Duration | Errors | Warnings |
|---------|--------|----------|--------|----------|
| single-tile | $STATUS_ICON | $(echo "$FIXTURE_STATUS" | grep -o '"duration": "[^"]*"' | cut -d'"' -f4) | $(echo "$FIXTURE_STATUS" | grep -o '"errors": [^,]*' | cut -d' ' -f2) | $(echo "$FIXTURE_STATUS" | grep -o '"warnings": [^,}]*' | cut -d' ' -f2) |

**Overall**: $STATUS_ICON $OVERALL_RESULT

EOF
fi

cat >> "$SUMMARY_FILE" << EOF
## Evidence Non-Inheritance

**This result is valid ONLY for git SHA $TIP_SHA.**

Historical results do NOT apply to new commits. Each code change requires a new acceptance run.

## Files

- \`run-info.json\`: Run metadata (SHA, binary hashes, parameters)
- \`$LEVEL/*/status.json\`: Per-fixture results
- \`$LEVEL/*/logs.txt\`: Processor logs
- \`$LEVEL/*/output/\`: Processed output
- \`summary.md\`: This file

---
Generated by R08 Acceptance Harness
EOF

log "Created summary: $SUMMARY_FILE"
log ""
log "=== Run Complete ==="
log "Results: $OUTPUT_DIR"
log ""
cat "$SUMMARY_FILE"

# Exit with failure if any test failed
if [[ "$LEVEL" == "d0" ]]; then
    if [[ "$FIXTURE_PASSED" == "true" ]]; then
        exit 0
    else
        exit 1
    fi
fi
