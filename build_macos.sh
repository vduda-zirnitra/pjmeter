#!/usr/bin/env bash
set -euo pipefail

# Configuration (override with environment variables if needed)
PACKAGE_NAME="${PACKAGE_NAME:-pjmeter}"
BINARY_NAME="${BINARY_NAME:-pjmeter}"
MAINTAINER="${MAINTAINER:-Your Name <you@example.com>}"
DESCRIPTION="${DESCRIPTION:-PJMeter: Rust-based tool bundling Apache JMeter}"
RUST_BUILD_TARGET="${RUST_BUILD_TARGET:-x86_64-apple-darwin}"   # macOS target (Intel)
STRIP_BINARIES="${STRIP_BINARIES:-1}"        # 1 to strip binary if possible
CREATE_DMG="${CREATE_DMG:-1}"                # 1 to create DMG, 0 for tar.gz
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# Source JMeter's version
. "$SCRIPT_DIR/JMETER_VERSION.sh"

# Paths
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
JMX_SRC_DIR="${REPO_ROOT}/.jmeter-app/apache-jmeter-${JMETER_VERSION}"
BUILD_DIR="${REPO_ROOT}/build/macos"

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

# Build the Rust binary for macOS
echo "Building Rust binary (${BINARY_NAME}) for macOS target ${RUST_BUILD_TARGET}..."
if [[ -n "${RUST_BUILD_TARGET}" ]]; then
  # Add macOS target if not already added (only if cross-compiling)
  if [[ "$(uname -s)" != "Darwin" ]]; then
    rustup target add "${RUST_BUILD_TARGET}" 2>/dev/null || true
  fi
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

# Prepare staging directory
PKG_DIR="${BUILD_DIR}/${PACKAGE_NAME}_${CARGO_VERSION}"
if [[ "${CREATE_DMG}" == "1" ]]; then
  # For DMG, create pjmeter directory directly (will be copied to DMG)
  APP_DIR="${PKG_DIR}/pjmeter"
else
  # For tar.gz, use Applications/pjmeter structure
  APP_DIR="${PKG_DIR}/Applications/pjmeter"
fi

rm -rf "${PKG_DIR}"
mkdir -p "${APP_DIR}/bin"
mkdir -p "${APP_DIR}/jmeter"
mkdir -p "${APP_DIR}/jmeter_plugins"

# Copy binary
cp "${BIN_PATH}" "${APP_DIR}/bin/${BINARY_NAME}"
chmod 0755 "${APP_DIR}/bin/${BINARY_NAME}"

# Optionally strip binary to reduce size
if [[ "${STRIP_BINARIES}" == "1" ]]; then
  # Try target-specific strip first (useful when cross-compiling), then fallback to regular strip
  if command -v "${RUST_BUILD_TARGET}-strip" >/dev/null 2>&1; then
    "${RUST_BUILD_TARGET}-strip" "${APP_DIR}/bin/${BINARY_NAME}" 2>/dev/null || true
  elif command -v strip >/dev/null 2>&1; then
    strip "${APP_DIR}/bin/${BINARY_NAME}" 2>/dev/null || true
  fi
fi

# Copy JMeter distribution
if [[ -d "${JMX_SRC_DIR}" ]]; then
  cp -a "${JMX_SRC_DIR}"/* "${APP_DIR}/jmeter/"
else
  echo "ERROR: ${JMX_SRC_DIR} not found."
  exit 1
fi

if [[ "${CREATE_DMG}" == "1" ]]; then
  # Create DMG with drag-and-drop installation
  echo "Creating DMG installer..."
  
  OUT_DMG="${BUILD_DIR}/${PACKAGE_NAME}_${CARGO_VERSION}_macos_x86_64.dmg"
  mkdir -p "${BUILD_DIR}"
  
  # Use the DMG creation script
  if [[ -x "${REPO_ROOT}/installer/macos/create_dmg.sh" ]]; then
    "${REPO_ROOT}/installer/macos/create_dmg.sh" \
      "${APP_DIR}" \
      "${OUT_DMG}" \
      "PJMeter ${CARGO_VERSION}" \
      "${CARGO_VERSION}"
    
    if [[ -f "${OUT_DMG}" ]]; then
      echo "Done: ${OUT_DMG}"
      echo "Open the DMG and drag pjmeter to Applications folder"
      exit 0
    else
      echo "Warning: DMG creation failed. Falling back to tar.gz"
      CREATE_DMG="0"
    fi
  else
    echo "Warning: DMG creation script not found. Falling back to tar.gz"
    CREATE_DMG="0"
  fi
fi

# Fallback to tar.gz or if CREATE_DMG=0
# Need to restructure if we're in DMG mode
if [[ ! -d "${PKG_DIR}/Applications" ]]; then
  # Restructure to Applications/pjmeter format
  TMP_APP_DIR="${PKG_DIR}/Applications/pjmeter"
  mkdir -p "$(dirname "${TMP_APP_DIR}")"
  mv "${APP_DIR}" "${TMP_APP_DIR}"
  APP_DIR="${TMP_APP_DIR}"
fi

# Create README for installation
cat > "${PKG_DIR}/README.txt" << 'EOF'
PJMeter for macOS
=================

Installation:
1. Extract this archive

2. Copy the pjmeter directory to /Applications:
   sudo cp -r Applications/pjmeter /Applications/
   
3. Add pjmeter to your PATH by adding this line to your ~/.zshrc or ~/.bash_profile:
   export PATH="/Applications/pjmeter/bin:$PATH"
   
4. Create the global configuration directory:
   sudo mkdir -p "/Library/Application Support/pjmeter"
   sudo chmod 755 "/Library/Application Support/pjmeter"
   
5. Reload your shell configuration:
   source ~/.zshrc  # or source ~/.bash_profile
   
6. Run pjmeter:
   pjmeter --help

For more information, visit: https://github.com/vduda-zirnitra/pjmeter
EOF

# Create install script
cat > "${PKG_DIR}/install.sh" << 'EOF'
#!/usr/bin/env bash
set -e

echo "Installing pjmeter to /Applications/pjmeter..."
sudo cp -r Applications/pjmeter /Applications/

# Verify the binary was copied successfully
if [[ ! -f "/Applications/pjmeter/bin/pjmeter" ]]; then
    echo "Error: Binary not found at /Applications/pjmeter/bin/pjmeter"
    exit 1
fi

echo "Creating symlink in /usr/local/bin..."
sudo mkdir -p /usr/local/bin
sudo ln -sf /Applications/pjmeter/bin/pjmeter /usr/local/bin/pjmeter

echo "Creating global configuration directory..."
sudo mkdir -p "/Library/Application Support/pjmeter"
sudo chmod 755 "/Library/Application Support/pjmeter"

echo ""
echo "Installation complete!"
echo "You can now run 'pjmeter' from any terminal window."
EOF
chmod +x "${PKG_DIR}/install.sh"

# Create a tar.gz archive
OUT_TAR="${BUILD_DIR}/${PACKAGE_NAME}_${CARGO_VERSION}_macos_x86_64.tar.gz"
mkdir -p "${BUILD_DIR}"
echo "Creating ${OUT_TAR} ..."
cd "${BUILD_DIR}"
tar -czf "$(basename "${OUT_TAR}")" "$(basename "${PKG_DIR}")"
cd "${REPO_ROOT}"

echo "Done: ${OUT_TAR}"
echo "Install by extracting and running: sudo ./install.sh"
echo "Or manually copy Applications/pjmeter to /Applications/"
