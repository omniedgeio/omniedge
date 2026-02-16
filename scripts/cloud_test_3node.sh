#!/bin/bash
# =============================================================================
# OmniEdge 3-Node Cloud Test Orchestrator (v3.0.0)
# Run from LOCAL machine, orchestrates tests between cloud instances
# Architecture: 3-Node (Nucleus in dual mode + Edge A + Edge B)
#
# Features:
#   - Nucleus server running in dual mode (signaling/relay + edge client)
#   - Two additional edge nodes connecting through the nucleus
#   - Support for localhost via Docker or native execution
#   - Automatic dependency installation
#   - Baseline vs VPN performance comparison
#   - IPv4 and IPv6 tunnel testing
#
# Example:
#   # All cloud nodes
#   ./scripts/cloud_test_3node.sh --nucleus 138.197.223.106 --node-a 54.x.x.x --node-b 35.x.x.x \
#      --network abc123 --key sk_xxx --ssh-key ~/.ssh/cloud.pem
#
#   # Localhost (Docker) + cloud nodes
#   ./scripts/cloud_test_3node.sh --nucleus 138.197.223.106 --node-a localhost --node-b 35.x.x.x \
#      --network abc123 --key sk_xxx --local-docker --ssh-key ~/.ssh/cloud.pem
# =============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

print_header() {
    echo -e "\n${GREEN}=== $1 ===${NC}\n"
}

print_step() {
    echo -e "${CYAN}>>> $1${NC}"
}

print_error() {
    echo -e "${RED}ERROR: $1${NC}"
}

# =============================================================================
# Configuration
# =============================================================================

NUCLEUS=""
NUCLEUS_PORT="${NUCLEUS_PORT:-51821}"
NODE_A=""
NODE_B=""
SSH_KEY=""
SSH_USER="${SSH_USER:-ubuntu}"
SSH_USER_NUCLEUS=""
SSH_USER_A=""
SSH_USER_B=""
NETWORK_ID=""
SECURITY_KEY=""
TEST_DURATION=${TEST_DURATION:-10}
RESULTS_DIR="./test_results"

# Virtual IPs are assigned by the network
VIP_NUCLEUS=""
VIP_A=""
VIP_B=""
VIP6_A=""
VIP6_B=""
TEST_IPV6=true

# Local execution settings
LOCAL_DOCKER=false
LOCAL_DOCKER_NAME="omni-node-local"
USE_LOCAL_CLI=false
USE_LOCAL_BIN=false

# Get script and project directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

show_help() {
    cat << EOF
OmniEdge 3-Node Cloud Test Orchestrator

Architecture:
   ┌──────────────────┐         ┌──────────────────┐
   │     Edge A       │         │     Edge B       │
   │  (omniedge CLI)  │         │  (omniedge CLI)  │
   └────────┬─────────┘         └────────┬─────────┘
            │                            │
            │      VPN Tunnel (P2P       │
            │       or via Relay)        │
            │                            │
            └──────────┬─────────────────┘
                       │
              ┌────────▼────────┐
              │     Nucleus     │
              │  (Dual Mode)    │
              │ Signaling+Relay │
              │   + Edge VIP    │
              └─────────────────┘

Usage:
  $0 --nucleus <IP> --node-a <IP> --node-b <IP> --network <ID> --key <KEY> [OPTIONS]

Required:
  --nucleus       IP address of Nucleus server (runs in dual mode)
  --node-a        IP address of Edge A (cloud server or "localhost")
  --node-b        IP address of Edge B (cloud server or "localhost")
  --network       OmniEdge Virtual Network ID
  --key           OmniEdge Security Key

Options:
  --nucleus-port  Nucleus signaling port (default: 51821)
  --ssh-key       Path to SSH private key
  --ssh-user      SSH username for all nodes (default: ubuntu)
  --ssh-user-nucleus  SSH username for Nucleus node (overrides --ssh-user)
  --ssh-user-a    SSH username for Edge A (overrides --ssh-user)
  --ssh-user-b    SSH username for Edge B (overrides --ssh-user)
  --duration      iperf3 test duration in seconds (default: 10)
  --no-ipv6       Skip IPv6 tests
  --skip-deploy   Skip OmniEdge installation (use existing)
  --use-local-bin Deploy pre-built binaries from ./scripts/ folder
  --use-local-cli Build and deploy from local source code
  --local-docker  Run localhost nodes in Docker containers
  --help          Show this help

Localhost Execution Modes:
  --local-docker  (default for localhost) Run in Docker container
                  - Isolated environment, requires Docker
                  
  --use-local-cli Run localhost natively on host system
                  - Uses cargo to build CLI from local source
                  - Requires sudo access

Examples:
  # All cloud nodes (Nucleus already running)
  $0 --nucleus 138.197.223.106 --node-a 54.x.x.x --node-b 35.x.x.x \\
     --network abc123 --key sk_xxx --ssh-key ~/.ssh/cloud.pem

  # Localhost (Docker) as Edge A + cloud Nucleus and Edge B
  $0 --nucleus 138.197.223.106 --node-a localhost --node-b 35.x.x.x \\
     --network abc123 --key sk_xxx --local-docker --ssh-key ~/.ssh/cloud.pem

  # Native localhost as Edge A (uses local cargo build)
  $0 --nucleus 138.197.223.106 --node-a localhost --node-b 35.x.x.x \\
     --network abc123 --key sk_xxx --use-local-cli --ssh-key ~/.ssh/cloud.pem

  # Different SSH users per node
  $0 --nucleus 138.197.223.106 --node-a 54.x.x.x --node-b 35.x.x.x \\
     --network abc123 --key sk_xxx --ssh-key ~/.ssh/cloud.pem \\
     --ssh-user-nucleus root --ssh-user-a ubuntu --ssh-user-b ec2-user

Prerequisites:
   - SSH access with key authentication to cloud nodes
   - Docker installed (if using --local-docker)
   - Nucleus server running omniedge in dual mode (--nucleus flag)
   - Root/sudo access for TUN interface creation
   - Ports: UDP 51821 (signaling), UDP 51820 (WireGuard), TCP 5201-5202 (iperf3)
EOF
}

# =============================================================================
# Helper Functions (from cloud_test.sh)
# =============================================================================

parse_ping_latency() {
    local ping_output="$1"
    local latency=""
    
    if echo "$ping_output" | grep -q "rtt"; then
        latency=$(echo "$ping_output" | grep "rtt" | awk -F'/' '{print $5}')
    elif echo "$ping_output" | grep -q "round-trip"; then
        latency=$(echo "$ping_output" | grep "round-trip" | awk -F'/' '{print $5}' | awk '{print $1}')
    fi
    
    echo "$latency"
}

