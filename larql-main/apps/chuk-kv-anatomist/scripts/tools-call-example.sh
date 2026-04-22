#!/usr/bin/env bash
# Terminal parity with NativeSearchAdapter: POST JSON tool calls (same contract as the UI).
# Usage:
#   export NATIVE_SEARCH_URL=http://127.0.0.1:8766
#   ./scripts/tools-call-example.sh get_model_info '{}'
#   ./scripts/tools-call-example.sh context_map '{"prompt":"hi","layer":1,"top_k":5}'
set -euo pipefail
BASE="${NATIVE_SEARCH_URL:-${VITE_NATIVE_SEARCH_URL:-http://127.0.0.1:8766}}"
BASE="${BASE%/}"
TOOL="${1:?tool name (e.g. get_model_info)}"
ARGS_JSON="${2:-{}}"
BODY="$(node -e "const name=process.argv[1], raw=process.argv[2]; console.log(JSON.stringify({ name, arguments: JSON.parse(raw) }));" "$TOOL" "$ARGS_JSON")"
exec curl -sS -X POST "${BASE}/tools/call" \
  -H 'Content-Type: application/json' \
  -d "$BODY"
