#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
HELPER_DIR="$SCRIPT_DIR/../helper"
HELPER_NAME="io.omniedge.helper"
INSTALL_PATH="/Library/PrivilegedHelperTools/$HELPER_NAME"
PLIST_PATH="/Library/LaunchDaemons/$HELPER_NAME.plist"

echo "🔧 OmniEdge Helper Installer"
echo "============================"

# Check for root
if [ "$EUID" -ne 0 ]; then
    echo "❌ Please run with sudo"
    exit 1
fi

# Stop existing service if running
if launchctl list | grep -q "$HELPER_NAME"; then
    echo "⏹️  Stopping existing helper..."
    launchctl unload "$PLIST_PATH" 2>/dev/null || true
fi

# Build the helper
echo "🏗️  Building Unified Helper..."
# We build from the root repo to include all dependencies
cd "$REPO_ROOT"
GOWORK=off go build -o "$HELPER_NAME" ./cmd/omniedge-helper

# Install helper binary
echo "📦 Installing helper to $INSTALL_PATH..."
mkdir -p /Library/PrivilegedHelperTools
cp "$HELPER_NAME" "$INSTALL_PATH"
chmod 755 "$INSTALL_PATH"
chown root:wheel "$INSTALL_PATH"

# Sign the helper binary for macOS
if [ -f "$INSTALL_PATH" ]; then
    echo "✍️  Signing helper binary..."
    codesign -s - --force "$INSTALL_PATH" 2>/dev/null && echo "✅ Helper binary signed" || echo "⚠️  Could not sign helper binary"
fi

# Clean up old edge binary if exists (we don't need it anymore)
rm -f /Library/PrivilegedHelperTools/edge

# Install launchd plist
echo "📋 Installing launchd plist..."
cp "$HELPER_DIR/$HELPER_NAME.plist" "$PLIST_PATH"
chmod 644 "$PLIST_PATH"
chown root:wheel "$PLIST_PATH"

# Load the service
echo "🚀 Loading helper service..."
launchctl load "$PLIST_PATH"

# Verify
sleep 1
if launchctl list | grep -q "$HELPER_NAME"; then
    echo "✅ Helper installed and running!"
    echo ""
    echo "📝 Logs: /var/log/omniedge-helper.log"
    echo "🔌 Socket: /var/run/omniedge-helper.sock"
else
    echo "❌ Failed to start helper. Check /var/log/omniedge-helper.log"
    exit 1
fi
