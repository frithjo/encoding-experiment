#!/bin/bash
# Terminal-based UI/UX testing using backend API behavior
# This tests the actual behavior that the UI relies on

set -e

echo "Terminal-based UI/UX Behavior Testing"
echo "=========================================="
echo "Testing backend API behavior that the UI depends on"
echo ""

STRICT_ANALYSIS_PAYLOAD='{"prompt":"The capital of Freedonia is","top_k":5,"mode":"fact_probe","truth_spans":["Markov"],"materially_false_spans":["Paris","London"],"coherence_markers":["capital","is"],"max_generated_tokens":1,"ridge_dead_zone":0.05}'

# Ensure larql-server is running
if ! curl -s http://localhost:8080/v1/health > /dev/null; then
    echo "✗ FAIL: larql-server is not running on port 8080"
    echo "Start it with: ./target/release/larql-server --port 8080 <vindex_path>"
    exit 1
fi

echo "✓ PASS: larql-server is running on port 8080"
echo ""

# Test 1: Empty prompt error handling
echo "Test 1: Empty prompt error handling"
echo "=========================================="
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/v1/analyze-infer \
    -H "Content-Type: application/json" \
    -d '{"prompt": "", "top_k": 5, "mode": "fact_probe", "truth_spans": [], "materially_false_spans": [], "coherence_markers": [], "max_generated_tokens": null, "ridge_dead_zone": null}')

if [ "$HTTP_CODE" = "400" ] || [ "$HTTP_CODE" = "422" ]; then
    echo "✓ PASS: Empty prompt returns error (HTTP $HTTP_CODE)"
else
    echo "✗ FAIL: Empty prompt should return error (got HTTP $HTTP_CODE)"
fi
echo ""

# Test 2: Valid prompt returns attention data
echo "Test 2: Valid prompt returns attention data"
echo "=========================================="
RESPONSE=$(curl -s -X POST http://localhost:8080/v1/analyze-infer \
    -H "Content-Type: application/json" \
    -d "$STRICT_ANALYSIS_PAYLOAD")

if echo "$RESPONSE" | grep -q "attention" && echo "$RESPONSE" | grep -q "num_layers"; then
    echo "✓ PASS: Valid prompt returns attention data with num_layers"
else
    echo "✗ FAIL: Valid prompt should return attention data"
    echo "Response: $RESPONSE"
fi
echo ""

# Test 3: Attention data has correct structure
echo "Test 3: Attention data structure validation"
echo "=========================================="
RESPONSE=$(curl -s -X POST http://localhost:8080/v1/analyze-infer \
    -H "Content-Type: application/json" \
    -d "$STRICT_ANALYSIS_PAYLOAD")

if echo "$RESPONSE" | grep -q "layer" && echo "$RESPONSE" | grep -q "heads"; then
    echo "✓ PASS: Attention data contains layer and heads fields"
else
    echo "✗ FAIL: Attention data should contain layer and heads fields"
    echo "Response: $RESPONSE"
fi
echo ""

# Test 4: Tokens are returned
echo "Test 4: Token data validation"
echo "=========================================="
RESPONSE=$(curl -s -X POST http://localhost:8080/v1/analyze-infer \
    -H "Content-Type: application/json" \
    -d "$STRICT_ANALYSIS_PAYLOAD")

if echo "$RESPONSE" | grep -q "tokens"; then
    echo "✓ PASS: Response contains tokens array"
else
    echo "✗ FAIL: Response should contain tokens array"
    echo "Response: $RESPONSE"
fi
echo ""

# Test 5: Predictions are returned
echo "Test 5: Predictions data validation"
echo "=========================================="
RESPONSE=$(curl -s -X POST http://localhost:8080/v1/analyze-infer \
    -H "Content-Type: application/json" \
    -d "$STRICT_ANALYSIS_PAYLOAD")

if echo "$RESPONSE" | grep -q "predictions" && \
   echo "$RESPONSE" | grep -q "analysis_summary" && \
   echo "$RESPONSE" | grep -q "ridge_by_layer" && \
   echo "$RESPONSE" | grep -q "token_analysis"; then
    echo "✓ PASS: Response contains full scientific analysis fields"
else
    echo "✗ FAIL: Response should contain full scientific analysis fields"
    echo "Response: $RESPONSE"
fi
echo ""

echo "UI/UX behavior testing complete"
echo "Note: These tests validate the backend behavior that the UI depends on"
