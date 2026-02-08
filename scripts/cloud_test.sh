#!/bin/bash
# =============================================================================
# OmniEdge Cloud-to-Cloud Test Orchestrator (v2.6.0)
# Run from LOCAL machine, orchestrates tests between cloud instances
# Architecture: 2-Node P2P (Edge A + Edge B via OmniEdge Backend)
#
# Features:
#   - Automatic dependency installation (iperf3, iproute2, netperf)
#   - Automatic OmniEdge installation via installer script
#   - Security key authentication
#   - Baseline vs VPN performance comparison
#   - IPv4 and IPv6 tunnel testing
#   - Localhost support via Docker (simulates Linux environment)
#
# Example:
# ./scripts/cloud_test.sh --node-a 54.x.x.x --node-b 35.x.x.x \
#    --network abc123 --key sk_xxx --ssh-key ~/.ssh/cloud.pem
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

NODE_A=""
NODE_B=""
SSH_KEY=""
SSH_USER="${SSH_USER:-ubuntu}"
NETWORK_ID=""
SECURITY_KEY=""
TEST_DURATION=${TEST_DURATION:-10}
RESULTS_DIR="./test_results"

# Virtual IPs are assigned by OmniEdge backend
VIP_A=""
VIP_B=""
VIP6_A=""
VIP6_B=""
TEST_IPV6=true

# Installer URL (default)
INSTALLER_URL="${INSTALLER_URL:-https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh}"
LOCAL_DOCKER=false
LOCAL_DOCKER_NAME="omni-node-local"

show_help() {
    cat << EOF
OmniEdge 2-Node Cloud Test Orchestrator

Architecture:
   ┌──────────────────┐                    ┌──────────────────┐
   │     Edge A       │◄───── P2P VPN ────►│     Edge B       │
   │  (via OmniEdge)  │                    │  (via OmniEdge)  │
   └──────────────────┘                    └──────────────────┘
           ▲                                        ▲
           │            OmniEdge Backend            │
           └──────────────────┬─────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │  Signaling/Relay  │
                    │  (Cloud Managed)  │
                    └───────────────────┘

Usage:
  $0 --node-a <IP> --node-b <IP> --network <NETWORK_ID> --key <SECURITY_KEY> [OPTIONS]

Required:
  --node-a        IP address of Edge A (cloud server or "localhost")
  --node-b        IP address of Edge B (cloud server or "localhost")
  --network       OmniEdge Virtual Network ID (from dashboard)
  --key           OmniEdge Security Key (from dashboard)

Options:
  --ssh-key       Path to SSH private key
  --ssh-user      SSH username (default: ubuntu)
  --duration      iperf3 test duration in seconds (default: 10)
  --no-ipv6       Skip IPv6 tests
  --skip-deploy   Skip OmniEdge installation (use existing)
  --local-docker  Run local nodes (localhost/127.0.0.1) in Docker containers
  --help          Show this help

Environment Variables:
  SSH_USER                SSH username
  TEST_DURATION           iperf3 test duration
  OMNIEDGE_NETWORK_ID     Virtual Network ID (fallback)
  OMNIEDGE_SECURITY_KEY   Security Key (fallback)

Example:
  $0 --node-a 54.x.x.x --node-b localhost \\
     --network abc123 --key sk_xxx \\
     --local-docker --ssh-key ~/.ssh/cloud.pem

Prerequisites:
   - SSH access with key authentication to cloud nodes
   - Docker installed (if using --local-docker)
   - Root/sudo access for TUN interface creation
   - Ports: UDP 51820 (WireGuard), TCP 5201-5202 (iperf3)
   - OmniEdge Network ID and Security Key from dashboard
EOF
}

# =============================================================================
# Host Helper Functions
# =============================================================================

