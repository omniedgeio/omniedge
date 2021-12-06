#!/bin/sh

set -e
#set -n noglob

STORAGE_URL=https://github.com/omniedgeio/omniedge-linux-cli/releases/download
PKG_NAME="omniedge"
VERSION="v0.1.0"
BIN_DIR="/usr/local/bin"

setup_env() {
    SUDO=sudo
    if [ $(id -u) -eq 0 ]; then
        SUDO=
    fi
}

download_and_verify() {
    setup_verify_arch
    verify_downloader curl || verify_downloader wget || fatal 'Can not find curl or wget for downloading files'
    verify_unzip unzip || fatal 'Can not find unzip'
    setup_tmp
    download_binary
    setup_binary
}

# --- create temporary directory and cleanup when done ---
setup_tmp() {
    TMP_DIR=$(mktemp -d -t omniedge-install.XXXXXXXXXX)
    TMP_HASH=${TMP_DIR}/omniedge.hash
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
    BIN_URL=${STORAGE_URL}/${VERSION}/${PKG_NAME}-${SUFFIX}.zip
    info "Downloading binary zip ${BIN_URL}"
    download ${TMP_ZIP} ${BIN_URL}
}

# --- setup permissions and move binary to system directory ---
setup_binary() {
    info "Unzip omniedge"
    $UNZIP ${TMP_ZIP} -d ${TMP_BIN}
    info "Installing omniedge to ${BIN_DIR}/omniedge"
    $SUDO chown root:root ${TMP_BIN}
    chmod 755 ${TMP_BIN}/omniedge
    $SUDO mv -f ${TMP_BIN}/omniedge ${BIN_DIR}/omniedge
}

# --- set arch and suffix, fatal if architecture not supported ---
setup_verify_arch() {
    if [ -z "$ARCH" ]; then
        ARCH=$(uname -m)
    fi
    case $ARCH in
    amd64)
        ARCH=amd64
        SUFFIX=amd64
        ;;
    x86_64)
        ARCH=amd64
        SUFFIX=amd64
        ;;
    arm64)
        ARCH=arm64
        SUFFIX=arm64v8
        ;;
    aarch64)
        ARCH=arm64
        SUFFIX=arm64v8
        ;;
    arm*)
        ARCH=arm
        SUFFIX=armv7
        ;;
    *)
        fatal "Unsupported architecture $ARCH"
        ;;
    esac
}

# --- verify existence of network downloader executable ---
verify_downloader() {
    # Return failure if it doesn't exist or is no executable
    [ -x "$(command -v $1)" ] || return 1

    # Set verified executable as our downloader program and return success
    DOWNLOADER=$1
    return 0
}

# --- verify unzip ---
verify_unzip() {
    # Return failure if it doesn't exist or is no executable
    [ -x "$(command -v $1)" ] || return 1

    # Set verified executable as our downloader program and return success
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
    echo '[INFO] ' "$@"
}
warn() {
    echo '[WARN] ' "$@" >&2
}
fatal() {
    echo '[ERROR] ' "$@" >&2
    exit 1
}

{
    setup_env
    download_and_verify
}
