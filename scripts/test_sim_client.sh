#!/bin/bash
set -e

# script to run test.yml step 5-8 locally for exit node simulation
# Usage: ./scripts/test_sim_client.sh <SECRET_KEY> <NETWORK_ID> <EXIT_NODE_IP>

if [ "$#" -ne 3 ]; then
    echo "Usage: $0 <SECRET_KEY> <NETWORK_ID> <EXIT_NODE_IP>"
    exit 1
fi

OMNIEDGE_SECRET_KEY=$1
OMNIEDGE_NETWORK_ID=$2
EXIT_NODE_IP=$3

echo "--- 5. Starting Docker Client Simulation ---"
# Remove existing container if any
docker rm -f client-sim 2>/dev/null || true

docker run -d --name client-sim --hostname Client-Sim-Box \
  --privileged --cap-add NET_ADMIN --device /dev/net/tun \
  -v $(pwd)/out:/app \
  debian:stable-slim sleep infinity

echo "--- 6. Setting up Docker Client Environment ---"
docker exec client-sim sh -c "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y iproute2 iputils-ping ca-certificates"

# Fix machine-id with a stable value to ensure identity consistency
docker exec client-sim sh -c "echo 550e8400e29b41d4a716446655440000 > /etc/machine-id"

# Fix /dev/net/tun node creation manually if needed
docker exec client-sim sh -c "if [ ! -d /dev/net ]; then mkdir /dev/net; fi"
docker exec client-sim sh -c "if [ ! -e /dev/net/tun ]; then mknod /dev/net/tun c 10 200; fi"

echo "--- 7. Clean Login on Docker Client ---"
docker exec client-sim /app/omniedge login -s "$OMNIEDGE_SECRET_KEY"

echo "--- 8. Starting Docker Client ---"
docker exec -d client-sim sh -c "/app/omniedge join -n $OMNIEDGE_NETWORK_ID -e $EXIT_NODE_IP > /app/client.log 2>&1"

echo "--- Waiting for Client IP and Routing ---"
MAX_RETRIES=30
for i in $(seq 1 $MAX_RETRIES); do
    CLIENT_IP=$(docker exec client-sim ip -4 addr show | grep -oE "100\.100\.[0-9]+\.[0-9]+" | head -n 1 || true)
    ROUTE_CHECK=$(docker exec client-sim ip route show | grep default | grep "$EXIT_NODE_IP" || true)
    
    if [ ! -z "$CLIENT_IP" ] && [ "$CLIENT_IP" != "$EXIT_NODE_IP" ] && [ ! -z "$ROUTE_CHECK" ]; then
        echo "Client connected with unique IP: $CLIENT_IP"
        echo "Exit Node Host: $EXIT_NODE_IP"
        echo "Client Container: $CLIENT_IP"
        
        echo "--- Verification: Pinging 1.1.1.1 through tunnel ---"
        if docker exec client-sim ping -c 4 -W 5 1.1.1.1; then
            echo "Internet access through tunnel: OK"
            echo "SUCCESS: Exit Node Simulation verified locally!"
            exit 0
        else
            echo "Internet access through tunnel: FAILED"
            exit 1
        fi
    fi
    echo -n "."
    sleep 2
done

echo ""
echo "ERROR: Exit Node Simulation Failed or Timeout"
echo "--- Client IP Status ---"
docker exec client-sim ip addr show || true
echo "--- Client Routing Table ---"
docker exec client-sim ip route show || true
echo "--- Client Logs ---"
docker exec client-sim cat /app/client.log || true
exit 1
