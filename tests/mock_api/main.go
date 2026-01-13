package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"sync"
)

// SuccessResponse wrapper
type SuccessResponse struct {
	Code    int         `json:"-"`
	Message string      `json:"message"`
	Data    interface{} `json:"data"`
}

type DeviceResponse struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	OS   string `json:"os"`
}

type ServerResponse struct {
	Host string `json:"host"`
}

type JoinVirtualNetworkResponse struct {
	CommunityName string          `json:"community_name"`
	SecretKey     string          `json:"secret_key"`
	VirtualIP     string          `json:"virtual_ip"`
	SubnetMask    string          `json:"subnet_mask"`
	Server        *ServerResponse `json:"server"`
}

var (
	deviceCount = 0
	mu          sync.Mutex
)

func main() {
	http.HandleFunc("/api/v2/devices", handleRegister)
	http.HandleFunc("/api/v2/virtual-networks/", handleVirtualNetwork)

	fmt.Println("Mock API Server listening on :8080")
	if err := http.ListenAndServe(":8080", nil); err != nil {
		log.Fatal(err)
	}
}

func handleRegister(w http.ResponseWriter, r *http.Request) {
	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	mu.Lock()
	deviceCount++
	id := fmt.Sprintf("dev-%d", deviceCount)
	mu.Unlock()

	resp := SuccessResponse{
		Message: "Success",
		Data: DeviceResponse{
			ID:   id,
			Name: fmt.Sprintf("mock-device-%s", id),
			OS:   "linux",
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}

func handleVirtualNetwork(w http.ResponseWriter, r *http.Request) {
	// Pattern: /api/v2/virtual-networks/:vnId/devices/:deviceId
	// We just blindly accept any join request and assign IPs sequentially

	if r.Method != "POST" {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	mu.Lock()
	// Assign IP based on device count (simple mapping for test)
	// assuming handleRegister called before join for each client
	// But actually start.go calls register then join.
	// We can just use the device ID from URL or just increment IP counter.
	// Let's use a simple counter for IPs.
	ipOctet := deviceCount + 1 // .1 used by supernode usually? or .1 is gateway.
	// Let's assign 100.100.0.x
	ip := fmt.Sprintf("100.100.0.%d", ipOctet)
	mu.Unlock()

	log.Printf("Joining device, assigning IP: %s\n", ip)

	resp := SuccessResponse{
		Message: "Success",
		Data: JoinVirtualNetworkResponse{
			CommunityName: "test-network",
			SecretKey:     "test-secret",
			VirtualIP:     ip,
			SubnetMask:    "255.255.255.0",
			Server: &ServerResponse{
				Host: "127.0.0.1:7654", // Local Supernode
			},
		},
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(resp)
}
