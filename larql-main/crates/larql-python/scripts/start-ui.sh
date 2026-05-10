#!/usr/bin/env bash
set -euo pipefail

HOST="${HOST:-127.0.0.1}"
PORT="${PORT:-8000}"
TERMINAL_FULLSCREEN="${TERMINAL_FULLSCREEN:-1}"
TERMINAL_HIDE_UI="${TERMINAL_HIDE_UI:-1}"
TERMINAL_CARBONYL_ARGS="${TERMINAL_CARBONYL_ARGS:-}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
REPO_RUNTIME_BIN="${REPO_ROOT}/apps/terminal-runtime/bin/carbonyl"

UI_PID=""
SHUTTING_DOWN=0

clear_port() {
  local pids
  pids="$(ss -ltnp "sport = :${PORT}" 2>/dev/null | awk -F 'pid=' 'NF>1 { split($2, a, ","); print a[1] }' | sort -u)"
  if [[ -n "${pids}" ]]; then
    echo "Clearing existing listener(s) on :${PORT}: ${pids}"
    while IFS= read -r pid; do
      [[ -z "${pid}" ]] && continue
      kill "${pid}" 2>/dev/null || true
    done <<< "${pids}"

    sleep 1

    local remaining
    remaining="$(ss -ltnp "sport = :${PORT}" 2>/dev/null | awk -F 'pid=' 'NF>1 { split($2, a, ","); print a[1] }' | sort -u)"
    if [[ -n "${remaining}" ]]; then
      echo "Force-killing remaining listener(s) on :${PORT}: ${remaining}"
      while IFS= read -r pid; do
        [[ -z "${pid}" ]] && continue
        kill -9 "${pid}" 2>/dev/null || true
      done <<< "${remaining}"
    fi
  fi
}

shutdown() {
  local exit_code=$?
  if [[ "${SHUTTING_DOWN}" -eq 1 ]]; then
    return "${exit_code}"
  fi
  SHUTTING_DOWN=1

  echo "Shutting down larql-ui..."
  if [[ -n "${UI_PID}" ]] && kill -0 "${UI_PID}" 2>/dev/null; then
    kill "${UI_PID}" 2>/dev/null || true
    wait "${UI_PID}" 2>/dev/null || true
  fi
  clear_port
  exit "${exit_code}"
}

trap shutdown INT TERM HUP EXIT

clear_port

echo "Starting larql-ui on http://${HOST}:${PORT}"
uv run --no-sync larql-ui --host "${HOST}" --port "${PORT}" &
UI_PID=$!
sleep 1

echo "Launching first-party terminal browser client..."
if [[ -x "${REPO_RUNTIME_BIN}" ]]; then
  export CARBONYL_BIN="${REPO_RUNTIME_BIN}"
fi
browser_args=()
if [[ "${TERMINAL_FULLSCREEN}" == "0" ]]; then
  browser_args+=("--no-fullscreen")
fi
if [[ "${TERMINAL_HIDE_UI}" == "0" ]]; then
  browser_args+=("--show-ui")
fi
if [[ -n "${TERMINAL_CARBONYL_ARGS}" ]]; then
  read -r -a extra_args <<< "${TERMINAL_CARBONYL_ARGS}"
  for arg in "${extra_args[@]}"; do
    browser_args+=("--carbonyl-arg=${arg}")
  done
fi
cargo run -q -p larql-terminal-browser -- "${browser_args[@]}" "http://${HOST}:${PORT}"
