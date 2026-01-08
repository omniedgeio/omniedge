#!/bin/sh

set -e

# OmniEdge CLI Install Script
# Usage: curl https://connect.omniedge.io/install/omniedge-install.sh | bash
# Manual version: curl https://connect.omniedge.io/install/omniedge-install.sh | OMNIEDGE_VERSION=v1.0.0 bash

REPO="omniedgeio/omniedge"
PKG_NAME="omniedge"
BIN_DIR="/usr/local/bin"
DEFAULT_VERSION="v1.0.0"

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

# --- get latest version from GitHub API ---
get_latest_version() {
    # If OMNIEDGE_VERSION is set in environment, use it
    if [ -n "$OMNIEDGE_VERSION" ]; then
        VERSION="$OMNIEDGE_VERSION"
        info "Using provided version: ${VERSION}"
        return
    fi

    info "Checking for latest version..."
    if command -v curl >/dev/null 2>&1; then
        VERSION=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    elif command -v wget >/dev/null 2>&1; then
        VERSION=$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    fi
    
    if [ -z "$VERSION" ]; then
        VERSION="$DEFAULT_VERSION"
        warn "Failed to get latest version from GitHub API, falling back to ${VERSION}"
    else
        info "Latest version: ${VERSION}"
    fi
}

download_and_verify() {
    get_latest_version
    setup_verify_arch
    verify_downloader curl || verify_downloader wget || fatal 'Cannot find curl or wget for downloading files'
    verify_unzip unzip || fatal 'Cannot find unzip'
    setup_tmp
    download_binary
    setup_binary
}

output_usage(){
    echo ""
    echo "${GREEN}✓ OmniEdge CLI installed successfully!${NC}"
    echo ""
    echo "Usage:"
    echo "  ${YELLOW}omniedge login -u your@email.com${NC}    # Login with email"
    echo "  ${YELLOW}omniedge login -s YOUR_SECRET_KEY${NC}   # Login with API key"
    echo "  ${YELLOW}sudo omniedge join${NC}                  # Join a network"
    echo ""
    echo "Documentation: https://connect.omniedge.io/docs"
    echo ""
}

# --- create temporary directory and cleanup when done ---
setup_tmp() {
    TMP_DIR=$(mktemp -d -t omniedge-install.XXXXXXXXXX)
    TMP_ZIP=${TMP_DIR}/omniedge.zip
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
    
    if [ "$OS" = "Darwin" ]; then
        # macOS - arm64 only (Apple Silicon)
        if [ "$ARCH" != "arm64" ]; then
            fatal "macOS amd64 is not supported. Please use an Apple Silicon Mac (M1/M2/M3/M4)."
        fi
        BIN_URL="https://github.com/${REPO}/releases/download/${VERSION}/${PKG_NAME}-${VERSION}-macos-arm64.zip"
    elif [ "$OS" = "FreeBSD" ]; then
        BIN_URL="https://github.com/${REPO}/releases/download/${VERSION}/${PKG_NAME}-${VERSION}-freebsd-14.zip"
    else
        # Linux
        # Determine if we need the openwrt- prefix (for mips/mipsle)
        case $ARCH in
            mips*|mipsle*)
                BIN_URL="https://github.com/${REPO}/releases/download/${VERSION}/${PKG_NAME}-${VERSION}-openwrt-${SUFFIX}.zip"
                ;;
            *)
                BIN_URL="https://github.com/${REPO}/releases/download/${VERSION}/${PKG_NAME}-${VERSION}-${SUFFIX}.zip"
                ;;
        esac
    fi
    
    info "Downloading ${BIN_URL}"
    download ${TMP_ZIP} ${BIN_URL}
}

# --- setup permissions and move binary to system directory ---
setup_binary() {
    info "Extracting omniedge..."
    $UNZIP -o ${TMP_ZIP} -d ${TMP_BIN} >/dev/null 2>&1
    
    # Find the omniedge binary (might be in root or subdirectory)
    OMNIEDGE_BIN=$(find ${TMP_BIN} -name "omniedge" -type f | head -1)
    
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
        SUFFIX=amd64
        ;;
    arm64|aarch64|armv8*)
        ARCH=arm64
        SUFFIX=arm64
        ;;
    arm*|armv7l)
        ARCH=arm
        SUFFIX=arm
        ;;
    riscv64)
        ARCH=riscv64
        SUFFIX=riscv64
        ;;
    loongarch64)
        ARCH=loongarch64
        SUFFIX=loongarch64
        ;;
    mips)
        ARCH=mips
        SUFFIX=mips
        ;;
    mipsel|mipsle)
        ARCH=mipsle
        SUFFIX=mipsle
        ;;
    *)
        fatal "Unsupported architecture: $ARCH"
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

# --- verify unzip ---
verify_unzip() {
    [ -x "$(command -v $1)" ] || return 1
    UNZIP=$1
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
