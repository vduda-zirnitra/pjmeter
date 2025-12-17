#!/usr/bin/env bash
set -euo pipefail

# Configuration (override with environment variables if needed)
PACKAGE_NAME="${PACKAGE_NAME:-pjmeter}"
BINARY_NAME="${BINARY_NAME:-pjmeter}"
MAINTAINER="${MAINTAINER:-Your Name <you@example.com>}"
DESCRIPTION="${DESCRIPTION:-PJMeter: Rust-based tool bundling Apache JMeter}"
RUST_BUILD_TARGET="${RUST_BUILD_TARGET:-x86_64-pc-windows-gnu}"   # Windows target
STRIP_BINARIES="${STRIP_BINARIES:-1}"        # 1 to strip binary if possible
USE_INNO_SETUP="${USE_INNO_SETUP:-1}"        # 1 to create installer with Inno Setup, 0 for ZIP
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# Source JMeter's version
. "$SCRIPT_DIR/JMETER_VERSION.sh"

# Paths
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JMX_SRC_DIR="${REPO_ROOT}/.jmeter-app/apache-jmeter-${JMETER_VERSION}"
BUILD_DIR="${REPO_ROOT}/build/windows"

# Tools check
command -v cargo >/dev/null 2>&1 || { echo "cargo not found. Please install Rust toolchain."; exit 1; }

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

# Download JMeter if not available
if [[ ! -d "${JMX_SRC_DIR}" ]]; then
  echo "Downloading JMeter..."
  ./init_dev_environment.sh
fi

# Build the Rust binary for Windows
echo "Building Rust binary (${BINARY_NAME}) for Windows target ${RUST_BUILD_TARGET}..."
# Add Windows target if not already added
rustup target add "${RUST_BUILD_TARGET}" 2>/dev/null || true
./cargo_db.sh build --release --target "${RUST_BUILD_TARGET}"
BIN_PATH="${REPO_ROOT}/target/${RUST_BUILD_TARGET}/release/${BINARY_NAME}.exe"

if [[ ! -f "${BIN_PATH}" ]]; then
  echo "Binary not found at ${BIN_PATH}"
  exit 1
fi

# Prepare staging directory
PKG_DIR="${BUILD_DIR}/${PACKAGE_NAME}_${CARGO_VERSION}"
rm -rf "${PKG_DIR}"
mkdir -p "${PKG_DIR}/bin"
mkdir -p "${PKG_DIR}/jmeter"
mkdir -p "${PKG_DIR}/jmeter_plugins"

# Copy binary
cp "${BIN_PATH}" "${PKG_DIR}/bin/${BINARY_NAME}.exe"
chmod 0755 "${PKG_DIR}/bin/${BINARY_NAME}.exe"

# Optionally strip binary to reduce size
if [[ "${STRIP_BINARIES}" == "1" ]]; then
  # Try cross-compiler strip first, then fallback to regular strip
  if command -v "${RUST_BUILD_TARGET}-strip" >/dev/null 2>&1; then
    "${RUST_BUILD_TARGET}-strip" "${PKG_DIR}/bin/${BINARY_NAME}.exe" 2>/dev/null || true
  elif command -v strip >/dev/null 2>&1; then
    strip "${PKG_DIR}/bin/${BINARY_NAME}.exe" 2>/dev/null || true
  fi
fi

# Copy JMeter distribution
if [[ -d "${JMX_SRC_DIR}" ]]; then
  cp -a "${JMX_SRC_DIR}"/* "${PKG_DIR}/jmeter/"
else
  echo "ERROR: ${JMX_SRC_DIR} not found."
  exit 1
fi

if [[ "${USE_INNO_SETUP}" == "1" ]]; then
  # Create installer with Inno Setup
  echo "Creating Windows installer with Inno Setup..."
  
  # Check for Inno Setup
  ISCC=""
  if command -v iscc >/dev/null 2>&1; then
    ISCC="iscc"
  elif command -v wine >/dev/null 2>&1 && [[ -f "/usr/share/innosetup/ISCC.exe" ]]; then
    ISCC="wine /usr/share/innosetup/ISCC.exe"
  elif [[ -f "$HOME/.wine/drive_c/Program Files (x86)/Inno Setup 6/ISCC.exe" ]]; then
    ISCC="wine \"$HOME/.wine/drive_c/Program Files (x86)/Inno Setup 6/ISCC.exe\""
  else
    echo "Warning: Inno Setup not found. Falling back to ZIP archive."
    echo "To use Inno Setup on Linux, install via:"
    echo "  sudo apt-get install wine innoextract"
    echo "  Download Inno Setup from https://jrsoftware.org/isdl.php"
    USE_INNO_SETUP="0"
  fi
  
  if [[ "${USE_INNO_SETUP}" == "1" ]]; then
    # Export environment variables for Inno Setup script
    export SOURCE_DIR="${REPO_ROOT}"
    export CARGO_VERSION
    export JMETER_VERSION
    
    # Run Inno Setup
    mkdir -p "${BUILD_DIR}"
    eval $ISCC "${REPO_ROOT}/installer/windows/pjmeter.iss"
    
    OUT_INSTALLER="${BUILD_DIR}/pjmeter_${CARGO_VERSION}_windows_x86_64_installer.exe"
    if [[ -f "${OUT_INSTALLER}" ]]; then
      echo "Done: ${OUT_INSTALLER}"
      echo "Run the installer to install pjmeter on Windows"
      exit 0
    else
      echo "Error: Installer not created. Falling back to ZIP."
      USE_INNO_SETUP="0"
    fi
  fi
fi

# Fallback to ZIP or if USE_INNO_SETUP=0
# Create README for installation
cat > "${PKG_DIR}/README.txt" << 'EOF'
PJMeter for Windows
===================

Installation:
1. Extract this archive to C:\Program Files\pjmeter\
   (or any location of your choice)
   
2. Add C:\Program Files\pjmeter\bin to your PATH environment variable:
   - Right-click "This PC" -> Properties -> Advanced system settings
   - Click "Environment Variables"
   - Under "System variables", find "Path" and click "Edit"
   - Click "New" and add: C:\Program Files\pjmeter\bin
   - Click "OK" on all dialogs

3. Create the global configuration directory:
   - Create folder: C:\ProgramData\pjmeter
   
4. Run pjmeter from command prompt or PowerShell:
   pjmeter --help

For more information, visit: https://github.com/vduda-zirnitra/pjmeter
EOF

# Create a ZIP archive
if ! command -v zip >/dev/null 2>&1; then
  echo "Error: 'zip' command not found. Please install it:"
  echo "  - Ubuntu/Debian: sudo apt-get install zip"
  echo "  - macOS: brew install zip"
  exit 1
fi

OUT_ZIP="${BUILD_DIR}/${PACKAGE_NAME}_${CARGO_VERSION}_windows_x86_64.zip"
mkdir -p "${BUILD_DIR}"
echo "Creating ${OUT_ZIP} ..."
cd "${BUILD_DIR}"
zip -r "$(basename "${OUT_ZIP}")" "$(basename "${PKG_DIR}")"
cd "${REPO_ROOT}"

echo "Done: ${OUT_ZIP}"
echo "Install by extracting to C:\\Program Files\\pjmeter\\"
