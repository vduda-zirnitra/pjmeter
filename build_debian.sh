#!/usr/bin/env bash
set -euo pipefail

# Configuration (override with environment variables if needed)
PACKAGE_NAME="${PACKAGE_NAME:-pjmeter}"
BINARY_NAME="${BINARY_NAME:-pjmeter}"
MAINTAINER="${MAINTAINER:-Your Name <you@example.com>}"
SECTION="${SECTION:-utils}"
PRIORITY="${PRIORITY:-optional}"
DESCRIPTION="${DESCRIPTION:-PJMeter: Rust-based tool bundling Apache JMeter}"
DEB_REVISION="${DEB_REVISION:-1}"            # Debian revision suffix after version, e.g. 1 → x.y.z-1
DEPENDS="${DEPENDS:-}"                       # e.g., "libc6 (>= 2.31)"; leave empty for none
RUST_BUILD_TARGET="${RUST_BUILD_TARGET:-}"   # e.g., "x86_64-unknown-linux-musl" for static builds
STRIP_BINARIES="${STRIP_BINARIES:-1}"        # 1 to strip binary if possible
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# Source JMeter's version
. "$SCRIPT_DIR/JMETER_VERSION.sh"

# Paths
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JMX_SRC_DIR="${REPO_ROOT}/.jmeter-app/apache-jmeter-${JMETER_VERSION}"
POSTINST_SRC="${REPO_ROOT}/debian_package/debian/postinst"
BUILD_DIR="${REPO_ROOT}/build/deb"

# Tools check
command -v cargo >/dev/null 2>&1 || { echo "cargo not found. Please install Rust toolchain."; exit 1; }
command -v dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb not found. Run on Debian/Ubuntu or install dpkg."; exit 1; }

# Determine version from Cargo.toml
if [[ -f "${REPO_ROOT}/Cargo.toml" ]]; then
  CARGO_VERSION="$(awk -F ' *= *\"' '/^version *= *\"/ {print $2}' "${REPO_ROOT}/Cargo.toml" | sed 's/\".*//')"
else
  echo "Cargo.toml not found in ${REPO_ROOT}"
  exit 1
fi
if [[ -z "${CARGO_VERSION}" ]]; then
  echo "Could not determine version from Cargo.toml"
  exit 1
fi
DEB_VERSION="${CARGO_VERSION}-${DEB_REVISION}"

# Download JMeter if not available
if [[ ! -d "${JMX_SRC_DIR}" ]]; then
  echo "Downloading JMeter..."
  ./init_dev_environment.sh
fi

# Build the Rust binary
echo "Building Rust binary (${BINARY_NAME}) in release mode..."
if [[ -n "${RUST_BUILD_TARGET}" ]]; then
  ./cargo_db.sh build --release --target "${RUST_BUILD_TARGET}"
  BIN_PATH="${REPO_ROOT}/target/${RUST_BUILD_TARGET}/release/${BINARY_NAME}"
else
  ./cargo_db.sh build --release
  BIN_PATH="${REPO_ROOT}/target/release/${BINARY_NAME}"
fi

if [[ ! -f "${BIN_PATH}" ]]; then
  echo "Binary not found at ${BIN_PATH}"
  exit 1
fi

# Determine Debian architecture
if command -v dpkg >/dev/null 2>&1; then
  ARCH="$(dpkg --print-architecture)"
else
  # Fallback using uname mapping (simplistic)
  case "$(uname -m)" in
    x86_64) ARCH="amd64" ;;
    aarch64) ARCH="arm64" ;;
    armv7l) ARCH="armhf" ;;
    i386|i686) ARCH="i386" ;;
    *) echo "Unknown architecture $(uname -m). Set ARCH env var."; exit 1 ;;
  esac
fi

# Prepare staging directory
PKG_DIR="${BUILD_DIR}/${PACKAGE_NAME}_${DEB_VERSION}"
rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/DEBIAN"
mkdir -p "${PKG_DIR}/opt/pjmeter/bin"
mkdir -p "${PKG_DIR}/opt/pjmeter/jmeter_plugins"
mkdir -p "${PKG_DIR}/var/opt/pjmeter"

# Copy binary
install -m 0755 "${BIN_PATH}" "${PKG_DIR}/opt/pjmeter/bin/${BINARY_NAME}"

# Optionally strip binary to reduce size
if [[ "${STRIP_BINARIES}" == "1" ]] && command -v strip >/dev/null 2>&1; then
  strip -s "${PKG_DIR}/opt/pjmeter/bin/${BINARY_NAME}" || true
fi

# Copy JMeter distribution (must exist in repo at debian_package/opt/pjmeter/jmeter)
if [[ -d "${JMX_SRC_DIR}" ]]; then
  mkdir -p "${PKG_DIR}/opt/pjmeter"
  cp -a "${JMX_SRC_DIR}" "${PKG_DIR}/opt/pjmeter/jmeter"
else
  echo "ERROR: ${JMX_SRC_DIR} not found."
  exit 1
fi

# Install postinst maintainer script if present
if [[ -f "${POSTINST_SRC}" ]]; then
  install -m 0755 "${POSTINST_SRC}" "${PKG_DIR}/DEBIAN/postinst"
else
  echo "NOTE: ${POSTINST_SRC} not found. No postinst script will be included."
fi

# Generate control file
CONTROL_FILE="${PKG_DIR}/DEBIAN/control"
{
  echo "Package: ${PACKAGE_NAME}"
  echo "Version: ${DEB_VERSION}"
  echo "Section: ${SECTION}"
  echo "Priority: ${PRIORITY}"
  echo "Architecture: ${ARCH}"
  if [[ -n "${DEPENDS}" ]]; then
    echo "Depends: ${DEPENDS}"
  fi
  echo "Maintainer: ${MAINTAINER}"
  echo "Description: ${DESCRIPTION}"
} > "${CONTROL_FILE}"

# md5sums (optional but nice to have)
pushd "${PKG_DIR}" >/dev/null
if command -v md5sum >/dev/null 2>&1; then
  if [[ -d opt ]]; then
    find opt -type f -print0 | xargs -0 md5sum > DEBIAN/md5sums
  fi
fi
popd >/dev/null

# Build the .deb
OUT_DEB="${BUILD_DIR}/${PACKAGE_NAME}_${DEB_VERSION}_${ARCH}.deb"
mkdir -p "${BUILD_DIR}"
echo "Building ${OUT_DEB} ..."
if command -v fakeroot >/dev/null 2>&1; then
  fakeroot dpkg-deb --build "${PKG_DIR}" "${OUT_DEB}"
else
  echo "fakeroot not found; building without it (ownership may not be recorded as root)."
  dpkg-deb --build "${PKG_DIR}" "${OUT_DEB}"
fi

echo "Done: ${OUT_DEB}"
echo "Install with: sudo dpkg -i \"${OUT_DEB}\""