# Get VIP with retry logic - tries omniedge status first, then falls back to ip addr
get_vip_with_retry() {
    local node="$1"
    local ip_version="${2:-4}"  # 4 or 6
    local max_attempts=12
    local attempt=1
    local vip=""
    
    export CURRENT_TARGET_NODE="$node"
    
    while [[ $attempt -le $max_attempts && -z "$vip" ]]; do
        # Method 1: Try omniedge status --json first (most reliable)
        if [[ "$ip_version" == "4" ]]; then
            vip=$(ssh_cmd "$node" "omniedge status --json 2>/dev/null | jq -r '.vip // empty'" 2>/dev/null || echo "")
        else
            vip=$(ssh_cmd "$node" "omniedge status --json 2>/dev/null | jq -r '.vip6 // empty'" 2>/dev/null || echo "")
        fi
        
        # Method 2: Fall back to ip addr if omniedge status doesn't work
        if [[ -z "$vip" || "$vip" == "null" ]]; then
            # First check if interface exists
            if ssh_cmd "$node" "ip link show omni0" &>/dev/null; then
                if [[ "$ip_version" == "4" ]]; then
                    vip=$(ssh_cmd "$node" "ip addr show omni0 2>/dev/null | grep 'inet ' | awk '{print \$2}' | cut -d/ -f1 | head -1" || echo "")
                else
                    vip=$(ssh_cmd "$node" "ip -6 addr show omni0 2>/dev/null | grep 'inet6' | grep -v 'fe80' | awk '{print \$2}' | cut -d/ -f1 | head -1" || echo "")
                fi
            fi
        fi
        
        if [[ -z "$vip" || "$vip" == "null" ]]; then
            echo "   Attempt $attempt/$max_attempts: Waiting for omni0 interface on $node..." >&2
            sleep 5
        fi
        attempt=$((attempt + 1))
    done
    
    unset CURRENT_TARGET_NODE
    
    # Return empty string instead of "null"
    if [[ "$vip" == "null" ]]; then
        echo ""
    else
        echo "$vip"
    fi
}

# Verify omni0 interface exists and has IP
verify_omniedge_interface() {
    local node="$1"
    export CURRENT_TARGET_NODE="$node"
    
    # Check if interface exists
    if ! ssh_cmd "$node" "ip link show omni0" &>/dev/null; then
        echo "  ❌ omni0 interface does not exist on $node" >&2
        unset CURRENT_TARGET_NODE
        return 1
    fi
    
    # Check if interface has an IP
    local has_ip=$(ssh_cmd "$node" "ip addr show omni0 2>/dev/null | grep -c 'inet '" || echo "0")
    if [[ "$has_ip" == "0" ]]; then
        echo "  ⚠️ omni0 interface exists but has no IP on $node" >&2
        unset CURRENT_TARGET_NODE
        return 1
    fi
    
    echo "  ✅ omni0 interface verified on $node" >&2
    unset CURRENT_TARGET_NODE
    return 0
}

is_local() {
    local host="$1"
    if [[ "$host" == "localhost" || "$host" == "127.0.0.1" ]]; then
        return 0
    fi
    # Check if host is one of our local IPs (macOS/Linux compatible)
    if command -v ifconfig &>/dev/null; then
        if ifconfig | grep -q "$host"; then return 0; fi
    elif command -v ip &>/dev/null; then
        if ip addr | grep -q "$host"; then return 0; fi
    fi
    return 1
}

ssh_cmd() {
    local host="$1"
    shift
    if is_local "$host"; then
        if [[ "$LOCAL_DOCKER" == "true" ]]; then
            # Run in local Docker container (single container like OmniNervous)
            # Strip 'sudo ' prefix if present, as we are already root in Docker
            local cmd="$*"
            if [[ "$cmd" == sudo\ * ]]; then
                cmd="${cmd#sudo }"
            fi
            # Also handle the case where sudo is used in a pipe, e.g., "curl ... | sudo bash"
            # This is a bit more complex, but we can replace "sudo " with "" globally for simple cases
            cmd=$(echo "$cmd" | sed 's/\bsudo //g')
            docker exec -t "$LOCAL_DOCKER_NAME" sh -c "$cmd"
        else
            # Native local execution
            sudo sh -c "$*"
        fi
    else
        ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
            ${SSH_KEY:+-i "$SSH_KEY"} \
            "$SSH_USER@$host" "$@"
    fi
}

scp_to() {
    local src="$1"
    local host="$2"
    local dest="$3"
    if is_local "$host"; then
        if [[ "$LOCAL_DOCKER" == "true" ]]; then
            # Resolve ~/ to /root inside the container
            local container_dest="${dest/#\~//root}"
            docker exec "$LOCAL_DOCKER_NAME" mkdir -p "$(dirname "$container_dest")"
            docker cp "$src" "$LOCAL_DOCKER_NAME:$container_dest"
        else
            # Native local copy
            local real_dest="${dest/#\~/$HOME}"
            mkdir -p "$(dirname "$real_dest")"
            cp "$src" "$real_dest"
        fi
    else
        scp -o StrictHostKeyChecking=no \
            ${SSH_KEY:+-i "$SSH_KEY"} \
            "$src" "$SSH_USER@$host:$dest"
    fi
}