ping_successful() {
    local ping_output="$1"
    if echo "$ping_output" | grep -qE "rtt|round-trip"; then
        return 0
    fi
    return 1
}

is_local() {
    local host="$1"
    if [[ "$host" == "localhost" || "$host" == "127.0.0.1" || "$host" == "::1" ]]; then
        return 0
    fi
    if command -v ifconfig &>/dev/null; then
        if ifconfig | grep -w "inet" | awk '{print $2}' | grep -qx "$host"; then return 0; fi
    elif command -v ip &>/dev/null; then
        if ip addr | grep -w "inet" | awk '{print $2}' | cut -d/ -f1 | grep -qx "$host"; then return 0; fi
    fi
    return 1
}

get_ssh_user_for_host() {
    local host="$1"
    if [[ "$host" == "$NUCLEUS" ]]; then
        echo "$SSH_USER_NUCLEUS"
    elif [[ "$host" == "$NODE_A" ]]; then
        echo "$SSH_USER_A"
    elif [[ "$host" == "$NODE_B" ]]; then
        echo "$SSH_USER_B"
    else
        echo "$SSH_USER"
    fi
}

ssh_cmd() {
    local host="$1"
    shift
    if is_local "$host"; then
        if [[ "$LOCAL_DOCKER" == "true" ]]; then
            local cmd="$*"
            if [[ "$cmd" == sudo\ * ]]; then
                cmd="${cmd#sudo }"
            fi
            cmd=$(echo "$cmd" | sed 's/\bsudo //g')
            
            if [[ "$cmd" == *' &' || "$cmd" == *'&' ]]; then
                cmd="${cmd% &}"
                cmd="${cmd%&}"
                docker exec -d "$LOCAL_DOCKER_NAME" sh -c "$cmd"
            else
                docker exec "$LOCAL_DOCKER_NAME" sh -c "$cmd"
            fi
        else
            sudo sh -c "$*"
        fi
    else
        local ssh_user
        ssh_user=$(get_ssh_user_for_host "$host")
        ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
            ${SSH_KEY:+-i "$SSH_KEY"} \
            "$ssh_user@$host" "$@"
    fi
}

scp_to() {
    local src="$1"
    local host="$2"
    local dest="$3"
    if is_local "$host"; then
        if [[ "$LOCAL_DOCKER" == "true" ]]; then
            local container_dest="${dest/#\~//root}"
            docker exec "$LOCAL_DOCKER_NAME" mkdir -p "$(dirname "$container_dest")"
            docker cp "$src" "$LOCAL_DOCKER_NAME:$container_dest"
        else
            local real_dest="${dest/#\~/$HOME}"
            mkdir -p "$(dirname "$real_dest")"
            cp "$src" "$real_dest"
        fi
    else
        local ssh_user
        ssh_user=$(get_ssh_user_for_host "$host")
        scp -o StrictHostKeyChecking=no \
            ${SSH_KEY:+-i "$SSH_KEY"} \
            "$src" "$ssh_user@$host:$dest"
    fi
}

get_vip_with_retry() {
    local node="$1"
    local ip_version="${2:-4}"
    local max_attempts=12
    local attempt=1
    local vip=""
    
    local node_os=""
    node_os=$(ssh_cmd "$node" "uname -s" 2>/dev/null | tr -d '\r\n')
    
    while [[ $attempt -le $max_attempts && -z "$vip" ]]; do
        if [[ "$ip_version" == "4" ]]; then
            vip=$(ssh_cmd "$node" "omniedge status --json 2>/dev/null | jq -r '.vip // empty'" 2>/dev/null | tr -d '\r\n' || echo "")
        else
            vip=$(ssh_cmd "$node" "omniedge status --json 2>/dev/null | jq -r '.vip6 // empty'" 2>/dev/null | tr -d '\r\n' || echo "")
        fi
        
        if [[ -z "$vip" || "$vip" == "null" ]]; then
            if [[ "$node_os" == "Darwin" ]]; then
                if [[ "$ip_version" == "4" ]]; then
                    vip=$(ssh_cmd "$node" "ifconfig 2>/dev/null | grep -A5 '^utun' | grep 'inet ' | grep '100\\.' | awk '{print \$2}' | head -1" || echo "")
                else
                    vip=$(ssh_cmd "$node" "ifconfig 2>/dev/null | grep -A5 '^utun' | grep 'inet6' | grep -v 'fe80' | awk '{print \$2}' | cut -d% -f1 | head -1" || echo "")
                fi
            else
                local iface_name=""
                if ssh_cmd "$node" "ip link show omniedge0" &>/dev/null; then
                    iface_name="omniedge0"
                elif ssh_cmd "$node" "ip link show omni0" &>/dev/null; then
                    iface_name="omni0"
                fi
                
                if [[ -n "$iface_name" ]]; then
                    if [[ "$ip_version" == "4" ]]; then
                        vip=$(ssh_cmd "$node" "ip addr show $iface_name 2>/dev/null | grep 'inet ' | awk '{print \$2}' | cut -d/ -f1 | head -1" || echo "")
                    else
                        vip=$(ssh_cmd "$node" "ip -6 addr show $iface_name 2>/dev/null | grep 'inet6' | grep -v 'fe80' | awk '{print \$2}' | cut -d/ -f1 | head -1" || echo "")
                    fi
                fi
            fi
        fi
        
        if [[ -z "$vip" || "$vip" == "null" ]]; then
            echo "   Attempt $attempt/$max_attempts: Waiting for OmniEdge interface on $node..." >&2
            sleep 5
        fi
        attempt=$((attempt + 1))
    done
    
    if [[ "$vip" == "null" ]]; then
        echo ""
    else
        echo "$vip"
    fi
}

