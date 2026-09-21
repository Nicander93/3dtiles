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
            
            # Run GE monotonicity check (Layer B)
            GE_RESULT=$("$PROCESSOR_PATH" check-ge --tileset "$FIXTURE_OUTPUT/output/tileset.json" 2>/dev/null || echo '{"checked": false, "passed": false, "violations": [], "total_tiles": 0}')
            GE_CHECKED=$(echo "$GE_RESULT" | jq -r '.checked')
            GE_PASSED=$(echo "$GE_RESULT" | jq -r '.passed')
            GE_VIOLATIONS=$(echo "$GE_RESULT" | jq -r '.violations | length')
            GE_TOTAL=$(echo "$GE_RESULT" | jq -r '.total_tiles')
        else
            OUTPUT_SIZE=0
            LAYER_A="FAIL"
            PASSED=false
            GE_CHECKED="false"
            GE_PASSED="false"
            GE_VIOLATIONS=0
            GE_TOTAL=0
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
  "spatialQuality": {
    "geMonotonicity": {
      "checked": $GE_CHECKED,
      "passed": $GE_PASSED,
      "violations": $GE_VIOLATIONS,
      "totalTiles": $GE_TOTAL,
      "note": "GE monotonicity check (R09.1)"
    },
    "geReasonableness": {
      "checked": false,
      "note": "Layer B not implemented (R09.2+)"
    },
    "bvTightness": {
      "checked": false,
      "note": "Layer B not implemented (R09.2+)"
    },
    "replaceCorrectness": {
      "checked": false,
      "note": "Layer B not implemented (R09.2+)"
    }
  },
  "cesiumAB": {
    "enabled": false,
    "status": "blocked",
    "blockedReason": "D0 too small for far-view HLOD A/B comparison; need D2 (LandsD/PlanD)",
    "note": "A/B blocked pending D2 data availability"
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
        log "Running D1 medium fixtures..."
        D1_DIR="$FIXTURES_DIR/d1-medium"
        D1_OUTPUT="$OUTPUT_DIR/d1-medium"
        mkdir -p "$D1_OUTPUT"
        
        # Run small-grid fixture
        FIXTURE="small-grid"
        FIXTURE_INPUT="$D1_DIR/$FIXTURE"
        FIXTURE_OUTPUT="$D1_OUTPUT/$FIXTURE"
        
        if [[ ! -d "$FIXTURE_INPUT" ]]; then
            error "D1 fixture not found: $FIXTURE_INPUT"
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
            
            # Run GE monotonicity check (Layer B)
            GE_RESULT=$("$PROCESSOR_PATH" check-ge --tileset "$FIXTURE_OUTPUT/output/tileset.json" 2>/dev/null || echo '{"checked": false, "passed": false, "violations": [], "total_tiles": 0}')
            GE_CHECKED=$(echo "$GE_RESULT" | jq -r '.checked')
            GE_PASSED=$(echo "$GE_RESULT" | jq -r '.passed')
            GE_VIOLATIONS=$(echo "$GE_RESULT" | jq -r '.violations | length')
            GE_TOTAL=$(echo "$GE_RESULT" | jq -r '.total_tiles')
            
            # Run frontier coverage check (Layer A+)
            if FRONTIER_RESULT=$("$PROCESSOR_PATH" check-frontier --tileset "$FIXTURE_OUTPUT/output/tileset.json" 2>/dev/null); then
                FRONTIER_CHECKED=$(echo "$FRONTIER_RESULT" | jq -r '.checked')
                FRONTIER_PASSED=$(echo "$FRONTIER_RESULT" | jq -r '.passed')
                FRONTIER_BLOCKS=$(echo "$FRONTIER_RESULT" | jq -r '.blocks_found')
                FRONTIER_GAPS=$(echo "$FRONTIER_RESULT" | jq -r '.gaps | length')
            else
                FRONTIER_CHECKED="false"
                FRONTIER_PASSED="false"
                FRONTIER_BLOCKS=0
                FRONTIER_GAPS=0
            fi
            
            # Run subtree retention check (Layer A+)
            if SUBTREE_RESULT=$("$PROCESSOR_PATH" check-subtree --input "$FIXTURE_INPUT/tileset.json" --output "$FIXTURE_OUTPUT/output/tileset.json" 2>/dev/null); then
                SUBTREE_CHECKED=$(echo "$SUBTREE_RESULT" | jq -r '.checked')
                SUBTREE_PASSED=$(echo "$SUBTREE_RESULT" | jq -r '.passed')
                SUBTREE_EXPECTED=$(echo "$SUBTREE_RESULT" | jq -r '.blocks_expected')
                SUBTREE_RETAINED=$(echo "$SUBTREE_RESULT" | jq -r '.blocks_retained')
                SUBTREE_LOST=$(echo "$SUBTREE_RESULT" | jq -r '.blocks_lost | length')
            else
                SUBTREE_CHECKED="false"
                SUBTREE_PASSED="false"
                SUBTREE_EXPECTED=0
                SUBTREE_RETAINED=0
                SUBTREE_LOST=0
            fi
        else
            OUTPUT_SIZE=0
            LAYER_A="FAIL"
            PASSED=false
            GE_CHECKED="false"
            GE_PASSED="false"
            GE_VIOLATIONS=0
            GE_TOTAL=0
            FRONTIER_CHECKED="false"
            FRONTIER_PASSED="false"
            FRONTIER_BLOCKS=0
            FRONTIER_GAPS=0
            SUBTREE_CHECKED="false"
            SUBTREE_PASSED="false"
            SUBTREE_EXPECTED=0
            SUBTREE_RETAINED=0
            SUBTREE_LOST=0
        fi
        
        # Create status.json for fixture
        cat > "$FIXTURE_OUTPUT/status.json" << EOF
{
  "fixture": "d1-medium/$FIXTURE",
  "passed": $PASSED,
  "exitCode": $EXIT_CODE,
  "duration": "${DURATION}s",
  "validation": {
    "layerA": "$LAYER_A",
    "errors": $ERRORS,
    "warnings": $WARNINGS
  },
  "spatialQuality": {
    "frontierCoverage": {
      "checked": $FRONTIER_CHECKED,
      "passed": $FRONTIER_PASSED,
      "gridSize": "2x2",
      "blocksExpected": 4,
      "blocksFound": $FRONTIER_BLOCKS,
      "gaps": $FRONTIER_GAPS,
      "note": "Frontier coverage check (R09.2)"
    },
    "subtreeRetention": {
      "checked": $SUBTREE_CHECKED,
      "passed": $SUBTREE_PASSED,
      "blocksExpected": $SUBTREE_EXPECTED,
      "blocksRetained": $SUBTREE_RETAINED,
      "blocksLost": $SUBTREE_LOST,
      "note": "Subtree retention check (R09.2)"
    },
    "transformConsistency": {
      "checked": false,
      "note": "Transform consistency check not yet implemented (R09.2+)"
    },
    "geMonotonicity": {
      "checked": $GE_CHECKED,
      "passed": $GE_PASSED,
      "violations": $GE_VIOLATIONS,
      "totalTiles": $GE_TOTAL,
      "note": "GE monotonicity check (R09.1)"
    },
    "geReasonableness": {
      "checked": false,
      "note": "Layer B not implemented (R09.2+)"
    },
    "bvTightness": {
      "checked": false,
      "note": "Layer B not implemented (R09.2+)"
    },
    "replaceCorrectness": {
      "checked": false,
      "note": "Layer B not implemented (R09.2+)"
    }
  },
  "cesiumAB": {
    "enabled": false,
    "status": "blocked",
    "blockedReason": "D1 too small for far-view HLOD A/B comparison; need D2 (LandsD/PlanD)",
    "note": "A/B blocked pending D2 data availability"
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
    FIXTURE_PASSED=$(echo "$FIXTURE_STATUS" | jq -r '.passed')
    
    if [[ "$FIXTURE_PASSED" == "true" ]]; then
        STATUS_ICON="✅"
        OVERALL_RESULT="PASS"
    else
        STATUS_ICON="❌"
        OVERALL_RESULT="FAIL"
    fi
    
    FIXTURE_DURATION=$(echo "$FIXTURE_STATUS" | jq -r '.duration')
    FIXTURE_ERRORS=$(echo "$FIXTURE_STATUS" | jq -r '.validation.errors')
    FIXTURE_WARNINGS=$(echo "$FIXTURE_STATUS" | jq -r '.validation.warnings')
    
    cat >> "$SUMMARY_FILE" << EOF
### D0 Tiny Fixtures

| Fixture | Status | Duration | Errors | Warnings |
|---------|--------|----------|--------|----------|
| single-tile | $STATUS_ICON | $FIXTURE_DURATION | $FIXTURE_ERRORS | $FIXTURE_WARNINGS |

**Overall**: $STATUS_ICON $OVERALL_RESULT

EOF
fi

if [[ "$LEVEL" == "d1" ]]; then
    FIXTURE_STATUS=$(cat "$OUTPUT_DIR/d1-medium/small-grid/status.json")
    FIXTURE_PASSED=$(echo "$FIXTURE_STATUS" | jq -r '.passed')
    
    if [[ "$FIXTURE_PASSED" == "true" ]]; then
        STATUS_ICON="✅"
        OVERALL_RESULT="PASS"
    else
        STATUS_ICON="❌"
        OVERALL_RESULT="FAIL"
    fi
    
    FIXTURE_DURATION=$(echo "$FIXTURE_STATUS" | jq -r '.duration')
    FIXTURE_ERRORS=$(echo "$FIXTURE_STATUS" | jq -r '.validation.errors')
    FIXTURE_WARNINGS=$(echo "$FIXTURE_STATUS" | jq -r '.validation.warnings')
    
    cat >> "$SUMMARY_FILE" << EOF
### D1 Medium Fixtures

| Fixture | Status | Duration | Errors | Warnings |
|---------|--------|----------|--------|----------|
| small-grid | $STATUS_ICON | $FIXTURE_DURATION | $FIXTURE_ERRORS | $FIXTURE_WARNINGS |

**Overall**: $STATUS_ICON $OVERALL_RESULT

**Note**: Spatial quality checks including GE monotonicity implemented (R09.1). Other Layer B checks pending R09.2+.

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

if [[ "$LEVEL" == "d1" ]]; then
    if [[ "$FIXTURE_PASSED" == "true" ]]; then
        exit 0
    else
        exit 1
    fi
fi
