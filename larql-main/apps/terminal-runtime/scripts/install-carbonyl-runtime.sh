#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
BIN_DIR="${APP_ROOT}/bin"
TARGET_BIN="${BIN_DIR}/carbonyl"
TARGET_REAL="${BIN_DIR}/carbonyl.real"
VERSION_FILE="${APP_ROOT}/CARBONYL_VERSION"

# Pinned Carbonyl release download URL (Linux x86_64)
# Update VERSION_FILE to change version/SHA256
GITHUB_RELEASE_BASE="https://github.com/fathyb/carbonyl/releases/download"

# Read pinned version and SHA256
if [[ ! -f "${VERSION_FILE}" ]]; then
  echo "Error: CARBONYL_VERSION file not found at: ${VERSION_FILE}"
  exit 1
fi

# Parse VERSION_FILE (format: VERSION<TAB>SHA256)
# Skip comment lines and empty lines
PINNED_VERSION=$(grep -v '^#' "${VERSION_FILE}" | grep -v '^$' | cut -f1)
PINNED_SHA256=$(grep -v '^#' "${VERSION_FILE}" | grep -v '^$' | cut -f2)

if [[ "${PINNED_SHA256}" == "PLACEHOLDER_SHA256_REPLACE_WITH_ACTUAL_VALUE" ]]; then
  echo "Error: CARBONYL_VERSION contains placeholder SHA256."
  echo "Please update ${VERSION_FILE} with the actual SHA256 from the release."
  exit 1
fi

# Usage:
#   ./install-carbonyl-runtime.sh [SOURCE]
# Where SOURCE is either:
#   - a Carbonyl binary path, or
#   - a Carbonyl runtime directory (contains carbonyl + libcarbonyl.so)
# If SOURCE is omitted:
#   1) use CARBONYL_BIN env var if set
#   2) resolve `carbonyl` on PATH
#   3) download pinned version from GitHub releases (Linux x86_64 only)

SOURCE_INPUT="${1:-${CARBONYL_BIN:-}}"
DOWNLOAD_MODE=false

if [[ -z "${SOURCE_INPUT}" ]]; then
  if command -v carbonyl >/dev/null 2>&1; then
    SOURCE_INPUT="$(command -v carbonyl)"
    # Check if the found carbonyl has the required runtime library
    SOURCE_DIR="$(cd "$(dirname "${SOURCE_INPUT}")" && pwd)"
    if [[ ! -f "${SOURCE_DIR}/libcarbonyl.so" ]]; then
      # Found carbonyl but missing runtime library - switch to download mode
      echo "Found carbonyl on PATH but missing required runtime library."
      echo "Switching to download mode."
      DOWNLOAD_MODE=true
      SOURCE_INPUT=""
    fi
  else
    # No local carbonyl found - switch to download mode
    DOWNLOAD_MODE=true
  fi
fi

# Download mode: fetch pinned release from GitHub
if [[ "${DOWNLOAD_MODE}" == true ]]; then
  echo "No local Carbonyl found. Downloading pinned version ${PINNED_VERSION}..."
  echo "Platform: Linux x86_64"
  
  # Create temp directory for download
  TMPDIR=$(mktemp -d)
  trap "rm -rf ${TMPDIR}" EXIT
  
  RELEASE_URL="${GITHUB_RELEASE_BASE}/v${PINNED_VERSION}/carbonyl.linux-amd64.zip"
  ZIPFILE="${TMPDIR}/carbonyl.zip"

  # Download
  echo "Downloading from ${RELEASE_URL}..."
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL -o "${ZIPFILE}" "${RELEASE_URL}"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "${ZIPFILE}" "${RELEASE_URL}"
  else
    echo "Error: Neither curl nor wget found. Please install one of them."
    exit 1
  fi

  # Verify SHA256
  echo "Verifying SHA256..."
  ACTUAL_SHA256=$(sha256sum "${ZIPFILE}" | cut -d' ' -f1)
  if [[ "${ACTUAL_SHA256}" != "${PINNED_SHA256}" ]]; then
    echo "Error: SHA256 mismatch!"
    echo "  Expected: ${PINNED_SHA256}"
    echo "  Actual:   ${ACTUAL_SHA256}"
    echo ""
    echo "The download may be corrupted or the version pin is outdated."
    echo "Update ${VERSION_FILE} if this is a new version."
    exit 1
  fi
  echo "SHA256 verified: ${ACTUAL_SHA256}"

  # Extract
  echo "Extracting..."
  unzip -q "${ZIPFILE}" -d "${TMPDIR}"

  # Find the extracted directory (usually carbonyl-<version>)
  EXTRACTED_DIR=$(find "${TMPDIR}" -maxdepth 1 -type d -name "carbonyl-*" | head -n1)
  if [[ -z "${EXTRACTED_DIR}" ]]; then
    echo "Error: Could not find extracted Carbonyl directory"
    exit 1
  fi
  
  # Set SOURCE_INPUT to the extracted directory for the rest of the script
  SOURCE_INPUT="${EXTRACTED_DIR}"
  echo "Downloaded and extracted to: ${SOURCE_INPUT}"
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