verify_omniedge_interface() {
    local node="$1"
    
    local node_os=""
    node_os=$(ssh_cmd "$node" "uname -s" 2>/dev/null | tr -d '\r\n')
    
    local iface_name=""
    local has_ip="0"
    
    if [[ "$node_os" == "Darwin" ]]; then
        local utun_with_ip=""
        utun_with_ip=$(ssh_cmd "$node" "ifconfig 2>/dev/null | grep -B5 'inet 100\\.' | grep '^utun' | awk -F: '{print \$1}' | head -1" || echo "")
        
        if [[ -n "$utun_with_ip" ]]; then
            iface_name="$utun_with_ip"
            has_ip="1"
        else
            iface_name=$(ssh_cmd "$node" "omniedge status --json 2>/dev/null | jq -r '.interface // empty'" 2>/dev/null | tr -d '\r\n' || echo "")
            if [[ -n "$iface_name" && "$iface_name" != "null" ]]; then
                has_ip=$(ssh_cmd "$node" "ifconfig $iface_name 2>/dev/null | grep -c 'inet '" || echo "0")
            fi
        fi
        
        if [[ -z "$iface_name" ]]; then
            echo "  ❌ OmniEdge interface (utun*) does not exist on $node" >&2
            return 1
        fi
    else
        if ssh_cmd "$node" "ip link show omniedge0" &>/dev/null; then
            iface_name="omniedge0"
        elif ssh_cmd "$node" "ip link show omni0" &>/dev/null; then
            iface_name="omni0"
        fi
        
        if [[ -z "$iface_name" ]]; then
            echo "  ❌ OmniEdge interface (omniedge0/omni0) does not exist on $node" >&2
            return 1
        fi
        
        has_ip=$(ssh_cmd "$node" "ip addr show $iface_name 2>/dev/null | grep -c 'inet '" || echo "0")
    fi
    
    if [[ "$has_ip" == "0" ]]; then
        echo "  ⚠️ $iface_name interface exists but has no IP on $node" >&2
        return 1
    fi
    
    echo "  ✅ $iface_name interface verified on $node" >&2
    return 0
}

# =============================================================================
# Local Docker Setup
# =============================================================================

