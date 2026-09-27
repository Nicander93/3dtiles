#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Testing P0 Benchmark Framework ==="
echo ""

TEST_FAILED=0

test_json_schema_valid() {
    echo "Test 1: Verify benchmark-schema.json is valid JSON"
    if jq empty "${WORKSPACE_ROOT}/docs/acceptance/large-data/benchmark-schema.json" 2>/dev/null; then
        echo "✅ PASS: benchmark-schema.json is valid JSON"
    else
        echo "❌ FAIL: benchmark-schema.json is invalid JSON"
        TEST_FAILED=1
    fi
    echo ""
}

test_script_executable() {
    echo "Test 2: Verify benchmark script is executable"
    if [ -x "${WORKSPACE_ROOT}/scripts/benchmark-large-dataset.sh" ]; then
        echo "✅ PASS: benchmark-large-dataset.sh is executable"
    else
        echo "❌ FAIL: benchmark-large-dataset.sh is not executable"
        TEST_FAILED=1
    fi
    echo ""
}

test_script_help() {
    echo "Test 3: Verify benchmark script --help works"
    if "${WORKSPACE_ROOT}/scripts/benchmark-large-dataset.sh" --help > /dev/null 2>&1; then
        echo "✅ PASS: benchmark-large-dataset.sh --help works"
    else
        echo "❌ FAIL: benchmark-large-dataset.sh --help failed"
        TEST_FAILED=1
    fi
    echo ""
}

test_dry_run() {
    echo "Test 4: Verify benchmark script --dry-run works"
    
    mkdir -p /tmp/test-p0-benchmark/Data
    
    if "${WORKSPACE_ROOT}/scripts/benchmark-large-dataset.sh" \
        -i /tmp/test-p0-benchmark/Data \
        -s test-sample \
        --dry-run > /dev/null 2>&1; then
        echo "✅ PASS: benchmark-large-dataset.sh --dry-run works"
    else
        echo "❌ FAIL: benchmark-large-dataset.sh --dry-run failed"
        TEST_FAILED=1
    fi
    
    rm -rf /tmp/test-p0-benchmark
    echo ""
}

test_unmeasured_output() {
    echo "Test 5: Verify UNMEASURED result generation"
    
    OUTPUT_DIR="/tmp/test-p0-benchmark-output"
    mkdir -p /tmp/test-p0-sample/Data/Block001
    
    "${WORKSPACE_ROOT}/scripts/benchmark-large-dataset.sh" \
        -i /tmp/test-p0-sample/Data \
        -s test-sample \
        -o "$OUTPUT_DIR" \
        --skip-csv > /dev/null 2>&1 || true
    
    if [ -f "$OUTPUT_DIR/result.json" ]; then
        local status=$(jq -r '.results.status' "$OUTPUT_DIR/result.json" 2>/dev/null || echo "error")
        local note=$(jq -r '.results.notes' "$OUTPUT_DIR/result.json" 2>/dev/null || echo "")
        
        if [[ "$status" == "unmeasured" && "$note" == *"UNMEASURED"* ]]; then
            echo "✅ PASS: Result correctly marked as UNMEASURED"
        else
            echo "❌ FAIL: Result not properly marked as UNMEASURED"
            echo "   Status: $status"
            echo "   Note: $note"
            TEST_FAILED=1
        fi
    else
        echo "❌ FAIL: result.json not generated"
        TEST_FAILED=1
    fi
    
    rm -rf /tmp/test-p0-sample "$OUTPUT_DIR"
    echo ""
}

test_result_schema_compliance() {
    echo "Test 6: Verify result matches schema structure"
    
    OUTPUT_DIR="/tmp/test-p0-result-schema"
    mkdir -p /tmp/test-p0-schema/Data
    
    "${WORKSPACE_ROOT}/scripts/benchmark-large-dataset.sh" \
        -i /tmp/test-p0-schema/Data \
        -s test \
        -o "$OUTPUT_DIR" \
        --skip-csv > /dev/null 2>&1 || true
    
    if [ -f "$OUTPUT_DIR/result.json" ]; then
        local required_fields=(
            "benchmark_version"
            "timestamp"
            "environment"
            "codebase"
            "configuration"
            "sample"
            "results"
        )
        
        local all_present=true
        for field in "${required_fields[@]}"; do
            if ! jq -e ".$field" "$OUTPUT_DIR/result.json" > /dev/null 2>&1; then
                echo "❌ FAIL: Missing required field: $field"
                all_present=false
                TEST_FAILED=1
            fi
        done
        
        if [ "$all_present" = true ]; then
            echo "✅ PASS: All required schema fields present"
        fi
    else
        echo "❌ FAIL: result.json not generated"
        TEST_FAILED=1
    fi
    
    rm -rf /tmp/test-p0-schema "$OUTPUT_DIR"
    echo ""
}

test_gitignore_rules() {
    echo "Test 7: Verify .gitignore excludes benchmark outputs"
    
    cd "$WORKSPACE_ROOT"
    
    local should_ignore=(
        "benchmark-output-20260927/result.json"
        "benchmark-results/test.json"
        "docs/acceptance/large-data/results.csv"
    )
    
    local all_ignored=true
    for path in "${should_ignore[@]}"; do
        if ! git check-ignore -q "$path" 2>/dev/null; then
            echo "❌ FAIL: $path should be ignored but isn't"
            all_ignored=false
            TEST_FAILED=1
        fi
    done
    
    local should_not_ignore=(
        "docs/acceptance/large-data/README.md"
        "docs/acceptance/large-data/benchmark-schema.json"
        "docs/acceptance/large-data/samples/sample-template.json"
    )
    
    for path in "${should_not_ignore[@]}"; do
        if git check-ignore -q "$path" 2>/dev/null; then
            echo "❌ FAIL: $path should NOT be ignored but is"
            all_ignored=false
            TEST_FAILED=1
        fi
    done
    
    if [ "$all_ignored" = true ]; then
        echo "✅ PASS: .gitignore rules correct"
    fi
    echo ""
}

test_documentation_exists() {
    echo "Test 8: Verify documentation files exist"
    
    local docs=(
        "docs/acceptance/large-data/README.md"
        "docs/acceptance/large-data/baseline-version.md"
        "docs/acceptance/large-data/benchmark-schema.json"
        "docs/acceptance/large-data/csv-schema.md"
        "docs/acceptance/large-data/samples/README.md"
    )
    
    local all_exist=true
    for doc in "${docs[@]}"; do
        if [ ! -f "${WORKSPACE_ROOT}/$doc" ]; then
            echo "❌ FAIL: Missing documentation: $doc"
            all_exist=false
            TEST_FAILED=1
        fi
    done
    
    if [ "$all_exist" = true ]; then
        echo "✅ PASS: All documentation files exist"
    fi
    echo ""
}

echo "Running P0 framework tests..."
echo ""

test_json_schema_valid
test_script_executable
test_script_help
test_dry_run
test_unmeasured_output
test_result_schema_compliance
test_gitignore_rules
test_documentation_exists

echo "=== Test Summary ==="
if [ $TEST_FAILED -eq 0 ]; then
    echo "✅ ALL TESTS PASSED"
    exit 0
else
    echo "❌ SOME TESTS FAILED"
    exit 1
fi
