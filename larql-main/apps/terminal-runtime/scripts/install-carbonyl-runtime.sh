#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BIN_DIR="${APP_ROOT}/bin"
TARGET_BIN="${BIN_DIR}/carbonyl"
TARGET_REAL="${BIN_DIR}/carbonyl.real"

# Usage:
#   ./install-carbonyl-runtime.sh [SOURCE]
# Where SOURCE is either:
#   - a Carbonyl binary path, or
#   - a Carbonyl runtime directory (contains carbonyl + libcarbonyl.so)
# If SOURCE is omitted:
#   1) use CARBONYL_BIN env var if set
#   2) resolve `carbonyl` on PATH

SOURCE_INPUT="${1:-${CARBONYL_BIN:-}}"
if [[ -z "${SOURCE_INPUT}" ]]; then
  if command -v carbonyl >/dev/null 2>&1; then
    SOURCE_INPUT="$(command -v carbonyl)"
  else
    echo "No Carbonyl runtime found. Provide source explicitly:"
    echo "  ./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh /absolute/path/to/carbonyl"
    echo "  ./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh /absolute/path/to/carbonyl-runtime-dir"
    exit 1
  fi
fi

if [[ ! -e "${SOURCE_INPUT}" ]]; then
  echo "Source does not exist: ${SOURCE_INPUT}"
  exit 1
fi

SOURCE_BIN=""
SOURCE_DIR=""
if [[ -d "${SOURCE_INPUT}" ]]; then
  SOURCE_DIR="${SOURCE_INPUT}"
  SOURCE_BIN="${SOURCE_DIR}/carbonyl"
else
  SOURCE_BIN="${SOURCE_INPUT}"
  SOURCE_DIR="$(cd "$(dirname "${SOURCE_BIN}")" && pwd)"
fi

if [[ ! -x "${SOURCE_BIN}" ]]; then
  echo "Source binary is not executable: ${SOURCE_BIN}"
  exit 1
fi
if [[ ! -f "${SOURCE_DIR}/libcarbonyl.so" ]]; then
  echo "Missing required runtime library: ${SOURCE_DIR}/libcarbonyl.so"
  exit 1
fi

mkdir -p "${BIN_DIR}"
cp "${SOURCE_BIN}" "${TARGET_REAL}"
chmod +x "${TARGET_REAL}"

# Cherry-pick only required runtime assets from the Carbonyl distribution.
for f in \
  libcarbonyl.so \
  icudtl.dat \
  v8_context_snapshot.bin \
  libEGL.so \
  libGLESv2.so \
  libvk_swiftshader.so \
  libvulkan.so.1 \
  vk_swiftshader_icd.json; do
  if [[ -f "${SOURCE_DIR}/${f}" ]]; then
    cp "${SOURCE_DIR}/${f}" "${BIN_DIR}/${f}"
  fi
done

if [[ -d "${SOURCE_DIR}/locales" ]]; then
  rm -rf "${BIN_DIR}/locales"
  cp -R "${SOURCE_DIR}/locales" "${BIN_DIR}/locales"
fi

cat > "${TARGET_BIN}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
BIN_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="${BIN_DIR}:${LD_LIBRARY_PATH:-}"
exec "${BIN_DIR}/carbonyl.real" "$@"
EOF
chmod +x "${TARGET_BIN}"

echo "Installed Carbonyl runtime into repo component:"
echo "  ${TARGET_BIN}"
echo "  ${TARGET_REAL}"