ensure_local_docker() {
    if [[ "$USE_LOCAL_CLI" == "true" ]]; then
        return 0
    fi
    if [[ "$LOCAL_DOCKER" != "true" ]]; then return 0; fi
    
    local container_exists=false
    if docker ps --format '{{.Names}}' | grep -q "^$LOCAL_DOCKER_NAME$"; then
        container_exists=true
    fi

    if [[ "$container_exists" == "false" ]]; then
        print_step "Setting up local Docker environment ($LOCAL_DOCKER_NAME)..."
        docker rm -f "$LOCAL_DOCKER_NAME" 2>/dev/null || true
        
        docker run -d --name "$LOCAL_DOCKER_NAME" \
            --privileged \
            --cap-add=NET_ADMIN \
            --device /dev/net/tun:/dev/net/tun \
            ubuntu:24.04 sleep infinity
            
        print_step "Installing dependencies in local Docker container..."
        docker exec "$LOCAL_DOCKER_NAME" apt-get update -qq
        docker exec "$LOCAL_DOCKER_NAME" apt-get install -y -qq iperf3 wireguard-tools iproute2 jq bc psmisc curl ca-certificates sudo iputils-ping procps
    else
        if ! docker exec "$LOCAL_DOCKER_NAME" which curl &>/dev/null || \
           ! docker exec "$LOCAL_DOCKER_NAME" which jq &>/dev/null; then
            print_step "Repairing missing dependencies in existing Docker container..."
            docker exec "$LOCAL_DOCKER_NAME" apt-get update -qq
            docker exec "$LOCAL_DOCKER_NAME" apt-get install -y -qq iperf3 wireguard-tools iproute2 jq bc psmisc curl ca-certificates sudo iputils-ping procps
        fi
    fi
    
    docker exec "$LOCAL_DOCKER_NAME" mkdir -p /root/.omniedge
    
    if [[ "$container_exists" == "true" ]]; then
        docker exec "$LOCAL_DOCKER_NAME" rm -rf /root/.omniedge/* 2>/dev/null || true
    fi
    
    if ! docker exec "$LOCAL_DOCKER_NAME" ls -la /dev/net/tun &>/dev/null; then
        print_step "Verifying/Creating TUN device in container..."
        docker exec "$LOCAL_DOCKER_NAME" mkdir -p /dev/net 2>/dev/null || true
        docker exec "$LOCAL_DOCKER_NAME" mknod /dev/net/tun c 10 200 2>/dev/null || true
        docker exec "$LOCAL_DOCKER_NAME" chmod 600 /dev/net/tun 2>/dev/null || true
    fi
}

# =============================================================================
# Build Functions
# =============================================================================

get_rust_target() {
    local arch="$1"
    local os="$2"
    
    case "$os" in
        Linux|linux)
            case "$arch" in
                x86_64) echo "x86_64-unknown-linux-gnu" ;;
                aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
                armv7l|armhf) echo "armv7-unknown-linux-gnueabihf" ;;
                *) echo "" ;;
            esac
            ;;
        Darwin|darwin)
            case "$arch" in
                x86_64) echo "x86_64-apple-darwin" ;;
                arm64|aarch64) echo "aarch64-apple-darwin" ;;
                *) echo "" ;;
            esac
            ;;
        *)
            echo ""
            ;;
    esac
}

detect_node_arch() {
    local node="$1"
    
    if is_local "$node" && [[ "$USE_LOCAL_CLI" == "true" ]]; then
        local arch=$(uname -m)
        local os=$(uname -s)
        echo "$arch:$os"
        return 0
    fi
    
    local arch=$(ssh_cmd "$node" "uname -m" 2>/dev/null | tr -d '\r\n')
    local os=$(ssh_cmd "$node" "uname -s" 2>/dev/null | tr -d '\r\n')
    
    echo "$arch:$os"
}

build_local_cli() {
    local target="$1"
    local output_dir="$SCRIPT_DIR"
    
    local output_name=""
    local search_pattern=""
    case "$target" in
        x86_64-unknown-linux-gnu)
            output_name="omniedge-cli-local-linux-x64"
            search_pattern="omniedge-cli-*-linux-x64"
            ;;
        aarch64-unknown-linux-gnu)
            output_name="omniedge-cli-local-linux-arm64"
            search_pattern="omniedge-cli-*-linux-arm64"
            ;;
        armv7-unknown-linux-gnueabihf)
            output_name="omniedge-cli-local-linux-armv7"
            search_pattern="omniedge-cli-*-linux-armv7"
            ;;
        x86_64-apple-darwin)
            output_name="omniedge-cli-local-macos-x64"
            search_pattern="omniedge-cli-*-macos-x64"
            ;;
        aarch64-apple-darwin)
            output_name="omniedge-cli-local-macos-arm64"
            search_pattern="omniedge-cli-*-macos-arm64"
            ;;
        *)
            output_name="omniedge-cli-local-$target"
            search_pattern="omniedge-cli-*"
            ;;
    esac
    
    local existing_bin=""
    existing_bin=$(ls -1 "$output_dir"/$search_pattern 2>/dev/null | sort -V | tail -1)
    
    if [[ -n "$existing_bin" && -f "$existing_bin" ]]; then
        echo "  ✅ Found existing binary: $(basename "$existing_bin")" >&2
        echo "$existing_bin"
        return 0
    fi
    
    print_step "Building OmniEdge CLI for target: $target"
    
    if ! command -v cargo &>/dev/null; then
        print_error "Cargo not found. Please install Rust: https://rustup.rs"
        return 1
    fi
    
    if [[ ! -f "$PROJECT_ROOT/Cargo.toml" ]]; then
        print_error "Cannot find project root (Cargo.toml not found at $PROJECT_ROOT)"
        return 1
    fi
    
    local host_arch=$(uname -m)
    local host_os=$(uname -s)
    local host_target=$(get_rust_target "$host_arch" "$host_os")
    
    local build_cmd="cargo build -p omniedge-cli --release"
    local binary_path="$PROJECT_ROOT/target/release/omniedge"
    
    if [[ "$target" != "$host_target" ]]; then
        echo "  Cross-compiling from $host_target to $target..."
        
        if ! rustup target list --installed | grep -q "$target"; then
            echo "  Installing target $target..."
            rustup target add "$target" || {
                print_error "Failed to add Rust target: $target"
                return 1
            }
        fi
        
        if command -v cross &>/dev/null; then
            build_cmd="cross build -p omniedge-cli --release --target $target"
            binary_path="$PROJECT_ROOT/target/$target/release/omniedge"
        else
            build_cmd="cargo build -p omniedge-cli --release --target $target"
            binary_path="$PROJECT_ROOT/target/$target/release/omniedge"
        fi
    fi
    
    echo "  Running: $build_cmd"
    (cd "$PROJECT_ROOT" && $build_cmd) || {
        print_error "Build failed for target: $target"
        return 1
    }
    
    if [[ ! -f "$binary_path" ]]; then
        print_error "Binary not found at: $binary_path"
        return 1
    fi
    
    cp "$binary_path" "$output_dir/$output_name"
    chmod +x "$output_dir/$output_name"
    
    echo "  ✅ Built: $output_dir/$output_name" >&2
    echo "$output_dir/$output_name"
}

get_binary_for_node() {
    local node="$1"
    local arch_os=$(detect_node_arch "$node")
    local arch="${arch_os%%:*}"
    local os="${arch_os##*:}"
    
    echo "  Detected $node: arch=$arch, os=$os" >&2
    
    local target=$(get_rust_target "$arch" "$os")
    if [[ -z "$target" ]]; then
        print_error "Unsupported architecture/OS: $arch / $os"
        return 1
    fi
    
    local binary_path=$(build_local_cli "$target") || return 1
    echo "$binary_path"
}

# =============================================================================
# Pre-flight Checks
# =============================================================================

preflight_check() {
    print_header "Pre-flight Checks"
    
    ensure_local_docker
    
    local errors=0
    
    print_step "Checking local dependencies..."
    local deps="ssh scp jq bc curl"
    if [[ "$LOCAL_DOCKER" == "true" ]]; then
        deps="$deps docker"
    fi
    if [[ "$USE_LOCAL_CLI" == "true" ]]; then
        deps="$deps cargo"
    fi
    for cmd in $deps; do
        if which "$cmd" &>/dev/null; then
            echo -e "  ✅ Local $cmd found"
        else
            echo -e "  ❌ Local $cmd NOT found. Please install it."
            errors=$((errors + 1))
        fi
    done
    
    if [[ "$USE_LOCAL_CLI" == "true" ]]; then
        if [[ -f "$PROJECT_ROOT/Cargo.toml" ]]; then
            echo -e "  ✅ Project root found: $PROJECT_ROOT"
        else
            echo -e "  ❌ Cargo.toml not found at $PROJECT_ROOT"
            errors=$((errors + 1))
        fi
    fi
    
    # Check connectivity to all nodes
    print_step "Testing connectivity to Nucleus ($NUCLEUS)..."
    if ssh_cmd "$NUCLEUS" "echo ok" &>/dev/null; then
        echo -e "  ✅ Connectivity to Nucleus successful"
    else
        echo -e "  ❌ Connectivity to Nucleus failed"
        errors=$((errors + 1))
    fi
    
    for node in "$NODE_A" "$NODE_B"; do
        print_step "Testing connectivity to $node..."
        if ssh_cmd "$node" "echo ok" &>/dev/null; then
            echo -e "  ✅ Connectivity to $node successful"
        else
            echo -e "  ❌ Connectivity to $node failed"
            errors=$((errors + 1))
        fi
    done
    
    # Check sudo/root access
    print_step "Checking root/sudo access..."
    for node in "$NUCLEUS" "$NODE_A" "$NODE_B"; do
        if ssh_cmd "$node" "sudo -n true" &>/dev/null; then
            echo -e "  ✅ Root/Sudo access available on $node"
        else
            if is_local "$node" && [[ "$LOCAL_DOCKER" == "true" ]]; then
                echo -e "  ✅ Root access available in Docker container on $node"
            else
                echo -e "  ⚠️ Sudo might require password on $node"
            fi
        fi
    done
    
    if [[ $errors -gt 0 ]]; then
        print_error "Pre-flight checks failed with $errors errors"
        exit 1
    fi
    
    echo -e "\n${GREEN}All pre-flight checks passed!${NC}"
}

# =============================================================================
# Install Dependencies
# =============================================================================

install_dependencies() {
    print_header "Installing Missing Dependencies"
    
    for node in "$NODE_A" "$NODE_B"; do
        print_step "Checking and installing dependencies on $node..."
        
        local remote_os=$(ssh_cmd "$node" "uname")
        if [[ "$remote_os" == "Darwin" ]]; then
            echo -e "  🍏 macOS detected on $node, assuming dependencies installed"
            continue
        fi

        local pkg_manager=""
        if ssh_cmd "$node" "which apt-get" &>/dev/null; then
            pkg_manager="apt"
        elif ssh_cmd "$node" "which dnf" &>/dev/null; then
            pkg_manager="dnf"
        elif ssh_cmd "$node" "which yum" &>/dev/null; then
            pkg_manager="yum"
        else
            echo -e "  ⚠️ Unknown package manager on $node, skipping"
            continue
        fi
        
        if [[ "$pkg_manager" == "apt" ]]; then
            ssh_cmd "$node" "sudo apt-get update -qq" || true
        fi
        
        ssh_cmd "$node" "which iperf3 &>/dev/null || (sudo $pkg_manager install -y iperf3 || true)" || true
        ssh_cmd "$node" "which ip &>/dev/null || (sudo $pkg_manager install -y iproute2 || true)" || true
        ssh_cmd "$node" "which jq &>/dev/null || (sudo $pkg_manager install -y jq || true)" || true
        ssh_cmd "$node" "which bc &>/dev/null || (sudo $pkg_manager install -y bc || true)" || true
        ssh_cmd "$node" "which curl &>/dev/null || (sudo $pkg_manager install -y curl || true)" || true
        
        echo -e "  ✅ Dependencies installed on $node"
    done
    
    echo -e "\n${GREEN}Dependency installation complete!${NC}"
}

# =============================================================================
# Deploy OmniEdge
# =============================================================================

deploy_omniedge() {
    print_header "Deploying OmniEdge to All Nodes"
    
    local nodes_to_deploy=("$NODE_A" "$NODE_B" "$NUCLEUS")
    
    if [[ "$USE_LOCAL_CLI" == "true" || "$USE_LOCAL_BIN" == "true" ]]; then
        echo -e "📦 Deploying OmniEdge CLI from local source/binaries..."
        
        for node in "${nodes_to_deploy[@]}"; do
            print_step "Deploying CLI to $node..."
            
            local bin_path
            bin_path=$(get_binary_for_node "$node") || exit 1
            
            if is_local "$node"; then
                echo -e "  🚀 Installing $(basename "$bin_path") locally..."
                sudo cp "$bin_path" /usr/local/bin/omniedge
                sudo chmod +x /usr/local/bin/omniedge
                
                local version
                version=$(omniedge --version 2>/dev/null | head -1 || echo "unknown")
                echo -e "  ✅ OmniEdge installed locally ($version)"
            else
                echo -e "  🚀 Uploading $(basename "$bin_path") to $node..."
                scp_to "$bin_path" "$node" "/tmp/omniedge"
                ssh_cmd "$node" "sudo mv /tmp/omniedge /usr/local/bin/omniedge && sudo chmod +x /usr/local/bin/omniedge"
                
                local version
                version=$(ssh_cmd "$node" "omniedge --version 2>/dev/null | head -1" || echo "unknown")
                echo -e "  ✅ OmniEdge installed on $node ($version)"
            fi
        done
    else
        # Use installer script
        local INSTALLER_URL="https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh"
        echo -e "📦 Installing OmniEdge via installer script..."
        
        for node in "${nodes_to_deploy[@]}"; do
            print_step "Installing OmniEdge on $node..."
            
            if ssh_cmd "$node" "which omniedge" &>/dev/null; then
                local existing_version
                existing_version=$(ssh_cmd "$node" "omniedge --version 2>/dev/null | head -1" || echo "unknown")
                echo -e "  ℹ️ OmniEdge already installed: $existing_version"
            fi
            
            ssh_cmd "$node" "curl -fsSL $INSTALLER_URL | sudo bash"
            
            if ssh_cmd "$node" "which omniedge" &>/dev/null; then
                local version
                version=$(ssh_cmd "$node" "omniedge --version 2>/dev/null | head -1" || echo "unknown")
                echo -e "  ✅ OmniEdge installed on $node ($version)"
            else
                print_error "Installation failed on $node"
                exit 1
            fi
        done
    fi
    
    echo -e "\n${GREEN}OmniEdge installation complete!${NC}"
}

# =============================================================================
# Run Test
# =============================================================================

run_test() {
    print_header "Running 3-Node OmniEdge VPN Test"
    
    mkdir -p "$RESULTS_DIR"
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)
    local result_file="$RESULTS_DIR/cloud_test_3node_$timestamp.json"
    
    if [[ -z "$NETWORK_ID" ]]; then
        print_error "--network is required"
        exit 1
    fi
    
    if [[ -z "$SECURITY_KEY" ]]; then
        print_error "--key (security key) is required"
        exit 1
    fi
    
    echo -e "🔐 Network ID: $NETWORK_ID"
    echo -e "🔑 Security Key: ${SECURITY_KEY:0:10}..."
    echo -e "🌐 Nucleus: $NUCLEUS:$NUCLEUS_PORT"

    # Clean up old processes on all nodes
    print_step "Cleaning up old processes..."
    for node in "$NODE_A" "$NODE_B" "$NUCLEUS"; do
        ssh_cmd "$node" "sudo pkill -9 -f 'omniedge.*--daemon' || true; \
                         sleep 1; \
                         sudo pkill -9 -f omniedge || true; \
                         sudo pkill -9 -f iperf3 || true; \
                         sudo ip link delete omniedge0 2>/dev/null || true; \
                         sudo ip link delete omni0 2>/dev/null || true; \
                         sudo rm -f /tmp/omni-*.log; \
                         sudo rm -f /root/.omniedge/logs/omniedge*.log" || true
    done
    
    sleep 3
    
    # Start Nucleus in dual mode
    print_step "Starting Nucleus in dual mode on $NUCLEUS..."
    ssh_cmd "$NUCLEUS" "sudo touch /tmp/omni-nucleus-cli.log && sudo chmod 666 /tmp/omni-nucleus-cli.log"
    # Note: Using --mode dual for dual mode (signaling + edge), --port for nucleus port
    ssh_cmd "$NUCLEUS" "sudo RUST_LOG=debug nohup omniedge start -v -n ${NETWORK_ID} -s ${SECURITY_KEY} --mode nucleus --port ${NUCLEUS_PORT} > /tmp/omni-nucleus-cli.log 2>&1 &"
    sleep 5
    
    # Get Nucleus VIP
    VIP_NUCLEUS=$(get_vip_with_retry "$NUCLEUS" "4")
    if [[ -z "$VIP_NUCLEUS" ]]; then
        print_error "Failed to start Nucleus or get VIP"
        # Try to show logs
        ssh_cmd "$NUCLEUS" "tail -20 /tmp/omni-nucleus-cli.log"
        exit 1
    fi
    echo -e "  📍 Nucleus VIP: ${VIP_NUCLEUS}"
    
    # Start Edge A
    # Note: RUST_LOG is inherited by the daemon process (see service.rs)
    # CLI output goes to /tmp/omni-edge-a.log, daemon logs to /root/.omniedge/logs/omniedge.log
    print_step "Starting Edge A on $NODE_A..."
    ssh_cmd "$NODE_A" "sudo touch /tmp/omni-edge-a.log && sudo chmod 666 /tmp/omni-edge-a.log"
    ssh_cmd "$NODE_A" "sudo RUST_LOG=debug nohup omniedge start -v -n ${NETWORK_ID} -s ${SECURITY_KEY} > /tmp/omni-edge-a.log 2>&1 &"
    sleep 5

    # Start Edge B
    print_step "Starting Edge B on $NODE_B..."
    ssh_cmd "$NODE_B" "sudo touch /tmp/omni-edge-b.log && sudo chmod 666 /tmp/omni-edge-b.log"
    ssh_cmd "$NODE_B" "sudo RUST_LOG=debug nohup omniedge start -v -n ${NETWORK_ID} -s ${SECURITY_KEY} > /tmp/omni-edge-b.log 2>&1 &"
    sleep 3
    
    # Wait for VPN tunnel establishment
    print_step "Waiting for VPN tunnel establishment (60s)..."
    sleep 60
    
    # Check daemon processes
    print_step "Checking daemon processes..."
    echo "Nucleus process:"
    ssh_cmd "$NUCLEUS" "pgrep -a omniedge || echo 'NOT RUNNING'"
    echo "Edge A process:"
    ssh_cmd "$NODE_A" "pgrep -a omniedge || echo 'NOT RUNNING'"
    echo "Edge B process:"
    ssh_cmd "$NODE_B" "pgrep -a omniedge || echo 'NOT RUNNING'"
    echo ""
    
    # Get VIPs
    print_step "Getting VPN IPs from interfaces..."
    VIP_A=$(get_vip_with_retry "$NODE_A" "4")
    VIP_B=$(get_vip_with_retry "$NODE_B" "4")
    
    echo "Nucleus VIP: ${VIP_NUCLEUS:-'(not assigned)'}"
    echo "Edge A VIP:  ${VIP_A:-'(not assigned)'}"
    echo "Edge B VIP:  ${VIP_B:-'(not assigned)'}"
    
    # Verify interfaces
    print_step "Verifying OmniEdge interfaces..."
    verify_omniedge_interface "$NUCLEUS" || true
    verify_omniedge_interface "$NODE_A" || true
    verify_omniedge_interface "$NODE_B" || true
    
    # Show connection status
    print_step "Connection Status..."
    echo "--- Nucleus status ---"
    ssh_cmd "$NUCLEUS" "omniedge status 2>/dev/null || echo 'Status unavailable'"
    echo ""
    echo "--- Edge A status ---"
    ssh_cmd "$NODE_A" "omniedge status 2>/dev/null || echo 'Status unavailable'"
    echo ""
    echo "--- Edge B status ---"
    ssh_cmd "$NODE_B" "omniedge status 2>/dev/null || echo 'Status unavailable'"
    echo ""
    
    # Show logs
    print_step "Edge logs (last 20 lines)..."
    echo "--- Edge A log ---"
    ssh_cmd "$NODE_A" "tail -20 /tmp/omni-edge-a.log 2>/dev/null || echo 'No log'"
    echo ""
    echo "--- Edge B log ---"
    ssh_cmd "$NODE_B" "tail -20 /tmp/omni-edge-b.log 2>/dev/null || echo 'No log'"
    echo ""
    
    # ==========================================================================
    # BASELINE TESTS
    # ==========================================================================
    print_header "Baseline Network Metrics (Public IP: A → B)"
    
    print_step "Baseline ping ($NODE_A → $NODE_B)..."
    local baseline_ping_output
    baseline_ping_output=$(ssh_cmd "$NODE_A" "ping -c 5 -W 5 $NODE_B 2>&1" || echo "PING_FAILED")
    local baseline_latency="N/A"
    if ping_successful "$baseline_ping_output"; then
        baseline_latency=$(parse_ping_latency "$baseline_ping_output")
        echo -e "  ✅ Baseline Ping: ${YELLOW}${baseline_latency} ms${NC}"
    else
        echo -e "  ⚠️ Baseline ping failed"
    fi
    
    print_step "Baseline iperf3 throughput..."
    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null; nohup iperf3 -s -p 5201 > /tmp/iperf_baseline.log 2>&1 &"
    sleep 3
    
    local baseline_iperf_json
    baseline_iperf_json=$(ssh_cmd "$NODE_A" "iperf3 -c $NODE_B -p 5201 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
    
    local baseline_throughput_bps=$(echo "$baseline_iperf_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
    local baseline_throughput_mbps=$(echo "scale=2; $baseline_throughput_bps / 1000000" | bc 2>/dev/null || echo "N/A")
    
    if [[ "$baseline_throughput_mbps" != "N/A" && "$baseline_throughput_mbps" != "0" ]]; then
        echo -e "  ✅ Baseline Throughput: ${YELLOW}${baseline_throughput_mbps} Mbps${NC}"
    else
        baseline_throughput_mbps="N/A"
    fi
    
    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null" || true
    
    # ==========================================================================
    # VPN TUNNEL TESTS
    # ==========================================================================
    print_header "VPN Tunnel Metrics (A → B via Nucleus)"
    
    local avg_latency="N/A"
    local throughput_mbps="0"
    local ping_success=false
    
    if [[ -n "$VIP_A" && -n "$VIP_B" ]]; then
        print_step "Ping over tunnel ($VIP_A → $VIP_B)..."
        for attempt in 1 2 3; do
            echo "   Attempt $attempt/3..."
            local ping_output=$(ssh_cmd "$NODE_A" "ping -c 5 -W 5 $VIP_B 2>&1" || echo "PING_FAILED")
            if ping_successful "$ping_output"; then
                avg_latency=$(parse_ping_latency "$ping_output")
                echo -e "  ✅ Ping: ${YELLOW}${avg_latency} ms${NC}"
                ping_success=true
                break
            else
                echo "   Ping failed, retrying in 10s..."
                sleep 10
            fi
        done
        
        if [[ "$ping_success" == "true" ]]; then
            print_step "Starting iperf3 server on Edge B (VIP)..."
            ssh_cmd "$NODE_B" "nohup iperf3 -s -p 5201 > /tmp/iperf_server.log 2>&1 &"
            sleep 3
        
            print_step "Running iperf3 throughput test ($TEST_DURATION seconds)..."
            local iperf_json
            iperf_json=$(ssh_cmd "$NODE_A" "iperf3 -c $VIP_B -p 5201 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
            
            local throughput_bps=$(echo "$iperf_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
            throughput_mbps=$(echo "scale=2; $throughput_bps / 1000000" | bc 2>/dev/null || echo "0")
            
            if [[ "$throughput_mbps" != "0" ]]; then
                echo -e "  ✅ Throughput: ${YELLOW}${throughput_mbps} Mbps${NC}"
            else
                echo -e "  ❌ iperf3 test failed"
            fi
        fi
    else
        echo -e "  ⚠️ Skipping VPN tests - VIPs not available"
    fi
    
    # ==========================================================================
    # IPv6 TUNNEL TESTS
    # ==========================================================================
    local avg_latency_v6="N/A"
    local throughput_mbps_v6="N/A"
    
    if [[ "$TEST_IPV6" == "true" && "$ping_success" == "true" ]]; then
        print_header "IPv6 VPN Tunnel Metrics"
        
        VIP6_A=$(get_vip_with_retry "$NODE_A" "6")
        VIP6_B=$(get_vip_with_retry "$NODE_B" "6")
        
        if [[ -n "$VIP6_A" && -n "$VIP6_B" ]]; then
            echo "Edge A IPv6: $VIP6_A"
            echo "Edge B IPv6: $VIP6_B"
            
            print_step "IPv6 Ping ($VIP6_A → $VIP6_B)..."
            local ping6_output=$(ssh_cmd "$NODE_A" "ping -6 -c 5 -W 5 $VIP6_B 2>&1" || echo "PING_FAILED")
            if ping_successful "$ping6_output"; then
                avg_latency_v6=$(parse_ping_latency "$ping6_output")
                echo -e "  ✅ IPv6 Ping: ${YELLOW}${avg_latency_v6} ms${NC}"
                
                ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null; nohup iperf3 -s -p 5202 > /tmp/iperf6_server.log 2>&1 &"
                sleep 3
                
                local iperf6_json=$(ssh_cmd "$NODE_A" "iperf3 -6 -c $VIP6_B -p 5202 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
                local throughput6_bps=$(echo "$iperf6_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
                throughput_mbps_v6=$(echo "scale=2; $throughput6_bps / 1000000" | bc 2>/dev/null || echo "N/A")
                
                if [[ "$throughput_mbps_v6" != "N/A" && "$throughput_mbps_v6" != "0" ]]; then
                    echo -e "  ✅ IPv6 Throughput: ${YELLOW}${throughput_mbps_v6} Mbps${NC}"
                fi
            else
                echo -e "  ⚠️ IPv6 ping failed"
            fi
        else
            echo -e "  ⚠️ IPv6 VIPs not available"
        fi
    fi
    
    # Collect logs (CLI stdout + daemon file logs separately, then merge)
    print_step "Collecting logs..."
    # Edge A: CLI output (nohup capture) + daemon log file + journal
    ssh_cmd "$NODE_A" "cat /tmp/omni-edge-a.log" > "$RESULTS_DIR/edge_a_cli.log" 2>/dev/null || true
    ssh_cmd "$NODE_A" "sudo cat /root/.omniedge/logs/omniedge*.log 2>/dev/null" > "$RESULTS_DIR/edge_a_daemon.log" 2>/dev/null || true
    ssh_cmd "$NODE_A" "sudo journalctl -u omniedge -n 500 --no-pager 2>/dev/null" >> "$RESULTS_DIR/edge_a_daemon.log" || true
    { echo "=== CLI OUTPUT ==="; cat "$RESULTS_DIR/edge_a_cli.log" 2>/dev/null; echo ""; echo "=== DAEMON LOG ==="; cat "$RESULTS_DIR/edge_a_daemon.log" 2>/dev/null; } > "$RESULTS_DIR/edge_a.log" 2>/dev/null || true
    
    # Edge B: CLI output + daemon log file + journal
    ssh_cmd "$NODE_B" "cat /tmp/omni-edge-b.log" > "$RESULTS_DIR/edge_b_cli.log" 2>/dev/null || true
    ssh_cmd "$NODE_B" "sudo cat /root/.omniedge/logs/omniedge*.log 2>/dev/null" > "$RESULTS_DIR/edge_b_daemon.log" 2>/dev/null || true
    ssh_cmd "$NODE_B" "sudo journalctl -u omniedge -n 500 --no-pager 2>/dev/null" >> "$RESULTS_DIR/edge_b_daemon.log" || true
    { echo "=== CLI OUTPUT ==="; cat "$RESULTS_DIR/edge_b_cli.log" 2>/dev/null; echo ""; echo "=== DAEMON LOG ==="; cat "$RESULTS_DIR/edge_b_daemon.log" 2>/dev/null; } > "$RESULTS_DIR/edge_b.log" 2>/dev/null || true
    
    # Nucleus: daemon log file + journal
    ssh_cmd "$NUCLEUS" "sudo cat /root/.omniedge/logs/omniedge*.log 2>/dev/null || tail -100 /tmp/omni-nucleus.log 2>/dev/null" > "$RESULTS_DIR/nucleus.log" 2>/dev/null || true
    ssh_cmd "$NUCLEUS" "sudo journalctl -u omniedge -n 500 --no-pager 2>/dev/null" >> "$RESULTS_DIR/nucleus.log" || true
    
    # Report log sizes for debugging
    echo "  Log sizes:"
    for f in "$RESULTS_DIR"/edge_a_daemon.log "$RESULTS_DIR"/edge_b_daemon.log "$RESULTS_DIR"/nucleus.log; do
        if [ -f "$f" ]; then
            echo "    $(basename "$f"): $(wc -c < "$f") bytes, $(wc -l < "$f") lines"
        fi
    done
    
    # Determine connectivity type
    local connectivity_type="unknown"
    if grep -q "Disco pong received" "$RESULTS_DIR/edge_a.log" 2>/dev/null; then
        connectivity_type="direct_p2p"
    elif grep -q "Relay session established" "$RESULTS_DIR/edge_a.log" 2>/dev/null; then
        connectivity_type="relay"
    fi
    
    # Write results JSON
    cat > "$result_file" << EOF
{
  "timestamp": "$timestamp",
  "architecture": "3-node (Nucleus + Edge A + Edge B)",
  "nucleus": "$NUCLEUS:$NUCLEUS_PORT",
  "nucleus_vip": "${VIP_NUCLEUS:-N/A}",
  "network_id": "$NETWORK_ID",
  "edge_a": {"public_ip": "$NODE_A", "vip": "${VIP_A:-N/A}", "vip6": "${VIP6_A:-N/A}"},
  "edge_b": {"public_ip": "$NODE_B", "vip": "${VIP_B:-N/A}", "vip6": "${VIP6_B:-N/A}"},
  "connectivity_type": "$connectivity_type",
  "test_duration_sec": $TEST_DURATION,
  "baseline": {
    "ping_ms": "$baseline_latency",
    "throughput_mbps": "$baseline_throughput_mbps"
  },
  "vpn_tunnel_ipv4": {
    "ping_ms": "$avg_latency",
    "throughput_mbps": "$throughput_mbps"
  },
  "vpn_tunnel_ipv6": {
    "ping_ms": "${avg_latency_v6:-N/A}",
    "throughput_mbps": "${throughput_mbps_v6:-N/A}"
  }
}
EOF
    
    # Summary
    print_header "Test Complete"
    
    echo -e "┌─────────────────────────────────────────────────────────┐"
    echo -e "│  ${GREEN}3-NODE OMNIEDGE TEST RESULTS${NC}                            │"
    echo -e "├─────────────────────────────────────────────────────────┤"
    echo -e "│  Nucleus:   $NUCLEUS → ${VIP_NUCLEUS:-N/A}"
    echo -e "│  Edge A:    $NODE_A → ${VIP_A:-N/A}"
    echo -e "│  Edge B:    $NODE_B → ${VIP_B:-N/A}"
    echo -e "│  Connectivity: ${connectivity_type}"
    echo -e "├─────────────────────────────────────────────────────────┤"
    echo -e "│  ${CYAN}BASELINE (Public IP)${NC}                                    │"
    echo -e "│    Latency:    ${YELLOW}${baseline_latency} ms${NC}"
    echo -e "│    Throughput: ${YELLOW}${baseline_throughput_mbps} Mbps${NC}"
    echo -e "├─────────────────────────────────────────────────────────┤"
    echo -e "│  ${CYAN}VPN TUNNEL (IPv4)${NC}                                       │"
    echo -e "│    Latency:    ${YELLOW}${avg_latency} ms${NC}"
    echo -e "│    Throughput: ${YELLOW}${throughput_mbps} Mbps${NC}"
    echo -e "├─────────────────────────────────────────────────────────┤"
    echo -e "│  ${CYAN}VPN TUNNEL (IPv6)${NC}                                       │"
    echo -e "│    Latency:    ${YELLOW}${avg_latency_v6:-N/A} ms${NC}"
    echo -e "│    Throughput: ${YELLOW}${throughput_mbps_v6:-N/A} Mbps${NC}"
    echo -e "└─────────────────────────────────────────────────────────┘"
    echo ""
    echo -e "Results saved to: ${CYAN}$result_file${NC}"
    echo -e "Logs saved to:    ${CYAN}$RESULTS_DIR/*.log${NC}"

    # Cleanup edge processes (not nucleus)
    print_step "Cleaning up edge processes..."
    for node in "$NODE_A" "$NODE_B"; do
        ssh_cmd "$node" "sudo pkill -9 -f 'omniedge.*--daemon' || true; \
                         sleep 1; \
                         sudo pkill -f omniedge || true; \
                         sudo pkill -f iperf3 || true" 2>/dev/null || true
    done
}

# =============================================================================
# Main
# =============================================================================

SKIP_DEPLOY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --nucleus) NUCLEUS="$2"; shift 2 ;;
        --nucleus-port) NUCLEUS_PORT="$2"; shift 2 ;;
        --node-a) NODE_A="$2"; shift 2 ;;
        --node-b) NODE_B="$2"; shift 2 ;;
        --ssh-key) SSH_KEY="$2"; shift 2 ;;
        --ssh-user) SSH_USER="$2"; shift 2 ;;
        --ssh-user-nucleus) SSH_USER_NUCLEUS="$2"; shift 2 ;;
        --ssh-user-a) SSH_USER_A="$2"; shift 2 ;;
        --ssh-user-b) SSH_USER_B="$2"; shift 2 ;;
        --network) NETWORK_ID="$2"; shift 2 ;;
        --key) SECURITY_KEY="$2"; shift 2 ;;
        --duration) TEST_DURATION="$2"; shift 2 ;;
        --skip-deploy) SKIP_DEPLOY=true; shift ;;
        --use-local-bin) USE_LOCAL_BIN=true; shift ;;
        --use-local-cli) USE_LOCAL_CLI=true; shift ;;
        --local-docker) LOCAL_DOCKER=true; shift ;;
        --no-ipv6) TEST_IPV6=false; shift ;;
        --help|-h) show_help; exit 0 ;;
        *) print_error "Unknown option: $1"; show_help; exit 1 ;;
    esac
done

NETWORK_ID="${NETWORK_ID:-$OMNIEDGE_NETWORK_ID}"
SECURITY_KEY="${SECURITY_KEY:-$OMNIEDGE_SECURITY_KEY}"

# Set per-node SSH users (fallback to global SSH_USER)
SSH_USER_NUCLEUS="${SSH_USER_NUCLEUS:-$SSH_USER}"
SSH_USER_A="${SSH_USER_A:-$SSH_USER}"
SSH_USER_B="${SSH_USER_B:-$SSH_USER}"

# Validate mutual exclusivity
if [[ "$USE_LOCAL_CLI" == "true" && "$LOCAL_DOCKER" == "true" ]]; then
    print_error "--use-local-cli and --local-docker are mutually exclusive"
    exit 1
fi

# Check for localhost with --use-local-cli
if [[ "$USE_LOCAL_CLI" == "true" ]]; then
    if is_local "$NODE_A" || is_local "$NODE_B"; then
        echo -e "${CYAN}Note: --use-local-cli mode - localhost will run natively${NC}"
    fi
fi

# Validate required arguments
if [[ -z "$NUCLEUS" || -z "$NODE_A" || -z "$NODE_B" || -z "$NETWORK_ID" || -z "$SECURITY_KEY" ]]; then
    print_error "Missing required arguments"
    show_help
    exit 1
fi

print_header "OmniEdge 3-Node Cloud Test"
echo "Nucleus:   $NUCLEUS:$NUCLEUS_PORT (user: $SSH_USER_NUCLEUS)"
echo "Edge A:    $NODE_A (user: $SSH_USER_A)"
echo "Edge B:    $NODE_B (user: $SSH_USER_B)"
echo "Network:   $NETWORK_ID"

preflight_check
install_dependencies
if ! $SKIP_DEPLOY; then deploy_omniedge; fi
run_test

echo -e "\n${GREEN}✅ 3-Node OmniEdge cloud test completed!${NC}"
