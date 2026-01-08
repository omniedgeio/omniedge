package main

import (
	"encoding/json"
	"io"
	"net"
	"os"
	"os/signal"
	"syscall"

	"github.com/omniedgeio/omniedge-cli/pkg/core"
	log "github.com/sirupsen/logrus"
)

const (
	SocketPath = "/var/run/omniedge-helper.sock"
	Version    = "1.1.0"
)

// Request represents a command from the main app
type Request struct {
	Command string           `json:"command"`
	Args    core.StartOption `json:"args"`
}

// Response represents the helper's response
type Response struct {
	Success bool   `json:"success"`
	Message string `json:"message"`
	Data    string `json:"data,omitempty"`
}

func main() {
	log.SetFormatter(&log.TextFormatter{
		FullTimestamp: true,
	})
	log.SetLevel(log.InfoLevel)

	// Check if running as root
	if os.Geteuid() != 0 {
		log.Fatal("Helper must run as root to manage network interfaces")
	}

	log.Infof("OmniEdge Unified Helper v%s starting...", Version)

	// Remove existing socket
	os.Remove(SocketPath)

	// Create Unix socket
	listener, err := net.Listen("unix", SocketPath)
	if err != nil {
		log.Fatalf("Failed to create socket: %v", err)
	}
	defer listener.Close()

	// Set socket permissions so non-root can connect
	if err := os.Chmod(SocketPath, 0666); err != nil {
		log.Warnf("Failed to set socket permissions: %v", err)
	}

	log.Infof("Listening on %s", SocketPath)

	// Handle graceful shutdown
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigChan
		log.Info("Shutting down helper...")
		listener.Close()
		os.Remove(SocketPath)
		os.Exit(0)
	}()

	// Accept connections
	for {
		conn, err := listener.Accept()
		if err != nil {
			log.Errorf("Accept error: %v", err)
			continue
		}
		go handleConnection(conn)
	}
}

func handleConnection(conn net.Conn) {
	defer conn.Close()

	encoder := json.NewEncoder(conn)

	for {
		var req Request
		// Read raw bytes first for logging
		buf := make([]byte, 4096)
		n, err := conn.Read(buf)
		if err != nil {
			if err != io.EOF {
				log.Errorf("Read error: %v", err)
			}
			return
		}
		rawJson := buf[:n]
		log.Infof("Raw Request: %s", string(rawJson))

		if err := json.Unmarshal(rawJson, &req); err != nil {
			log.Errorf("Unmarshal error: %v", err)
			// Try to handle old format if unmarshal failed?
			return
		}

		log.Infof("Parsed Request: Command=%s, Args=%+v", req.Command, req.Args)
		resp := handleRequest(&req)

		if err := encoder.Encode(resp); err != nil {
			log.Errorf("Encode error: %v", err)
			return
		}
	}
}

var activeVPN *core.StartService

func handleRequest(req *Request) Response {
	switch req.Command {
	case "ping":
		return Response{Success: true, Message: "pong"}

	case "version":
		return Response{Success: true, Message: Version}

	case "start_vpn":
		return startVPN(req.Args)

	case "stop_vpn":
		return stopVPN()

	case "status":
		return getStatus()

	default:
		return Response{Success: false, Message: "Unknown command"}
	}
}

func startVPN(opt core.StartOption) Response {
	if activeVPN != nil {
		return Response{Success: false, Message: "VPN already running in this helper instance"}
	}

	log.Infof("Starting VPN for community: %s", opt.CommunityName)

	service := &core.StartService{
		StartOption: opt,
	}

	// Start VPN in a goroutine because Start() is blocking in the current implementation
	// We might need to refactor pkg/core to allow non-blocking start or handle it here.
	go func() {
		activeVPN = service
		if err := service.Start(); err != nil {
			log.Errorf("VPN Start error: %v", err)
			activeVPN = nil
		}
	}()

	return Response{Success: true, Message: "VPN start initiated"}
}

func stopVPN() Response {
	if activeVPN == nil {
		return Response{Success: true, Message: "VPN not running"}
	}

	activeVPN.Stop()
	activeVPN = nil

	return Response{Success: true, Message: "VPN stopped"}
}

func getStatus() Response {
	if activeVPN != nil {
		return Response{Success: true, Message: "running"}
	}
	return Response{Success: true, Message: "stopped"}
}