ensure_local_docker() {
    if [[ "$LOCAL_DOCKER" != "true" ]]; then return 0; fi
    
    if ! docker ps --format '{{.Names}}' | grep -q "^$LOCAL_DOCKER_NAME$"; then
        print_step "Setting up local Docker environment ($LOCAL_DOCKER_NAME)..."
        docker rm -f "$LOCAL_DOCKER_NAME" 2>/dev/null || true
        
        # Use a lightweight ubuntu image with necessary tools
        docker run -d --name "$LOCAL_DOCKER_NAME" \
            --privileged \
            --cap-add=NET_ADMIN \
            --device /dev/net/tun:/dev/net/tun \
            ubuntu:24.04 sleep infinity
            
        print_step "Installing dependencies in local Docker container..."
        docker exec "$LOCAL_DOCKER_NAME" apt-get update -qq
        # Added ca-certificates for curl to work with HTTPS
        docker exec "$LOCAL_DOCKER_NAME" apt-get install -y -qq iperf3 wireguard-tools iproute2 jq bc psmisc curl ca-certificates sudo iputils-ping procps
        
        # Create root config directory
        docker exec "$LOCAL_DOCKER_NAME" mkdir -p /root/.omniedge
        
        # Verify TUN device is available in container
        print_step "Verifying TUN device in container..."
        if docker exec "$LOCAL_DOCKER_NAME" ls -la /dev/net/tun &>/dev/null; then
            echo -e "  ✅ TUN device available"
        else
            # Try to create TUN device if it doesn't exist
            echo -e "  ⚠️ TUN device not found, attempting to create..."
            docker exec "$LOCAL_DOCKER_NAME" mkdir -p /dev/net
            docker exec "$LOCAL_DOCKER_NAME" mknod /dev/net/tun c 10 200
            docker exec "$LOCAL_DOCKER_NAME" chmod 600 /dev/net/tun
            if docker exec "$LOCAL_DOCKER_NAME" ls -la /dev/net/tun &>/dev/null; then
                echo -e "  ✅ TUN device created successfully"
            else
                print_error "Failed to create TUN device - VPN may not work"
            fi
        fi
    fi
}

# =============================================================================
# Pre-flight Checks
# =============================================================================

preflight_check() {
    print_header "Pre-flight Checks"
    
    # Ensure local Docker if requested
    ensure_local_docker
    
    local errors=0
    
    # Check for local dependencies
    print_step "Checking local dependencies..."
    local deps="ssh scp jq bc curl"
    if [[ "$LOCAL_DOCKER" == "true" ]]; then
        deps="$deps docker"
    fi
    for cmd in $deps; do
        if which "$cmd" &>/dev/null; then
            echo -e "  ✅ Local $cmd found"
        else
            echo -e "  ❌ Local $cmd NOT found. Please install it."
            errors=$((errors + 1))
        fi
    done
    
    # Check connectivity
    for node in "$NODE_A" "$NODE_B"; do
        export CURRENT_TARGET_NODE="$node"
        print_step "Testing connectivity to $node..."
        if ssh_cmd "$node" "echo ok" &>/dev/null; then
            echo -e "✅ Connectivity to $node successful"
        else
            echo -e "❌ Connectivity to $node failed"
            errors=$((errors + 1))
        fi
    done
    unset CURRENT_TARGET_NODE
    
    # Check sudo/root on edge nodes
    print_step "Checking root/sudo access on edge nodes..."
    for node in "$NODE_A" "$NODE_B"; do
        export CURRENT_TARGET_NODE="$node"
        if ssh_cmd "$node" "sudo -n true" &>/dev/null; then
            echo -e "  ✅ Root/Sudo access available on $node"
        else
            if is_local "$node" && [[ "$LOCAL_DOCKER" == "true" ]]; then
                echo -e "  ✅ Root access available in Docker container on $node"
            else
                echo -e "  ⚠️ Sudo might require password on $node (script may hang)"
            fi
        fi
    done
    unset CURRENT_TARGET_NODE
    
    # Check networking tools on edge nodes (will be installed if missing)
    print_step "Checking networking tools on edge nodes..."
    for node in "$NODE_A" "$NODE_B"; do
        export CURRENT_TARGET_NODE="$node"
        local node_os=$(ssh_cmd "$node" "uname")
        if [[ "$node_os" == "Darwin" ]]; then
            echo -e "  🍏 macOS detected on $node (using native networking)"
            unset CURRENT_TARGET_NODE
            continue
        fi
        
        if ssh_cmd "$node" "which iperf3" &>/dev/null; then
            echo -e "  ✅ iperf3 installed on $node"
        else
            echo -e "  ⚠️ iperf3 not installed on $node (will be installed)"
        fi
        
        if ssh_cmd "$node" "which ip" &>/dev/null; then
            echo -e "  ✅ iproute2 installed on $node"
        else
            echo -e "  ⚠️ iproute2 not installed on $node (will be installed)"
        fi
        unset CURRENT_TARGET_NODE
    done

    if [[ $errors -gt 0 ]]; then
        print_error "Pre-flight checks failed with $errors errors"
        exit 1
    fi
    
    echo -e "\n${GREEN}All pre-flight checks passed!${NC}"
}

