#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

max_lines=999
failed=0

is_exempt_source_file() {
  local path="$1"
  local base
  base="$(basename "$path")"

  case "$path" in
    */rules_engine.rs|*/rules_engine_*.rs) return 0 ;;
    */registry/*.rs|*/registries/*.rs|*/ledger/*.rs|*/ledgers/*.rs) return 0 ;;
  esac

  case "$base" in
    *registry*.rs|*registries*.rs|*ledger*.rs|*ledgers*.rs) return 0 ;;
  esac

  return 1
}

while IFS= read -r -d '' path; do
  rel="${path#./}"
  if is_exempt_source_file "$rel"; then
    continue
  fi

  lines="$(wc -l <"$path")"
  if (( lines > max_lines )); then
    printf 'SOURCE_FILE_TOO_LONG %s lines=%s max=%s\n' "$rel" "$lines" "$max_lines" >&2
    failed=1
  fi
done < <(
  find \
    crates/larql-governance/src \
    crates/larql-cli/src/commands \
    crates/larql-cli/src/bin \
    crates/larql-policy-engine-projection/src \
    -type f -name '*.rs' -print0 2>/dev/null
)

exit "$failed"
