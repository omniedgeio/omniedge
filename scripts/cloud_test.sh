#!/bin/bash
# =============================================================================
# OmniEdge Cloud-to-Cloud Test Orchestrator (v2.5.0)
# Run from LOCAL machine, orchestrates tests between cloud instances
# Architecture: 2-Node P2P (Edge A + Edge B via OmniEdge Backend)
# Example with installer script:
# ./scripts/cloud_test.sh --node-a 54.x.x.x --node-b 35.x.x.x \
#    --network abc123 --key sk_xxx --ssh-key ~/.ssh/cloud.pem \
#    --use-installer
# Example with pre-built binary:
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

# Deployment method: "binary" (local file) or "installer" (remote curl script)
DEPLOY_METHOD="auto"

# Virtual IPs are assigned by OmniEdge backend
VIP_A=""
VIP_B=""
VIP6_A=""
VIP6_B=""
TEST_IPV6=true

show_help() {
    cat << EOF
OmniEdge 2-Node Cloud Test Orchestrator

Architecture:
   ┌──────────────────┐      ┌──────────────────┐
   │     Edge A       │◄────►│     Edge B       │
   │  (via OmniEdge)  │      │  (via OmniEdge)  │
   └──────────────────┘      └──────────────────┘

Usage:
  $0 --node-a <IP> --node-b <IP> --network <NETWORK_ID> --key <SECURITY_KEY> [OPTIONS]

Required:
  --node-a        IP address of Edge A
  --node-b        IP address of Edge B
  --network       OmniEdge Virtual Network ID
  --key           OmniEdge Security Key (from dashboard)

Options:
  --ssh-key       Path to SSH private key
  --ssh-user      SSH username (default: ubuntu)
  --duration      iperf3 test duration (default: 10s)
  --no-ipv6       Skip IPv6 tests
  --use-installer Use remote install script instead of local binary
  --skip-deploy   Skip deployment (use existing installation)
  --help          Show this help

Environment Variables:
  SSH_USER        SSH username
  OMNIEDGE_NETWORK_ID   Virtual Network ID
  OMNIEDGE_SECURITY_KEY Security Key

  Example:
    $0 --node-a 54.x.x.x --node-b 35.x.x.x \\
       --network abc123 --key sk_xxx \\
       --ssh-key ~/.ssh/cloud.pem

Prerequisites:
   - iperf3 installed on edge nodes
   - SSH access with key authentication
   - Root access for TUN interface creation
   - OmniEdge Security Key with network access
EOF
}

# =============================================================================
# SSH Helper Functions
# =============================================================================

ssh_cmd() {
    local host="$1"
    shift
    ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 \
        ${SSH_KEY:+-i "$SSH_KEY"} \
        "$SSH_USER@$host" "$@"
}

scp_to() {
    local src="$1"
    local host="$2"
    local dest="$3"
    scp -o StrictHostKeyChecking=no \
        ${SSH_KEY:+-i "$SSH_KEY"} \
        "$src" "$SSH_USER@$host:$dest"
}

# =============================================================================
# Pre-flight Checks
# =============================================================================