# =============================================================================
# Install Missing Dependencies
# =============================================================================

install_dependencies() {
    print_header "Installing Missing Dependencies"
    
    for node in "$NODE_A" "$NODE_B"; do
        export CURRENT_TARGET_NODE="$node"
        print_step "Checking and installing dependencies on $node..."
        
        # Detect remote platform
        local remote_os=$(ssh_cmd "$node" "uname")
        if [[ "$remote_os" == "Darwin" ]]; then
            echo -e "  🍏 macOS detected on $node, assuming dependencies installed via Homebrew"
            unset CURRENT_TARGET_NODE
            continue
        fi

        # Detect package manager (Linux)
        local pkg_manager=""
        if ssh_cmd "$node" "which apt-get" &>/dev/null; then
            pkg_manager="apt"
        elif ssh_cmd "$node" "which dnf" &>/dev/null; then
            pkg_manager="dnf"
        elif ssh_cmd "$node" "which yum" &>/dev/null; then
            pkg_manager="yum"
        else
            echo -e "  ⚠️ Unknown package manager on $node, skipping auto-install"
            unset CURRENT_TARGET_NODE
            continue
        fi
        echo -e "  📦 Detected package manager: $pkg_manager"
        
        # Update package lists (apt only)
        if [[ "$pkg_manager" == "apt" ]]; then
            echo -e "  📥 Updating package lists..."
            ssh_cmd "$node" "sudo apt-get update -qq" || true
        fi
        
        # Install netperf (optional, for latency testing)
        if ! ssh_cmd "$node" "which netperf" &>/dev/null; then
            echo -e "  📥 Installing netperf..."
            case $pkg_manager in
                apt)
                    ssh_cmd "$node" "sudo apt-get install -y -qq netperf" || echo "  ⚠️ netperf not available"
                    ;;
                dnf|yum)
                    ssh_cmd "$node" "sudo $pkg_manager install -y netperf" || echo "  ⚠️ netperf not available"
                    ;;
            esac
        else
            echo -e "  ✅ netperf already installed"
        fi

        # Install iproute2, jq, bc, psmisc (for fuser)
        echo -e "  📥 Installing utility tools..."
        ssh_cmd "$node" "which ip &>/dev/null || (sudo $pkg_manager install -y iproute2 || sudo $pkg_manager install -y iproute || true)" || true
        ssh_cmd "$node" "which jq &>/dev/null || (sudo $pkg_manager install -y jq || true)" || true
        ssh_cmd "$node" "which bc &>/dev/null || (sudo $pkg_manager install -y bc || true)" || true
        ssh_cmd "$node" "which fuser &>/dev/null || (sudo $pkg_manager install -y psmisc || true)" || true
        ssh_cmd "$node" "which pkill &>/dev/null || (sudo $pkg_manager install -y procps || true)" || true
        ssh_cmd "$node" "which curl &>/dev/null || (sudo $pkg_manager install -y curl || true)" || true
        
        echo -e "  ✅ Dependencies installed on $node"
        unset CURRENT_TARGET_NODE
    done
    
    echo -e "\n${GREEN}Dependency installation complete!${NC}"
}

# =============================================================================
# Deploy OmniEdge via Installer Script
# =============================================================================

