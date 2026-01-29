#!/bin/sh

set -e

# OmniEdge CLI Install Script
# Usage: curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh | bash
# Manual version: curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh | OMNIEDGE_VERSION=v2.0.0 bash

REPO="omniedgeio/omniedge"
PKG_NAME="omniedge-cli"
BIN_DIR="/usr/local/bin"
DEFAULT_VERSION="latest"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

setup_env() {
    SUDO=sudo
    if [ $(id -u) -eq 0 ]; then
        SUDO=
    fi
}

# --- get latest version from GitHub ---
get_latest_version() {
    # If OMNIEDGE_VERSION is set in environment, use it
    if [ -n "$OMNIEDGE_VERSION" ]; then
        VERSION="$OMNIEDGE_VERSION"
        info "Using provided version: ${VERSION}"
        return
    fi

    info "Checking for latest version..."
    
    # Try fetching via redirect URL (more reliable without API limits)
    if command -v curl >/dev/null 2>&1; then
        VERSION_URL=$(curl -Ls -o /dev/null -w %{url_effective} "https://github.com/${REPO}/releases/latest")
        VERSION=$(echo "$VERSION_URL" | sed 's:.*/tag/::')
    fi

    # Fallback to API if redirect failed or returned empty/wrong content
    if [ -z "$VERSION" ] || [ "$VERSION" = "latest" ]; then
        if command -v curl >/dev/null 2>&1; then
            VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
        elif command -v wget >/dev/null 2>&1; then
            VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
        fi
    fi
    
    if [ -z "$VERSION" ] || [ "$VERSION" = "latest" ]; then
        fatal "Failed to get latest version from GitHub."
    else
        info "Latest version: ${VERSION}"
    fi
}

download_and_verify() {
    get_latest_version
    setup_verify_arch
    verify_downloader curl || verify_downloader wget || fatal 'Cannot find curl or wget for downloading files'
    setup_tmp
    download_binary
    setup_binary
}

output_usage(){
    echo ""
    echo "${GREEN}OmniEdge CLI installed successfully!${NC}"
    echo ""
    echo "Usage:"
    echo "  ${YELLOW}omniedge login -u your@email.com${NC}    # Login with email"
    echo "  ${YELLOW}omniedge login -s YOUR_SECRET_KEY${NC}   # Login with API key"
    echo "  ${YELLOW}sudo omniedge start -n <network_id>${NC} # Start VPN connection"
    echo "  ${YELLOW}omniedge status${NC}                     # Check connection status"
    echo ""
    echo "Documentation: https://omniedge.io/docs"
    echo ""
}

# --- create temporary directory and cleanup when done ---
setup_tmp() {
    TMP_DIR=$(mktemp -d -t omniedge-install.XXXXXXXXXX)
    TMP_ARCHIVE=${TMP_DIR}/omniedge.tar.gz
    TMP_BIN=${TMP_DIR}/omniedge.bin
    cleanup() {
        code=$?
        set +e
        trap - EXIT
        rm -rf ${TMP_DIR}
        exit $code
    }
    trap cleanup INT EXIT
}

# --- download binary from github url ---
download_binary() {
    OS=$(uname)
    BIN_URL=""
    # Remove 'v' prefix from VERSION for artifact naming (workflow uses version without 'v')
    VERSION_NUM="${VERSION#v}"
    
    if [ "$OS" = "Darwin" ]; then
        # macOS - both x64 and arm64 supported
        BIN_URL="https://github.com/${REPO}/releases/download/${VERSION}/${PKG_NAME}-${VERSION_NUM}-macos-${SUFFIX}.tar.gz"
    else
        # Linux
        BIN_URL="https://github.com/${REPO}/releases/download/${VERSION}/${PKG_NAME}-${VERSION_NUM}-linux-${SUFFIX}.tar.gz"
    fi
    
    info "Downloading ${BIN_URL}"
    download ${TMP_ARCHIVE} ${BIN_URL}
}

# --- setup permissions and move binary to system directory ---
setup_binary() {
    info "Extracting omniedge..."
    mkdir -p ${TMP_BIN}
    tar -xzf ${TMP_ARCHIVE} -C ${TMP_BIN}
    
    # Find the omniedge binary (the extracted binary has a versioned name like omniedge-cli-2.0.0-linux-x64)
    OMNIEDGE_BIN=$(find ${TMP_BIN} -type f -perm -111 | head -1)
    
    if [ -z "$OMNIEDGE_BIN" ]; then
        # Try finding by name pattern
        OMNIEDGE_BIN=$(find ${TMP_BIN} -name "omniedge-cli-*" -type f | head -1)
    fi
    
    if [ -z "$OMNIEDGE_BIN" ]; then
        fatal "Failed to find omniedge binary in archive"
    fi
    
    info "Installing omniedge to ${BIN_DIR}/omniedge"
    chmod 755 ${OMNIEDGE_BIN}
    $SUDO mv -f ${OMNIEDGE_BIN} ${BIN_DIR}/omniedge
}

# --- set arch and suffix, fatal if architecture not supported ---
setup_verify_arch() {
    if [ -z "$ARCH" ]; then
        ARCH=$(uname -m)
    fi
    case $ARCH in
    amd64|x86_64)
        ARCH=amd64
        SUFFIX=x64
        ;;
    arm64|aarch64|armv8*)
        ARCH=arm64
        SUFFIX=arm64
        ;;
    arm*|armv7l)
        ARCH=arm
        SUFFIX=armv7
        ;;
    riscv64)
        ARCH=riscv64
        SUFFIX=riscv64
        ;;
    *)
        fatal "Unsupported architecture: $ARCH. Supported: x86_64, arm64, armv7, riscv64"
        ;;
    esac
    info "Detected architecture: $ARCH"
}

# --- verify existence of network downloader executable ---
verify_downloader() {
    [ -x "$(command -v $1)" ] || return 1
    DOWNLOADER=$1
    return 0
}

# --- download from github url ---
download() {
    [ $# -eq 2 ] || fatal 'download needs exactly 2 arguments'

    case $DOWNLOADER in
    curl)
        curl -o $1 -sfL $2
        ;;
    wget)
        wget -qO $1 $2
        ;;
    *)
        fatal "Incorrect executable '$DOWNLOADER'"
        ;;
    esac

    # Abort if download command failed
    [ $? -eq 0 ] || fatal 'Download failed'
}

# --- helper functions for logs ---
info() {
    echo -e "${GREEN}[INFO]${NC} $@"
}
warn() {
    echo -e "${YELLOW}[WARN]${NC} $@" >&2
}
fatal() {
    echo -e "${RED}[ERROR]${NC} $@" >&2
    exit 1
}

# --- main ---
{
    echo ""
    echo "╔═══════════════════════════════════════════════════════════╗"
    echo "║             OmniEdge CLI Installer                        ║"
    echo "║        Secure P2P Mesh Networking for IoT/AI              ║"
    echo "╚═══════════════════════════════════════════════════════════╝"
    echo ""
    
    setup_env
    download_and_verify
    output_usage
}