preflight_check() {
    print_header "Pre-flight Checks"
    
    local errors=0
    
    # Check for local dependencies
    print_step "Checking local dependencies..."
    for cmd in ssh scp jq bc curl; do
        if which "$cmd" &>/dev/null; then
            echo -e "  ✅ Local $cmd found"
        else
            echo -e "  ❌ Local $cmd NOT found. Please install it."
            errors=$((errors + 1))
        fi
    done

    # Get scripts directory
    local SCRIPT_DIR
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    
    # Auto-detect deployment method if not specified
    if [[ "$DEPLOY_METHOD" == "auto" ]]; then
        local LINUX_BINARY="$SCRIPT_DIR/omniedge-linux-amd64"
        if [[ -f "$LINUX_BINARY" ]]; then
            DEPLOY_METHOD="binary"
        else
            DEPLOY_METHOD="installer"
        fi
        echo -e "  📦 Auto-detected deployment method: ${YELLOW}$DEPLOY_METHOD${NC}"
    fi
    
    # Check deployment source based on method
    if [[ "$DEPLOY_METHOD" == "binary" ]]; then
        local LINUX_BINARY="$SCRIPT_DIR/omniedge-linux-amd64"
        if [[ -f "$LINUX_BINARY" ]]; then
            # Verify it's actually an x86_64 ELF binary
            if which file &>/dev/null && file "$LINUX_BINARY" | grep -q "ELF 64-bit.*x86-64"; then
                local binary_size
                binary_size=$(ls -lh "$LINUX_BINARY" | awk '{print $5}')
                echo -e "  ✅ Pre-built binary found: $LINUX_BINARY ($binary_size)"
                echo "     Architecture: x86-64 ELF (correct for cloud deployment)"
            else
                echo -e "  ⚠️ Binary found but cannot verify architecture (file command missing)"
            fi
        else
            echo -e "  ❌ Pre-built binary not found: $LINUX_BINARY"
            echo ""
            echo "     To get the binary, download from:"
            echo "     ${CYAN}https://github.com/omniedgeio/omniedge/releases${NC}"
            echo ""
            echo "     Or use --use-installer to install via:"
            echo "     ${CYAN}curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh | bash${NC}"
            errors=$((errors + 1))
        fi
    else
        echo -e "  📦 Will use remote installer script on nodes"
        echo "     ${CYAN}curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh | bash${NC}"
    fi
    
    # Check SSH connectivity
    for node in "$NODE_A" "$NODE_B"; do
        print_step "Testing SSH to $node..."
        if ssh_cmd "$node" "echo ok" &>/dev/null; then
            echo -e "✅ SSH to $node successful"
        else
            echo -e "❌ SSH to $node failed"
            errors=$((errors + 1))
        fi
    done
    
    # Check iperf3 on edge nodes
    print_step "Checking iperf3 and sudo on edge nodes..."
    for node in "$NODE_A" "$NODE_B"; do
        if ssh_cmd "$node" "which iperf3" &>/dev/null; then
            echo -e "  ✅ iperf3 installed on $node"
        else
            echo -e "  ❌ iperf3 not installed on $node"
            errors=$((errors + 1))
        fi
        
        if ssh_cmd "$node" "sudo -n true" &>/dev/null; then
            echo -e "  ✅ Passwordless sudo available on $node"
        else
            echo -e "  ⚠️  Sudo might require password on $node (script may hang)"
        fi
    done
    
    # Check networking tools on edge nodes
    print_step "Checking networking tools (iproute2) on edge nodes..."
    for node in "$NODE_A" "$NODE_B"; do
        for cmd in ip; do
            if ssh_cmd "$node" "which $cmd" &>/dev/null; then
                echo -e "  ✅ $cmd command found on $node"
            else
                echo -e "  ❌ $cmd command NOT found on $node"
                errors=$((errors + 1))
            fi
        done
    done

    if [[ $errors -gt 0 ]]; then
        print_error "Pre-flight checks failed with $errors errors"
        exit 1
    fi
    
    echo -e "\n${GREEN}All pre-flight checks passed!${NC}"
}

# =============================================================================
# Deploy OmniEdge (Binary or Installer)
# =============================================================================

deploy_binaries() {
    print_header "Deploying OmniEdge"
    
    if [[ "$DEPLOY_METHOD" == "installer" ]]; then
        deploy_via_installer
    else
        deploy_via_binary
    fi
}

# Deploy using the remote install script
deploy_via_installer() {
    echo -e "📦 Using remote installer script..."
    
    for node in "$NODE_A" "$NODE_B"; do
        print_step "Installing OmniEdge on $node via installer script..."
        
        # Run the install script on the remote node
        ssh_cmd "$node" "curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh | sudo bash"
        
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
}