deploy_omniedge() {
    print_header "Deploying OmniEdge"
    
    echo -e "📦 Installing OmniEdge via installer script..."
    echo -e "   ${CYAN}$INSTALLER_URL${NC}"
    
    for node in "$NODE_A" "$NODE_B"; do
        print_step "Installing OmniEdge on $node..."
        
        # Check if already installed
        if ssh_cmd "$node" "which omniedge" &>/dev/null; then
            local existing_version
            existing_version=$(ssh_cmd "$node" "omniedge --version 2>/dev/null | head -1" || echo "unknown")
            echo -e "  ℹ️ OmniEdge already installed: $existing_version"
            echo -e "  🔄 Reinstalling to ensure latest version..."
        fi
        
        # Run the install script on the remote node
        ssh_cmd "$node" "curl -fsSL $INSTALLER_URL | sudo bash"
        
        # Verify installation
        if ssh_cmd "$node" "which omniedge" &>/dev/null; then
            local version
            version=$(ssh_cmd "$node" "omniedge --version 2>/dev/null | head -1" || echo "unknown")
            echo -e "  ✅ OmniEdge installed on $node ($version)"
        else
            print_error "Installation failed on $node"
            exit 1
        fi
    done
    
    echo -e "\n${GREEN}OmniEdge installation complete!${NC}"
}

# =============================================================================
# Run Test
# =============================================================================

