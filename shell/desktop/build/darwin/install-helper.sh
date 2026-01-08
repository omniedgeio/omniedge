#!/bin/bash
#
# OmniEdge macOS Helper Installation Script
# This script installs the omniedge-helper launchd service
#
# Usage:
#   sudo ./install-helper.sh
#   sudo ./install-helper.sh --build  # Build from source if binary not found
#

set -e

HELPER_BIN="/usr/local/bin/omniedge-helper"
LAUNCHD_PLIST="/Library/LaunchDaemons/io.omniedge.helper.plist"
SOCKET_PATH="/var/run/omniedge-helper.sock"

# Check for root privileges
if [ "$(id -u)" != "0" ]; then
    echo "Error: This script must be run as root"
    exit 1
fi

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

echo "=== OmniEdge Helper Installation (macOS) ==="

# Stop existing service if running
if launchctl list | grep -q "io.omniedge.helper"; then
    echo "Stopping existing omniedge-helper service..."
    launchctl unload "$LAUNCHD_PLIST" 2>/dev/null || true
fi

# Remove old socket
rm -f "$SOCKET_PATH"

# Find or build helper binary
HELPER_FOUND=false
if [ -f "$SCRIPT_DIR/omniedge-helper" ]; then
    echo "Found helper binary in script directory"
    cp "$SCRIPT_DIR/omniedge-helper" "$HELPER_BIN"
    HELPER_FOUND=true
elif [ -f "$PROJECT_ROOT/io.omniedge.helper" ]; then
    echo "Found helper binary in project root"
    cp "$PROJECT_ROOT/io.omniedge.helper" "$HELPER_BIN"
    HELPER_FOUND=true
elif [ -f "$PROJECT_ROOT/bin/omniedge-helper" ]; then
    echo "Found helper binary in project bin directory"
    cp "$PROJECT_ROOT/bin/omniedge-helper" "$HELPER_BIN"
    HELPER_FOUND=true
elif [ "$1" = "--build" ] && [ -f "$PROJECT_ROOT/cmd/omniedge-helper/main.go" ]; then
    echo "Building omniedge-helper from source..."
    cd "$PROJECT_ROOT"
    CGO_ENABLED=1 go build -o "$HELPER_BIN" ./cmd/omniedge-helper
    HELPER_FOUND=true
fi

if [ "$HELPER_FOUND" = false ]; then
    echo ""
    echo "Warning: omniedge-helper binary not found!"
    echo "Build it with: cd $PROJECT_ROOT && go build -o $HELPER_BIN ./cmd/omniedge-helper"
    echo "Or run this script with: $0 --build"
    echo ""
fi

chmod 755 "$HELPER_BIN" 2>/dev/null || true
chown root:wheel "$HELPER_BIN" 2>/dev/null || true

# Install launchd plist
echo "Installing launchd plist..."
cp "$SCRIPT_DIR/io.omniedge.helper.plist" "$LAUNCHD_PLIST"
chown root:wheel "$LAUNCHD_PLIST"
chmod 644 "$LAUNCHD_PLIST"

# Create log directory
touch /var/log/omniedge-helper.log
chmod 644 /var/log/omniedge-helper.log

# Load the service
echo "Loading omniedge-helper service..."
launchctl load "$LAUNCHD_PLIST"

# Verify service is running
sleep 1
if launchctl list | grep -q "io.omniedge.helper"; then
    echo ""
    echo "=== Installation Complete ==="
    echo "omniedge-helper service is now running"
    echo ""
    echo "To check status: sudo launchctl list | grep omniedge"
    echo "To view logs:    tail -f /var/log/omniedge-helper.log"
else
    echo ""
    echo "Warning: Service may not have started. Check logs with:"
    echo "  tail -f /var/log/omniedge-helper.log"
fi