# Deploy using pre-built binary
deploy_via_binary() {
    # Get the scripts directory
    local SCRIPT_DIR
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    
    # Check for pre-built binary in same folder
    local LINUX_BINARY="$SCRIPT_DIR/omniedge-linux-amd64"
    
    if [ ! -f "$LINUX_BINARY" ]; then
        print_error "Pre-built binary not found: $LINUX_BINARY"
        echo "   Download from GitHub releases, or use --use-installer"
        exit 1
    fi
    
    echo -e "📦 Using pre-built binary: $LINUX_BINARY"
    
    # Deploy to all nodes
    for node in "$NODE_A" "$NODE_B"; do
        print_step "Deploying to $node..."
        
        # Clean up and create remote directory
        ssh_cmd "$node" "rm -rf ~/omni-test && mkdir -p ~/omni-test"
        
        # Copy binary
        scp_to "$LINUX_BINARY" "$node" "~/omni-test/omniedge"
        
        # Make executable
        ssh_cmd "$node" "chmod +x ~/omni-test/omniedge"
        
        echo -e "  ✅ Deployed to $node"
    done
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

    # Kill any existing processes
    print_step "Cleaning up old processes and logs..."
    for node in "$NODE_A" "$NODE_B"; do
        ssh_cmd "$node" "sudo pkill -9 -f omniedge 2>/dev/null; sudo pkill -9 -f iperf3 2>/dev/null; sudo rm -f /tmp/omni-*.log" || true
    done
    sleep 2
    
    # Determine omniedge command path based on deployment method
    local OMNIEDGE_CMD
    if [[ "$DEPLOY_METHOD" == "installer" ]]; then
        OMNIEDGE_CMD="omniedge"
    else
        OMNIEDGE_CMD="./omni-test/omniedge"
    fi
    
    # Start Edge A
    print_step "Starting Edge A on $NODE_A..."
    ssh_cmd "$NODE_A" "sudo sh -c \"nohup $OMNIEDGE_CMD start -n $NETWORK_ID -s $SECURITY_KEY > /tmp/omni-edge-a.log 2>&1 &\" < /dev/null"
    sleep 3

    # Start Edge B
    print_step "Starting Edge B on $NODE_B..."
    ssh_cmd "$NODE_B" "sudo sh -c \"nohup $OMNIEDGE_CMD start -n $NETWORK_ID -s $SECURITY_KEY > /tmp/omni-edge-b.log 2>&1 &\" < /dev/null"
    sleep 3
    
    # Wait for VPN tunnel establishment
    print_step "Waiting for VPN tunnel establishment (60s for peer discovery)..."
    echo "   This includes OAuth, peer discovery, and WireGuard configuration."
    sleep 60
    
    # Check if daemons are running
    print_step "Checking daemon processes..."
    echo "Edge A process:"
    ssh_cmd "$NODE_A" "pgrep -a omniedge || echo 'NOT RUNNING'"
    echo "Edge B process:"
    ssh_cmd "$NODE_B" "pgrep -a omniedge || echo 'NOT RUNNING'"
    echo ""
    
    # Get VIPs from status
    print_step "Getting VPN IPs from status..."
    VIP_A=$(ssh_cmd "$NODE_A" "$OMNIEDGE_CMD status 2>/dev/null | grep -oP 'VPN IP: \K[0-9.]+' || echo ''")
    VIP_B=$(ssh_cmd "$NODE_B" "$OMNIEDGE_CMD status 2>/dev/null | grep -oP 'VPN IP: \K[0-9.]+' || echo ''")
    
    if [[ -z "$VIP_A" ]]; then
        echo -e "  ⚠️ Could not get VIP for Edge A, trying interface..."
        VIP_A=$(ssh_cmd "$NODE_A" "ip addr show omni0 2>/dev/null | grep 'inet ' | awk '{print \$2}' | cut -d/ -f1 || echo ''")
    fi
    if [[ -z "$VIP_B" ]]; then
        echo -e "  ⚠️ Could not get VIP for Edge B, trying interface..."
        VIP_B=$(ssh_cmd "$NODE_B" "ip addr show omni0 2>/dev/null | grep 'inet ' | awk '{print \$2}' | cut -d/ -f1 || echo ''")
    fi
    
    echo "Edge A VPN IP: $VIP_A"
    echo "Edge B VPN IP: $VIP_B"
    
    # Show logs for debugging
    print_step "Daemon logs (last 15 lines from /tmp)..."
    echo "--- Edge A log ---"
    ssh_cmd "$NODE_A" "tail -15 /tmp/omni-edge-a.log 2>/dev/null || echo 'No log in /tmp/omni-edge-a.log'"
    echo ""
    echo "--- Edge B log ---"
    ssh_cmd "$NODE_B" "tail -15 /tmp/omni-edge-b.log 2>/dev/null || echo 'No log in /tmp/omni-edge-b.log'"
    echo ""
    
    # ==========================================================================
    # BASELINE TESTS: Public IP (before VPN comparison)
    # ==========================================================================
    print_header "Baseline Network Metrics (Public IP: A → B)"
    echo "   These tests use public IPs WITHOUT the VPN tunnel."
    echo "   Results will be compared against VPN tunnel performance."
    echo ""
    
    # Baseline ping test (public IP)
    print_step "Baseline ping over public IP ($NODE_A → $NODE_B)..."
    local baseline_ping_output
    baseline_ping_output=$(ssh_cmd "$NODE_A" "ping -c 5 -W 5 $NODE_B 2>&1" || echo "PING_FAILED")
    local baseline_latency="N/A"
    if echo "$baseline_ping_output" | grep -q "rtt"; then
        baseline_latency=$(echo "$baseline_ping_output" | grep "rtt" | awk -F'/' '{print $5}')
        echo -e "  ✅ Baseline Ping: ${YELLOW}${baseline_latency} ms${NC}"
    else
        echo -e "  ⚠️ Baseline ping failed (firewall may be blocking ICMP)"
    fi
    
    # Baseline iperf3 test (public IP)
    print_step "Starting iperf3 server on Edge B (public IP)..."
    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null; nohup iperf3 -s -p 5201 > /tmp/iperf_baseline.log 2>&1 &"
    sleep 3
    
    print_step "Baseline iperf3 throughput test ($TEST_DURATION seconds) over public IP..."
    local baseline_iperf_json
    baseline_iperf_json=$(ssh_cmd "$NODE_A" "iperf3 -c $NODE_B -p 5201 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
    
    local baseline_throughput_bps
    baseline_throughput_bps=$(echo "$baseline_iperf_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
    local baseline_throughput_mbps
    baseline_throughput_mbps=$(echo "scale=2; $baseline_throughput_bps / 1000000" | bc 2>/dev/null || echo "N/A")
    
    if [[ "$baseline_throughput_mbps" != "N/A" && "$baseline_throughput_mbps" != "0" && "$baseline_throughput_mbps" != ".00" ]]; then
        echo -e "  ✅ Baseline Throughput: ${YELLOW}${baseline_throughput_mbps} Mbps${NC}"
    else
        echo -e "  ⚠️ Baseline iperf3 failed (port 5201 may be blocked)"
        baseline_throughput_mbps="N/A"
    fi
    
    # Stop baseline iperf3 serve
    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null" || true
    
    # ==========================================================================
    # VPN TUNNEL TESTS
    # ==========================================================================

    # Check interfaces on edges
    print_step "Verifying VPN interfaces..."
    echo "Edge A interfaces:"
    ssh_cmd "$NODE_A" "ip addr show omni0 2>/dev/null || echo 'omni0 not found'"
    echo ""
    echo "Edge B interfaces:"
    ssh_cmd "$NODE_B" "ip addr show omni0 2>/dev/null || echo 'omni0 not found'"
    
    # Network tests over VPN tunnel
    print_header "VPN Tunnel Metrics (OmniEdge: A → B)"
    echo "   These tests use VPN IPs ($VIP_A → $VIP_B) over encrypted tunnel."
    echo ""
    
    local avg_latency="N/A"
    local throughput_mbps="0"
    
    if [[ -n "$VIP_A" && -n "$VIP_B" ]]; then
        # Ping test over tunnel with retry
        print_step "Ping over tunnel ($VIP_A → $VIP_B) with retries..."
        local ping_output=""
        for attempt in 1 2 3; do
            echo "   Attempt $attempt/3..."
            ping_output=$(ssh_cmd "$NODE_A" "ping -c 5 -W 5 $VIP_B 2>&1" || echo "PING_FAILED")
            if echo "$ping_output" | grep -q "rtt"; then
                avg_latency=$(echo "$ping_output" | grep "rtt" | awk -F'/' '{print $5}')
                echo -e "  ✅ Ping: ${YELLOW}${avg_latency} ms${NC}"
                break
            else
                echo "   Ping failed, retrying in 10s..."
                sleep 10
            fi
        done
        if [[ "$avg_latency" == "N/A" ]]; then
            echo "     Diagnostics (IP):"
            ssh_cmd "$NODE_A" "ip addr show omni0" || true
            ssh_cmd "$NODE_B" "ip addr show omni0" || true
            echo "     Diagnostics (Route):"
            ssh_cmd "$NODE_A" "ip route" || true
            ssh_cmd "$NODE_B" "ip route" || true
            echo "     Check logs for peer discovery errors"
        fi
        
        # Check interfaces are up before iperf3
        print_step "Verifying VPN interfaces before iperf3..."
        local wg_a_up=false
        local wg_b_up=false
        if ssh_cmd "$NODE_A" "ip addr show omni0 2>/dev/null | grep -E -q 'state UP|state UNKNOWN'"; then
            wg_a_up=true
            echo "  ✅ Edge A omni0: UP"
        else
            echo "  ⚠️ Edge A omni0: DOWN or not found"
        fi
        if ssh_cmd "$NODE_B" "ip addr show omni0 2>/dev/null | grep -E -q 'state UP|state UNKNOWN'"; then
            wg_b_up=true
            echo "  ✅ Edge B omni0: UP"
        else
            echo "  ⚠️ Edge B omni0: DOWN or not found"
        fi

        # iperf3 over tunnel (only if interface is up)
        if [[ "$wg_a_up" == "true" && "$wg_b_up" == "true" ]]; then
            print_step "Starting iperf3 server on Edge B..."
            ssh_cmd "$NODE_B" "nohup iperf3 -s -p 5201 > iperf_server.log 2>&1 &"
            sleep 3
        
            print_step "Running iperf3 throughput test ($TEST_DURATION seconds) over tunnel..."
            local iperf_json
            iperf_json=$(ssh_cmd "$NODE_A" "iperf3 -c $VIP_B -p 5201 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
            
            local throughput_bps
            throughput_bps=$(echo "$iperf_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
            throughput_mbps=$(echo "scale=2; $throughput_bps / 1000000" | bc 2>/dev/null || echo "N/A")
            
            if [[ "$throughput_mbps" != "N/A" && "$throughput_mbps" != "0" && "$throughput_mbps" != ".00" ]]; then
                echo -e "  ✅ Throughput: ${YELLOW}${throughput_mbps} Mbps${NC}"
            else
                echo -e "  ❌ iperf3 test failed (tunnel may not be active)"
                throughput_mbps="0"
            fi
            
            # ==================================================================
            # IPv6 TUNNEL TESTS
            # ==================================================================
            if [[ "$TEST_IPV6" == "true" ]]; then
                print_header "IPv6 VPN Tunnel Metrics (OmniEdge: A → B)"
                echo "   Testing IPv6 connectivity over VPN tunnel."
                echo ""
                
                # Get IPv6 VIPs from interface
                VIP6_A=$(ssh_cmd "$NODE_A" "ip -6 addr show omni0 2>/dev/null | grep 'inet6' | grep -v 'fe80' | awk '{print \$2}' | cut -d/ -f1 | head -1" || echo "")
                VIP6_B=$(ssh_cmd "$NODE_B" "ip -6 addr show omni0 2>/dev/null | grep 'inet6' | grep -v 'fe80' | awk '{print \$2}' | cut -d/ -f1 | head -1" || echo "")
                
                if [[ -n "$VIP6_A" && -n "$VIP6_B" ]]; then
                    echo "Edge A IPv6: $VIP6_A"
                    echo "Edge B IPv6: $VIP6_B"
                    
                    # IPv6 Ping test
                    print_step "IPv6 Ping over tunnel ($VIP6_A → $VIP6_B)..."
                    local ping6_output
                    ping6_output=$(ssh_cmd "$NODE_A" "ping -6 -c 5 -W 5 $VIP6_B 2>&1" || echo "PING_FAILED")
                    if echo "$ping6_output" | grep -q "rtt"; then
                        avg_latency_v6=$(echo "$ping6_output" | grep "rtt" | awk -F'/' '{print $5}')
                        echo -e "  ✅ IPv6 Ping: ${YELLOW}${avg_latency_v6} ms${NC}"
                    else
                        avg_latency_v6="N/A"
                        echo -e "  ⚠️ IPv6 ping failed"
                    fi
                    
                    # IPv6 iperf3 test
                    print_step "Starting iperf3 server on Edge B (IPv6)..."
                    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null; nohup iperf3 -s -p 5202 > /tmp/iperf6_server.log 2>&1 &"
                    sleep 3
                    
                    print_step "Running IPv6 iperf3 throughput test ($TEST_DURATION seconds)..."
                    local iperf6_json
                    iperf6_json=$(ssh_cmd "$NODE_A" "iperf3 -6 -c $VIP6_B -p 5202 -t $TEST_DURATION -M 1300 -P 2 --json 2>/dev/null" || echo "{}")
                    
                    local throughput6_bps
                    throughput6_bps=$(echo "$iperf6_json" | jq '.end.sum_sent.bits_per_second // 0' 2>/dev/null || echo "0")
                    throughput_mbps_v6=$(echo "scale=2; $throughput6_bps / 1000000" | bc 2>/dev/null || echo "N/A")
                    
                    if [[ "$throughput_mbps_v6" != "N/A" && "$throughput_mbps_v6" != "0" && "$throughput_mbps_v6" != ".00" ]]; then
                        echo -e "  ✅ IPv6 Throughput: ${YELLOW}${throughput_mbps_v6} Mbps${NC}"
                    else
                        echo -e "  ⚠️ IPv6 iperf3 test failed"
                        throughput_mbps_v6="N/A"
                    fi
                    
                    ssh_cmd "$NODE_B" "pkill iperf3 2>/dev/null" || true
                else
                    echo -e "  ⚠️ IPv6 not configured on VPN interfaces, skipping IPv6 tests"
                    avg_latency_v6="N/A"
                    throughput_mbps_v6="N/A"
                fi
            else
                avg_latency_v6="N/A"
                throughput_mbps_v6="N/A"
            fi
        else
            echo -e "  ⚠️ Skipping iperf3 test - VPN interfaces not ready"
            throughput_mbps="0"
            avg_latency_v6="N/A"
            throughput_mbps_v6="N/A"
        fi
    else
        echo -e "  ⚠️ Skipping VPN tests - VIPs not available"
        avg_latency_v6="N/A"
        throughput_mbps_v6="N/A"
    fi
    
    # Collect logs
    print_step "Collecting logs..."
    ssh_cmd "$NODE_A" "cat /tmp/omni-edge-a.log" > "$RESULTS_DIR/edge_a.log" 2>/dev/null || true
    ssh_cmd "$NODE_B" "cat /tmp/omni-edge-b.log" > "$RESULTS_DIR/edge_b.log" 2>/dev/null || true
    
    # Create results JSON
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
    
    # Cleanup
    print_step "Cleaning up remote processes..."
    for node in "$NODE_A" "$NODE_B"; do
        ssh_cmd "$node" "sudo pkill -f omniedge || true; pkill -f iperf3 || true" 2>/dev/null || true
    done
    
    # Summary
    print_header "Test Complete"
    
    echo -e "┌─────────────────────────────────────────────────────────┐"
    echo -e "│  ${GREEN}2-NODE OMNIEDGE P2P TEST RESULTS${NC}                        │"
    echo -e "├─────────────────────────────────────────────────────────┤"
    echo -e "│  Network:     $NETWORK_ID"
    echo -e "│  Edge A:      $NODE_A → $VIP_A"
    echo -e "│  Edge B:      $NODE_B → $VIP_B"
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
    echo -e "Logs: ${CYAN}$RESULTS_DIR/*.log${NC}"
}

# =============================================================================
# Main
# =============================================================================

SKIP_DEPLOY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --node-a)
            NODE_A="$2"
            shift 2
            ;;
        --node-b)
            NODE_B="$2"
            shift 2
            ;;
        --ssh-key)
            SSH_KEY="$2"
            shift 2
            ;;
        --ssh-user)
            SSH_USER="$2"
            shift 2
            ;;
        --network)
            NETWORK_ID="$2"
            shift 2
            ;;
        --key)
            SECURITY_KEY="$2"
            shift 2
            ;;
        --duration)
            TEST_DURATION="$2"
            shift 2
            ;;
        --use-installer)
            DEPLOY_METHOD="installer"
            shift
            ;;
        --skip-deploy)
            SKIP_DEPLOY=true
            shift
            ;;
        --no-ipv6)
            TEST_IPV6=false
            shift
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Try environment variables as fallback
NETWORK_ID="${NETWORK_ID:-$OMNIEDGE_NETWORK_ID}"
SECURITY_KEY="${SECURITY_KEY:-$OMNIEDGE_SECURITY_KEY}"

# Validate required args
if [[ -z "$NODE_A" || -z "$NODE_B" ]]; then
    print_error "--node-a and --node-b are required"
    show_help
    exit 1
fi

if [[ -z "$NETWORK_ID" ]]; then
    print_error "--network (or OMNIEDGE_NETWORK_ID env var) is required"
    show_help
    exit 1
fi

if [[ -z "$SECURITY_KEY" ]]; then
    print_error "--key (or OMNIEDGE_SECURITY_KEY env var) is required"
    show_help
    exit 1
fi

print_header "OmniEdge 2-Node Cloud Test"
echo "Edge A:    $NODE_A"
echo "Edge B:    $NODE_B"
echo "Network:   $NETWORK_ID"
echo "Auth:      Security Key"

# Run test sequence
preflight_check

if ! $SKIP_DEPLOY; then
    deploy_binaries
fi

run_test

echo -e "\n${GREEN}✅ 2-Node OmniEdge cloud test completed!${NC}"
