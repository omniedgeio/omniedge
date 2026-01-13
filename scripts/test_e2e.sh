#!/bin/bash
set -e

# Setup Directories
ROOT_DIR=$(pwd)
OUT_DIR="$ROOT_DIR/out"
TMP_DIR="$ROOT_DIR/tmp/e2e"

mkdir -p "$OUT_DIR"
rm -rf "$TMP_DIR"
mkdir -p "$TMP_DIR/client_a" "$TMP_DIR/client_b"

echo "=== 1. Building Components ==="
# Build Supernode
echo "Building Supernode..."
cd "$ROOT_DIR/internal/coren2n/n2n"
make supernode
cp supernode "$OUT_DIR/"
cd "$ROOT_DIR"

# Build OmniEdge CLI
echo "Building OmniEdge CLI..."
go build -trimpath -o "$OUT_DIR/omniedge" ./cmd/edgecli/main.go

# Build Mock API
echo "Building Mock API..."
go build -trimpath -o "$OUT_DIR/mock_api" ./tests/mock_api/main.go

echo "=== 2. Starting Infrastructure ==="
# Start Supernode
echo "Starting Supernode on port 7654..."
"$OUT_DIR/supernode" -l 7654 > "$TMP_DIR/supernode.log" 2>&1 &
SN_PID=$!
echo "Supernode PID: $SN_PID"

# Start Mock API
echo "Starting Mock API on port 8080..."
"$OUT_DIR/mock_api" > "$TMP_DIR/mock_api.log" 2>&1 &
API_PID=$!
echo "Mock API PID: $API_PID"

# Cleanup function
cleanup() {
    echo "=== Shutting down... ==="
    kill $SN_PID || true
    kill $API_PID || true
    if [ -f "$TMP_DIR/client_a.pid" ]; then
        kill $(cat "$TMP_DIR/client_a.pid") || true
    fi
    if [ -f "$TMP_DIR/client_b.pid" ]; then
        kill $(cat "$TMP_DIR/client_b.pid") || true
    fi
}
trap cleanup EXIT

sleep 2

echo "=== 3. Starting Clients ==="

# Pre-seed Auth (Mock Token)
echo '{"device":{"uuid":"dev-1"},"authresponse":{"token":"mock-token"}}' > "$TMP_DIR/client_a/auth.json"
echo '{"device":{"uuid":"dev-2"},"authresponse":{"token":"mock-token"}}' > "$TMP_DIR/client_b/auth.json"

# Start Client A
echo "Starting Client A..."
export OMNIEDGE_PID_FILE="$TMP_DIR/client_a.pid"
export OMNIEDGE_LOG_FILE="$TMP_DIR/client_a.log"
export OMNIEDGE_REST_ENDPOINT_URL="http://localhost:8080/api/v2"
export OMNIEDGE_MDNS_ENABLE=0 # Disable multicast discovery if possible to force SN usage

# Start in background/daemon mode via 'start' logic
"$OUT_DIR/omniedge" start -f "$TMP_DIR/client_a/auth.json" -n test-net &
# Client A should get 100.100.0.1 (based on mock api logic)

# Start Client B
echo "Starting Client B..."
export OMNIEDGE_PID_FILE="$TMP_DIR/client_b.pid"
export OMNIEDGE_LOG_FILE="$TMP_DIR/client_b.log"
# Reuse same environment vars for common parts

"$OUT_DIR/omniedge" start -f "$TMP_DIR/client_b/auth.json" -n test-net &
# Client B should get 100.100.0.2

echo "Waiting for clients to connect..."
sleep 15

# Verify Client A Log
echo "--- Client A Log Tail ---"
tail -n 10 "$TMP_DIR/client_a.log"

echo "=== 4. Verifying Connectivity ==="
# Verify if interfaces exist (Cross-platform)
check_interface() {
    if command -v ip >/dev/null 2>&1; then
        ip link show "$1" > /dev/null 2>&1
    else
        ifconfig "$1" > /dev/null 2>&1
    fi
}

if check_interface OmniEdge0; then
    echo "Interface OmniEdge0 found."
else
    echo "Interface OmniEdge0 NOT found. Test failed."
    exit 1
fi

if check_interface OmniEdge1; then
    echo "Interface OmniEdge1 found."
else
    echo "Interface OmniEdge1 NOT found. Test failed."
    exit 1
fi

# Ping 100.100.0.2 (Client B) from Host (routing through OmniEdge0 implicitly or explicitly)
echo "Pinging 100.100.0.2 ..."
if ping -c 4 -W 5 100.100.0.2; then
    echo "✅ Success: Connectivity verified!"
else
    echo "❌ Failed: Could not ping Client B."
    echo "--- Supernode Log ---"
    cat "$TMP_DIR/supernode.log"
    echo "--- Client A Log ---"
    cat "$TMP_DIR/client_a.log"
    echo "--- Client B Log ---"
    cat "$TMP_DIR/client_b.log"
    exit 1
fi
