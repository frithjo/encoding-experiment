#!/bin/bash
# Terminal-based UI/UX testing using backend API behavior
# This tests the actual behavior that the UI relies on

set -e

echo "Terminal-based UI/UX Behavior Testing"
echo "=========================================="
echo "Testing backend API behavior that the UI depends on"
echo ""

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
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/tools/call \
    -H "Content-Type: application/json" \
    -d '{"name": "batch_dla_scan", "arguments": {"prompt": ""}}')

if [ "$HTTP_CODE" = "400" ] || [ "$HTTP_CODE" = "422" ]; then
    echo "✓ PASS: Empty prompt returns error (HTTP $HTTP_CODE)"
else
    echo "✗ FAIL: Empty prompt should return error (got HTTP $HTTP_CODE)"
fi
echo ""

# Test 2: Valid prompt returns attention data
echo "Test 2: Valid prompt returns attention data"
echo "=========================================="
RESPONSE=$(curl -s -X POST http://localhost:8080/tools/call \
    -H "Content-Type: application/json" \
    -d '{"name": "batch_dla_scan", "arguments": {"prompt": "test prompt"}}')

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
RESPONSE=$(curl -s -X POST http://localhost:8080/tools/call \
    -H "Content-Type: application/json" \
    -d '{"name": "batch_dla_scan", "arguments": {"prompt": "test prompt"}}')

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
RESPONSE=$(curl -s -X POST http://localhost:8080/tools/call \
    -H "Content-Type: application/json" \
    -d '{"name": "batch_dla_scan", "arguments": {"prompt": "test prompt"}}')

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
RESPONSE=$(curl -s -X POST http://localhost:8080/tools/call \
    -H "Content-Type: application/json" \
    -d '{"name": "batch_dla_scan", "arguments": {"prompt": "test prompt"}}')

if echo "$RESPONSE" | grep -q "predictions"; then
    echo "✓ PASS: Response contains predictions array"
else
    echo "✗ FAIL: Response should contain predictions array"
    echo "Response: $RESPONSE"
fi
echo ""

echo "UI/UX behavior testing complete"
echo "Note: These tests validate the backend behavior that the UI depends on"
