#!/bin/bash
#
# OmniEdge Linux Helper Installation Script
# This script installs the omniedge-helper systemd service
#
# Usage:
#   sudo ./install-helper.sh
#   sudo ./install-helper.sh --build  # Build from source if binary not found
#

set -e

HELPER_BIN="/usr/local/bin/omniedge-helper"
SERVICE_FILE="/etc/systemd/system/omniedge-helper.service"
SOCKET_PATH="/var/run/omniedge-helper.sock"

# Check for root privileges
if [ "$(id -u)" != "0" ]; then
    echo "Error: This script must be run as root"
    exit 1
fi

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

echo "=== OmniEdge Helper Installation ==="

# Stop existing service if running
if systemctl is-active --quiet omniedge-helper 2>/dev/null; then
    echo "Stopping existing omniedge-helper service..."
    systemctl stop omniedge-helper
fi

# Find or build helper binary
HELPER_FOUND=false
if [ -f "$SCRIPT_DIR/omniedge-helper" ]; then
    echo "Found helper binary in script directory"
    cp "$SCRIPT_DIR/omniedge-helper" "$HELPER_BIN"
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

# Install systemd service file
echo "Installing systemd service file..."
cp "$SCRIPT_DIR/omniedge-helper.service" "$SERVICE_FILE"

# Reload systemd daemon
echo "Reloading systemd daemon..."
systemctl daemon-reload

# Enable and start the service
echo "Enabling omniedge-helper service..."
systemctl enable omniedge-helper

echo "Starting omniedge-helper service..."
systemctl start omniedge-helper || true

# Verify service is running
if systemctl is-active --quiet omniedge-helper; then
    echo ""
    echo "=== Installation Complete ==="
    echo "omniedge-helper service is now running"
    echo ""
    echo "To check status: systemctl status omniedge-helper"
    echo "To view logs:    journalctl -u omniedge-helper -f"
else
    echo ""
    echo "Warning: Service may not have started. Check logs with:"
    echo "  sudo journalctl -u omniedge-helper -xe"
fi
