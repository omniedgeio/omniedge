package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net"
	"os"
	"os/exec"
	"os/signal"
	"syscall"

	log "github.com/sirupsen/logrus"
)

const (
	SocketPath = "/var/run/omniedge-helper.sock"
	Version    = "1.0.0"
)

// Request represents a command from the main app
type Request struct {
	Command string            `json:"command"`
	Args    map[string]string `json:"args"`
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
		log.Fatal("Helper must run as root")
	}

	log.Infof("OmniEdge Helper v%s starting...", Version)

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
		log.Info("Shutting down...")
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

	decoder := json.NewDecoder(conn)
	encoder := json.NewEncoder(conn)

	for {
		var req Request
		if err := decoder.Decode(&req); err != nil {
			if err != io.EOF {
				log.Errorf("Decode error: %v", err)
			}
			return
		}

		log.Infof("Received command: %s", req.Command)
		resp := handleRequest(&req)

		if err := encoder.Encode(resp); err != nil {
			log.Errorf("Encode error: %v", err)
			return
		}
	}
}

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

var edgeProcess *exec.Cmd

func startVPN(args map[string]string) Response {
	if edgeProcess != nil && edgeProcess.Process != nil {
		return Response{Success: false, Message: "VPN already running"}
	}

	communityName := args["community_name"]
	virtualIP := args["virtual_ip"]
	secretKey := args["secret_key"]
	superNode := args["super_node"]
	deviceMac := args["device_mac"]
	subnetMask := args["subnet_mask"]
	_ = args["hostname"] // not used in n2n 2.6.0

	// Find edge binary - check common locations
	edgePaths := []string{
		"/usr/local/bin/edge",
		"/opt/homebrew/bin/edge",
		"/usr/bin/edge",
		"/Library/PrivilegedHelperTools/edge",
	}

	var edgePath string
	for _, path := range edgePaths {
		if _, err := os.Stat(path); err == nil {
			edgePath = path
			break
		}
	}

	if edgePath == "" {
		return Response{
			Success: false,
			Message: "Edge binary not found. Please install n2n edge: brew install n2n",
		}
	}

	// Build edge command - n2n 2.6.0 compatible
	// Note: -d (device name) is not supported in 2.6.0
	// Use -a for IP address (without CIDR, use -s for netmask)
	// Use -p 0 for random local port to avoid conflicts
	// Use -M 0 to disable management port
	edgeArgs := []string{
		"-c", communityName,
		"-a", virtualIP,
		"-s", subnetMask,
		"-k", secretKey,
		"-l", superNode,
		"-m", deviceMac,
		"-p", "0", // Random local port
		"-M", "0", // Disable management port
		"-r",
		"-E",
	}

	log.Infof("Starting edge: %s %v", edgePath, edgeArgs)
	edgeCmd := exec.Command(edgePath, edgeArgs...)

	// Redirect output to log file
	logFile, _ := os.OpenFile("/var/log/omniedge-edge.log", os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0644)
	if logFile != nil {
		edgeCmd.Stdout = logFile
		edgeCmd.Stderr = logFile
	}

	if err := edgeCmd.Start(); err != nil {
		return Response{Success: false, Message: fmt.Sprintf("Failed to start edge: %v", err)}
	}

	edgeProcess = edgeCmd
	log.Infof("Edge process started with PID %d", edgeProcess.Process.Pid)

	// Wait for process in background
	go func() {
		if err := edgeProcess.Wait(); err != nil {
			log.Errorf("Edge process exited: %v", err)
		}
		edgeProcess = nil
	}()

	return Response{Success: true, Message: "VPN started", Data: fmt.Sprintf("%d", edgeProcess.Process.Pid)}
}

// maskToCIDR converts subnet mask to CIDR notation
func maskToCIDR(mask string) string {
	switch mask {
	case "255.255.255.0":
		return "24"
	case "255.255.0.0":
		return "16"
	case "255.0.0.0":
		return "8"
	case "255.255.255.128":
		return "25"
	case "255.255.255.192":
		return "26"
	case "255.255.255.224":
		return "27"
	case "255.255.255.240":
		return "28"
	default:
		return "24" // default
	}
}

func stopVPN() Response {
	if edgeProcess == nil || edgeProcess.Process == nil {
		return Response{Success: true, Message: "VPN not running"}
	}

	if err := edgeProcess.Process.Signal(syscall.SIGTERM); err != nil {
		if err := edgeProcess.Process.Kill(); err != nil {
			return Response{Success: false, Message: fmt.Sprintf("Failed to stop VPN: %v", err)}
		}
	}

	edgeProcess = nil
	return Response{Success: true, Message: "VPN stopped"}
}

func getStatus() Response {
	if edgeProcess != nil && edgeProcess.Process != nil {
		return Response{Success: true, Message: "running", Data: fmt.Sprintf("%d", edgeProcess.Process.Pid)}
	}
	return Response{Success: true, Message: "stopped"}
}
