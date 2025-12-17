#!/usr/bin/env bash
set -euo pipefail

# This script creates a DMG file for macOS with a drag-and-drop installation experience
# It also creates a symlink script to add pjmeter to /usr/local/bin

if [[ $# -lt 4 ]]; then
  echo "Usage: $0 <app_dir> <output_dmg> <volume_name> <version>"
  exit 1
fi

APP_DIR="$1"
OUTPUT_DMG="$2"
VOLUME_NAME="$3"
VERSION="$4"

# Create a temporary directory for DMG contents
TEMP_DIR="$(mktemp -d)"
trap "rm -rf '$TEMP_DIR'" EXIT

# Create the DMG structure
mkdir -p "${TEMP_DIR}/.background"
cp -R "${APP_DIR}" "${TEMP_DIR}/pjmeter"

# Create an Applications symlink for drag-and-drop
ln -s /Applications "${TEMP_DIR}/Applications"

# Create an install script that creates symlink to /usr/local/bin
cat > "${TEMP_DIR}/Install CLI Tool.command" << 'EOFSCRIPT'
#!/bin/bash
# This script creates a symlink to pjmeter in /usr/local/bin

# Get the directory where this script is located (the DMG mount point)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "Installing pjmeter CLI tool..."

# Create /usr/local/bin if it doesn't exist
if [ ! -d /usr/local/bin ]; then
    echo "Creating /usr/local/bin directory..."
    sudo mkdir -p /usr/local/bin
fi

# Create symlink
echo "Creating symlink in /usr/local/bin..."
sudo ln -sf "/Applications/pjmeter/bin/pjmeter" /usr/local/bin/pjmeter

# Create global config directory
echo "Creating global configuration directory..."
sudo mkdir -p "/Library/Application Support/pjmeter"
sudo chmod 755 "/Library/Application Support/pjmeter"

echo ""
echo "Installation complete!"
echo "You can now run 'pjmeter' from any terminal window."
echo ""
read -p "Press Enter to close this window..."
EOFSCRIPT

chmod +x "${TEMP_DIR}/Install CLI Tool.command"

# Create a README
cat > "${TEMP_DIR}/README.txt" << 'EOF'
PJMeter for macOS
=================

Installation:
1. Drag the 'pjmeter' folder to the 'Applications' folder
2. Double-click 'Install CLI Tool.command' to add pjmeter to your PATH
   (This will prompt for your password)

Alternative manual installation:
- After dragging to Applications, run in Terminal:
  sudo ln -sf /Applications/pjmeter/bin/pjmeter /usr/local/bin/pjmeter

For more information, visit: https://github.com/vduda-zirnitra/pjmeter
EOF

# Create the DMG
echo "Creating DMG: ${OUTPUT_DMG}"
if command -v hdiutil >/dev/null 2>&1; then
  # On macOS, use hdiutil
  hdiutil create -volname "${VOLUME_NAME}" \
    -srcfolder "${TEMP_DIR}" \
    -ov -format UDZO \
    "${OUTPUT_DMG}"
elif command -v genisoimage >/dev/null 2>&1; then
  # On Linux, use genisoimage (creates ISO, not DMG, but functional)
  echo "Warning: hdiutil not available. Creating ISO instead of DMG."
  OUTPUT_ISO="${OUTPUT_DMG%.dmg}.iso"
  genisoimage -V "${VOLUME_NAME}" -D -R -apple -no-pad \
    -o "${OUTPUT_ISO}" "${TEMP_DIR}"
  mv "${OUTPUT_ISO}" "${OUTPUT_DMG}"
else
  echo "Error: Neither hdiutil nor genisoimage found."
  echo "On Linux, install genisoimage: sudo apt-get install genisoimage"
  exit 1
fi

echo "DMG created successfully: ${OUTPUT_DMG}"