run_test() {
    print_header "Running 2-Node OmniEdge VPN Test"
    
    # Create local results directory
    mkdir -p "$RESULTS_DIR"
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)
    local result_file="$RESULTS_DIR/cloud_test_$timestamp.json"
    
    # Validate required parameters
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

    # Kill any existing processes and clean up
    print_step "Cleaning up old processes and logs..."
    for node in "$NODE_A" "$NODE_B"; do
        export CURRENT_TARGET_NODE="$node"
        ssh_cmd "$node" "sudo pkill -9 -f omniedge 2>/dev/null; \
                         sudo pkill -9 -f iperf3 2>/dev/null; \
                         if command -v fuser &>/dev/null; then sudo fuser -k 51820/udp 2>/dev/null; fi; \
                         sudo rm -f /tmp/omni-*.log" || true
        unset CURRENT_TARGET_NODE
    done
    
    # Give it time to release sockets
    echo "   Waiting for resource release..."
    sleep 5
    
    # Start Edge A
    print_step "Starting Edge A on $NODE_A..."
    export CURRENT_TARGET_NODE="$NODE_A"
    ssh_cmd "$NODE_A" "sudo sh -c \"nohup omniedge start -n $NETWORK_ID -s $SECURITY_KEY > /tmp/omni-edge-a.log 2>&1 &\" < /dev/null"
    unset CURRENT_TARGET_NODE
    sleep 3

    # Start Edge B
    print_step "Starting Edge B on $NODE_B..."
    export CURRENT_TARGET_NODE="$NODE_B"
    ssh_cmd "$NODE_B" "sudo sh -c \"nohup omniedge start -n $NETWORK_ID -s $SECURITY_KEY > /tmp/omni-edge-b.log 2>&1 &\" < /dev/null"
    unset CURRENT_TARGET_NODE
    sleep 3
    
    # Wait for VPN tunnel establishment
    print_step "Waiting for VPN tunnel establishment (60s for peer discovery)..."
    echo "   This includes authentication, peer discovery, and WireGuard configuration."
    sleep 60
    
    # Check if daemons are running
    print_step "Checking daemon processes..."
    echo "Edge A process:"
    export CURRENT_TARGET_NODE="$NODE_A"
    ssh_cmd "$NODE_A" "pgrep -a omniedge || echo 'NOT RUNNING'"
    unset CURRENT_TARGET_NODE
    echo "Edge B process:"
    export CURRENT_TARGET_NODE="$NODE_B"
    ssh_cmd "$NODE_B" "pgrep -a omniedge || echo 'NOT RUNNING'"
    unset CURRENT_TARGET_NODE
    echo ""
    
    # Get VIPs from interface with retry logic
    print_step "Getting VPN IPs from interfaces (with retry)..."
    VIP_A=$(get_vip_with_retry "$NODE_A" "4")
    VIP_B=$(get_vip_with_retry "$NODE_B" "4")
    
    echo "Edge A VPN IP: ${VIP_A:-'(not assigned)'}"
    echo "Edge B VPN IP: ${VIP_B:-'(not assigned)'}"
    
    # Verify interfaces exist
    print_step "Verifying OmniEdge interfaces..."
    verify_omniedge_interface "$NODE_A" || true
    verify_omniedge_interface "$NODE_B" || true
    
    # Show logs for debugging
    print_step "Daemon logs (last 15 lines)..."
    echo "--- Edge A log ---"
    export CURRENT_TARGET_NODE="$NODE_A"
    ssh_cmd "$NODE_A" "tail -15 /tmp/omni-edge-a.log 2>/dev/null || echo 'No log available'"
    
    # Check for P2P connection success (like OmniNervous)
    if ssh_cmd "$NODE_A" "grep -i 'p2p' /tmp/omni-edge-a.log | grep -i 'established' &>/dev/null"; then
        echo -e "  ✅ ${GREEN}Direct P2P Link Established${NC} on Edge A"
    elif ssh_cmd "$NODE_A" "grep -i 'relay' /tmp/omni-edge-a.log | grep -i 'active' &>/dev/null"; then
        echo -e "  ⚠️ ${YELLOW}Relay Fallback Active${NC} on Edge A"
    fi
    unset CURRENT_TARGET_NODE

    echo ""
    echo "--- Edge B log ---"
    export CURRENT_TARGET_NODE="$NODE_B"
    ssh_cmd "$NODE_B" "tail -15 /tmp/omni-edge-b.log 2>/dev/null || echo 'No log available'"
    
    if ssh_cmd "$NODE_B" "grep -i 'p2p' /tmp/omni-edge-b.log | grep -i 'established' &>/dev/null"; then
        echo -e "  ✅ ${GREEN}Direct P2P Link Established${NC} on Edge B"
    fi
    unset CURRENT_TARGET_NODE
    echo ""
    
    # ==========================================================================
    # BASELINE TESTS: Public IP (before VPN comparison)
    # ==========================================================================
    print_header "Baseline Network Metrics (Public IP: A → B)"
    echo "   These tests use public IPs WITHOUT the VPN tunnel."
    echo ""
    
    # Baseline ping test
    print_step "Baseline ping over public IP ($NODE_A → $NODE_B)..."
    export CURRENT_TARGET_NODE="$NODE_A"
    local baseline_ping_output
    baseline_ping_output=$(ssh_cmd "$NODE_A" "ping -c 5 -W 5 $NODE_B 2>&1" || echo "PING_FAILED")
    local baseline_latency="N/A"
    if echo "$baseline_ping_output" | grep -q "rtt"; then
        baseline_latency=$(echo "$baseline_ping_output" | grep "rtt" | awk -F'/' '{print $5}')
        echo -e "  ✅ Baseline Ping: ${YELLOW}${baseline_latency} ms${NC}"
    else
        echo -e "  ⚠️ Baseline ping failed"
    fi
    unset CURRENT_TARGET_NODE
    
    # Baseline iperf3 test
    print_step "Starting iperf3 server on Edge B (public IP)..."
    export CURRENT_TARGET_NODE="$NODE_B"
    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null; nohup iperf3 -s -p 5201 > /tmp/iperf_baseline.log 2>&1 &"
    sleep 3
    unset CURRENT_TARGET_NODE
    
    print_step "Baseline iperf3 throughput test ($TEST_DURATION seconds)..."
    export CURRENT_TARGET_NODE="$NODE_A"
    local baseline_iperf_json
    baseline_iperf_json=$(ssh_cmd "$NODE_A" "iperf3 -c $NODE_B -p 5201 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
    
    local baseline_throughput_bps=$(echo "$baseline_iperf_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
    local baseline_throughput_mbps=$(echo "scale=2; $baseline_throughput_bps / 1000000" | bc 2>/dev/null || echo "N/A")
    
    if [[ "$baseline_throughput_mbps" != "N/A" && "$baseline_throughput_mbps" != "0" ]]; then
        echo -e "  ✅ Baseline Throughput: ${YELLOW}${baseline_throughput_mbps} Mbps${NC}"
    else
        echo -e "  ⚠️ Baseline iperf3 failed"
        baseline_throughput_mbps="N/A"
    fi
    unset CURRENT_TARGET_NODE
    
    export CURRENT_TARGET_NODE="$NODE_B"
    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null" || true
    unset CURRENT_TARGET_NODE
    
    # ==========================================================================
    # VPN TUNNEL TESTS
    # ==========================================================================

    print_header "VPN Tunnel Metrics (OmniEdge: A → B)"
    echo "   These tests use VPN IPs ($VIP_A → $VIP_B) over encrypted tunnel."
    echo ""
    
    local avg_latency="N/A"
    local throughput_mbps="0"
    local ping_success=false
    
    if [[ -n "$VIP_A" && -n "$VIP_B" ]]; then
        print_step "Ping over tunnel ($VIP_A → $VIP_B) with retries..."
        export CURRENT_TARGET_NODE="$NODE_A"
        for attempt in 1 2 3; do
            echo "   Attempt $attempt/3..."
            local ping_output=$(ssh_cmd "$NODE_A" "ping -c 5 -W 5 $VIP_B 2>&1" || echo "PING_FAILED")
            if echo "$ping_output" | grep -q "rtt"; then
                avg_latency=$(echo "$ping_output" | grep "rtt" | awk -F'/' '{print $5}')
                echo -e "  ✅ Ping: ${YELLOW}${avg_latency} ms${NC}"
                ping_success=true
                break
            else
                echo "   Ping failed, retrying in 10s..."
                sleep 10
            fi
        done
        unset CURRENT_TARGET_NODE
        
        if [[ "$ping_success" == "true" ]]; then
            print_step "Starting iperf3 server on Edge B..."
            export CURRENT_TARGET_NODE="$NODE_B"
            ssh_cmd "$NODE_B" "nohup iperf3 -s -p 5201 > /tmp/iperf_server.log 2>&1 &"
            sleep 3
            unset CURRENT_TARGET_NODE
        
            print_step "Running iperf3 throughput test ($TEST_DURATION seconds) over tunnel..."
            export CURRENT_TARGET_NODE="$NODE_A"
            local iperf_json
            iperf_json=$(ssh_cmd "$NODE_A" "iperf3 -c $VIP_B -p 5201 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
            
            local throughput_bps=$(echo "$iperf_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
            throughput_mbps=$(echo "scale=2; $throughput_bps / 1000000" | bc 2>/dev/null || echo "N/A")
            
            if [[ "$throughput_mbps" != "N/A" && "$throughput_mbps" != "0" ]]; then
                echo -e "  ✅ Throughput: ${YELLOW}${throughput_mbps} Mbps${NC}"
            else
                echo -e "  ❌ iperf3 test failed"
                throughput_mbps="0"
            fi
            unset CURRENT_TARGET_NODE
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
        print_header "IPv6 VPN Tunnel Metrics (OmniEdge: A → B)"
        echo ""
        
        # Use retry logic for IPv6 VIP detection
        print_step "Getting IPv6 VPN IPs (with retry)..."
        VIP6_A=$(get_vip_with_retry "$NODE_A" "6")
        VIP6_B=$(get_vip_with_retry "$NODE_B" "6")
        
        if [[ -n "$VIP6_A" && -n "$VIP6_B" ]]; then
            echo "Edge A IPv6: $VIP6_A"
            echo "Edge B IPv6: $VIP6_B"
            
            print_step "IPv6 Ping over tunnel ($VIP6_A → $VIP6_B)..."
            export CURRENT_TARGET_NODE="$NODE_A"
            local ping6_output=$(ssh_cmd "$NODE_A" "ping -6 -c 5 -W 5 $VIP6_B 2>&1" || echo "PING_FAILED")
            if echo "$ping6_output" | grep -q "rtt"; then
                avg_latency_v6=$(echo "$ping6_output" | grep "rtt" | awk -F'/' '{print $5}')
                echo -e "  ✅ IPv6 Ping: ${YELLOW}${avg_latency_v6} ms${NC}"
                
                print_step "Starting iperf3 server on Edge B (IPv6)..."
                export CURRENT_TARGET_NODE="$NODE_B"
                ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null; nohup iperf3 -s -p 5202 > /tmp/iperf6_server.log 2>&1 &"
                sleep 3
                unset CURRENT_TARGET_NODE
                
                print_step "Running IPv6 iperf3 throughput test ($TEST_DURATION seconds)..."
                export CURRENT_TARGET_NODE="$NODE_A"
                local iperf6_json=$(ssh_cmd "$NODE_A" "iperf3 -6 -c $VIP6_B -p 5202 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
                local throughput6_bps=$(echo "$iperf6_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
                throughput_mbps_v6=$(echo "scale=2; $throughput6_bps / 1000000" | bc 2>/dev/null || echo "N/A")
                
                if [[ "$throughput_mbps_v6" != "N/A" && "$throughput_mbps_v6" != "0" ]]; then
                    echo -e "  ✅ IPv6 Throughput: ${YELLOW}${throughput_mbps_v6} Mbps${NC}"
                else
                    echo -e "  ⚠️ IPv6 iperf3 test failed"
                fi
                unset CURRENT_TARGET_NODE
            else
                echo -e "  ⚠️ IPv6 ping failed"
            fi
            unset CURRENT_TARGET_NODE
        else
            echo -e "  ⚠️ IPv6 VIPs not available (Edge A: ${VIP6_A:-'none'}, Edge B: ${VIP6_B:-'none'})"
        fi
    fi
    
    # Collect logs and store JSON
    print_step "Collecting logs..."
    export CURRENT_TARGET_NODE="$NODE_A"
    ssh_cmd "$NODE_A" "cat /tmp/omni-edge-a.log" > "$RESULTS_DIR/edge_a.log" 2>/dev/null || true
    unset CURRENT_TARGET_NODE
    export CURRENT_TARGET_NODE="$NODE_B"
    ssh_cmd "$NODE_B" "cat /tmp/omni-edge-b.log" > "$RESULTS_DIR/edge_b.log" 2>/dev/null || true
    unset CURRENT_TARGET_NODE
    
    cat > "$result_file" << EOF
{
  "timestamp": "$timestamp",
  "architecture": "2-node (OmniEdge P2P)",
  "network_id": "$NETWORK_ID",
  "edge_a": {"public_ip": "$NODE_A", "vip": "$VIP_A", "vip6": "${VIP6_A:-N/A}"},
  "edge_b": {"public_ip": "$NODE_B", "vip": "$VIP_B", "vip6": "${VIP6_B:-N/A}"},
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
    echo -e "│  ${GREEN}2-NODE OMNIEDGE TEST RESULTS${NC}                            │"
    echo -e "├─────────────────────────────────────────────────────────┤"
    echo -e "│  Edge A:      $NODE_A → ${VIP_A:-N/A}"
    echo -e "│  Edge B:      $NODE_B → ${VIP_B:-N/A}"
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

    # Cleanup remote processes (if not debugging)
    print_step "Cleaning up remote processes..."
    for node in "$NODE_A" "$NODE_B"; do
        export CURRENT_TARGET_NODE="$node"
        ssh_cmd "$node" "sudo pkill -f omniedge || true; sudo pkill -f iperf3 || true" 2>/dev/null || true
        unset CURRENT_TARGET_NODE
    done
}

# =============================================================================
# Main
# =============================================================================

SKIP_DEPLOY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --node-a) NODE_A="$2"; shift 2 ;;
        --node-b) NODE_B="$2"; shift 2 ;;
        --ssh-key) SSH_KEY="$2"; shift 2 ;;
        --ssh-user) SSH_USER="$2"; shift 2 ;;
        --network) NETWORK_ID="$2"; shift 2 ;;
        --key) SECURITY_KEY="$2"; shift 2 ;;
        --duration) TEST_DURATION="$2"; shift 2 ;;
        --skip-deploy) SKIP_DEPLOY=true; shift ;;
        --local-docker) LOCAL_DOCKER=true; shift ;;
        --no-ipv6) TEST_IPV6=false; shift ;;
        --help|-h) show_help; exit 0 ;;
        *) print_error "Unknown option: $1"; show_help; exit 1 ;;
    esac
done

NETWORK_ID="${NETWORK_ID:-$OMNIEDGE_NETWORK_ID}"
SECURITY_KEY="${SECURITY_KEY:-$OMNIEDGE_SECURITY_KEY}"

if [[ -z "$NODE_A" || -z "$NODE_B" || -z "$NETWORK_ID" || -z "$SECURITY_KEY" ]]; then
    print_error "Missing required arguments"
    show_help
    exit 1
fi

print_header "OmniEdge 2-Node Cloud Test"
echo "Edge A:    $NODE_A"
echo "Edge B:    $NODE_B"
echo "Network:   $NETWORK_ID"

preflight_check
install_dependencies
if ! $SKIP_DEPLOY; then deploy_omniedge; fi
run_test

echo -e "\n${GREEN}✅ 2-Node OmniEdge cloud test completed!${NC}"
