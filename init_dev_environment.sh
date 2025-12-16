#!/usr/bin/env bash
set -euo pipefail

# Simple script to download a fixed Apache JMeter version (.zip) from:
#   https://archive.apache.org/dist/jmeter/binaries/
# and extract it to:
#   .jmeter-app/   (at the same level as this script)
#
# Requirements: curl and unzip

# Resolve project root (where this script lives) and destination dir
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# Source JMeter's version
. "$SCRIPT_DIR/JMETER_VERSION.sh"
BASE_URL="https://archive.apache.org/dist/jmeter/binaries"
ZIP_NAME="apache-jmeter-${JMETER_VERSION}.zip"
URL="${BASE_URL}/${ZIP_NAME}"
DEST_DIR="${SCRIPT_DIR}/.jmeter-app"
TARGET_DIR="${DEST_DIR}/apache-jmeter-${JMETER_VERSION}"
SANDBOX_DIR="${SCRIPT_DIR}/.sandbox"
DB_DIR="${SANDBOX_DIR}/db"
JMETER_PLUGINS_PATH="${SANDBOX_DIR}/jmeter_plugins"
LINK_PATH="${SANDBOX_DIR}/jmeter"
LINK_TARGET_REL="../.jmeter-app/apache-jmeter-${JMETER_VERSION}"


# Ensure prerequisites
command -v curl >/dev/null 2>&1 || { echo "Error: 'curl' is required."; exit 1; }
command -v unzip >/dev/null 2>&1 || { echo "Error: 'unzip' is required."; exit 1; }

mkdir -p "${DEST_DIR}"

# Skip if already present
if [[ -d "${TARGET_DIR}" ]]; then
  echo "JMeter ${JMETER_VERSION} already present at: ${TARGET_DIR}"
  exit 0
fi

# Temporary download directory
TMPDIR="$(mktemp -d)"
cleanup() { rm -rf "${TMPDIR}"; }
trap cleanup EXIT

ZIP_PATH="${TMPDIR}/${ZIP_NAME}"

echo "Downloading ${ZIP_NAME} from ${URL} ..."
curl -fL -o "${ZIP_PATH}" "${URL}"

echo "Extracting to ${DEST_DIR} ..."
unzip -q "${ZIP_PATH}" -d "${DEST_DIR}"

# Delete the downloaded .zip explicitly
rm -f "${ZIP_PATH}"

echo "Done."
echo "JMeter installed at: ${TARGET_DIR}"

mkdir -p "${DB_DIR}"
mkdir -p "${JMETER_PLUGINS_PATH}"
ln -sfn "${LINK_TARGET_REL}" "${LINK_PATH}"
echo "${SANDBOX_DIR} initialized"