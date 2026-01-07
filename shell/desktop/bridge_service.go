package main

import (
	"encoding/base64"
	"fmt"
	"net"
	"os/exec"
	"regexp"
	"strconv"

	"github.com/omniedgeio/omniedge-cli/pkg/api"
	"github.com/omniedgeio/omniedge-cli/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/wailsapp/wails/v3/pkg/application"
)

// ConnectionStatus represents the VPN connection state
type ConnectionStatus string

const (
	StatusDisconnected ConnectionStatus = "disconnected"
	StatusConnecting   ConnectionStatus = "connecting"
	StatusConnected    ConnectionStatus = "connected"
	StatusError        ConnectionStatus = "error"
)

// BridgeService exposes core OmniEdge functionality to the frontend
type BridgeService struct {
	app              *application.App
	systemTray       *application.SystemTray
	iconConnected    []byte
	iconDisconnected []byte
	status           ConnectionStatus
	virtualIP        string
	communityName    string
	token            string
	refreshToken     string
	baseURL          string
	hardwareUUID     string
	appIcon          []byte
}

// NewBridgeService creates a new BridgeService instance
func NewBridgeService() *BridgeService {
	core.Env = "prod"
	core.LoadClientConfig()
	log.SetLevel(log.DebugLevel)
	uuid, _ := core.RevealHardwareUUID()
	return &BridgeService{
		status:       StatusDisconnected,
		baseURL:      core.ConfigV.GetString("rest-endpoint-url"),
		hardwareUUID: uuid,
	}
}

// SetAppIcon sets the app icon bytes
func (b *BridgeService) SetAppIcon(icon []byte) {
	b.appIcon = icon
}

// GetLogos returns the app logo as base64
func (b *BridgeService) GetLogos() string {
	if len(b.appIcon) == 0 {
		return ""
	}
	return base64.StdEncoding.EncodeToString(b.appIcon)
}

// SetSystemTray sets the system tray reference and icons for dynamic switching
func (b *BridgeService) SetSystemTray(tray *application.SystemTray, connected []byte, disconnected []byte) {
	b.systemTray = tray
	b.iconConnected = connected
	b.iconDisconnected = disconnected
	b.updateTrayIcon()
}

func (b *BridgeService) updateTrayIcon() {
	if b.systemTray == nil {
		return
	}
	if b.status == StatusConnected {
		if len(b.iconConnected) > 0 {
			b.systemTray.SetIcon(b.iconConnected)
		}
	} else {
		if len(b.iconDisconnected) > 0 {
			b.systemTray.SetIcon(b.iconDisconnected)
		}
	}
}

// SetApp sets the Wails app reference
func (b *BridgeService) SetApp(app *application.App) {
	b.app = app
	b.CheckExistingConnection()
}

// CheckExistingConnection checks if there's an existing VPN connection
func (b *BridgeService) CheckExistingConnection() {
	ifaces, err := net.Interfaces()
	if err != nil {
		return
	}

	for _, iface := range ifaces {
		if (iface.Flags&net.FlagUp) != 0 && (len(iface.Name) >= 4 && iface.Name[:4] == "utun") {
			addrs, _ := iface.Addrs()
			for _, addr := range addrs {
				if ipnet, ok := addr.(*net.IPNet); ok {
					ip := ipnet.IP.To4()
					if ip != nil && (ip[0] == 100 || ip[0] == 10) {
						b.status = StatusConnected
						b.virtualIP = ip.String()
						b.updateTrayIcon()
						if b.app != nil {
							b.app.Event.Emit("status-changed", string(b.status))
						}
						return
					}
				}
			}
		}
	}
}

// LoginResult represents the result of a login attempt
type LoginResult struct {
	Success bool   `json:"success"`
	Message string `json:"message"`
}

// NetworkInfo represents basic network information
type NetworkInfo struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	IPRange string `json:"ip_range"`
}

// Login authenticates with the OmniEdge API using a security key
func (b *BridgeService) Login(securityKey string) LoginResult {
	if securityKey == "" {
		return LoginResult{Success: false, Message: "Security key is required"}
	}

	authService := api.AuthService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
		},
	}
	opt := &api.AuthOption{
		SecretKey:  securityKey,
		AuthMethod: api.LoginBySecretKey,
	}

	resp, err := authService.Login(opt)
	if err != nil {
		log.Errorf("BridgeService.Login failed: %v", err)
		return LoginResult{Success: false, Message: err.Error()}
	}

	b.token = "Bearer " + resp.Token
	b.refreshToken = resp.RefreshToken
	log.Infof("BridgeService.Login successful. Token set.")
	return LoginResult{Success: true, Message: "Login successful"}
}

// GetProfile returns the current user's profile
func (b *BridgeService) GetProfile() (*api.ProfileResponse, error) {
	if b.token == "" {
		log.Warn("BridgeService.GetProfile called without token")
		return nil, fmt.Errorf("not logged in")
	}

	log.Debugf("BridgeService.GetProfile called with token: %s...", b.token[:10])

	authService := api.AuthService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
			Token:   b.token,
		},
	}
	profile, err := authService.Me()
	if err != nil {
		log.Errorf("BridgeService.GetProfile failed: %v", err)
	}
	return profile, err
}

// GetNetworks returns the list of available virtual networks
func (b *BridgeService) GetNetworks() ([]NetworkInfo, error) {
	if b.token == "" {
		return nil, fmt.Errorf("not logged in")
	}
	vnService := api.VirtualNetworkService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
			Token:   b.token,
		},
	}
	nets, err := vnService.List()
	if err != nil {
		return nil, err
	}

	result := make([]NetworkInfo, len(nets))
	for i, net := range nets {
		result[i] = NetworkInfo{
			ID:      net.ID,
			Name:    net.Name,
			IPRange: net.IPRange,
		}
	}
	return result, nil
}

// Connect initiates a VPN connection to the specified network
func (b *BridgeService) Connect(networkID string) error {
	b.status = StatusConnecting
	if b.app != nil {
		b.app.Event.Emit("status-changed", string(b.status))
	}

	// TODO: Implement actual VPN connection logic
	// This would involve calling the privileged helper

	b.status = StatusConnected
	b.updateTrayIcon()
	if b.app != nil {
		b.app.Event.Emit("status-changed", string(b.status))
	}
	return nil
}

// Disconnect terminates the current VPN connection
func (b *BridgeService) Disconnect() error {
	b.status = StatusDisconnected
	b.virtualIP = ""
	b.updateTrayIcon()
	if b.app != nil {
		b.app.Event.Emit("status-changed", string(b.status))
	}
	return nil
}

// GetStatus returns the current connection status
func (b *BridgeService) GetStatus() string {
	return string(b.status)
}

// GetVirtualIP returns the virtual IP assigned to this device
func (b *BridgeService) GetVirtualIP() string {
	return b.virtualIP
}

// GetLocalIP returns the local IP address
func (b *BridgeService) GetLocalIP() string {
	addrs, err := net.InterfaceAddrs()
	if err != nil {
		return "Unknown"
	}
	for _, address := range addrs {
		if ipnet, ok := address.(*net.IPNet); ok && !ipnet.IP.IsLoopback() {
			if ipnet.IP.To4() != nil {
				return ipnet.IP.String()
			}
		}
	}
	return "127.0.0.1"
}

// GetNetworkDevices returns devices in a specific network
func (b *BridgeService) GetNetworkDevices(networkID string) ([]api.VirtualNetworkDeviceResponse, error) {
	if b.token == "" {
		return nil, fmt.Errorf("not logged in")
	}
	vnService := api.VirtualNetworkService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
			Token:   b.token,
		},
	}
	return vnService.GetDevices(networkID)
}

// Ping measures latency to a target IP
func (b *BridgeService) Ping(targetIP string) (int, error) {
	if targetIP == "" {
		return 0, fmt.Errorf("empty IP")
	}

	cmd := exec.Command("ping", "-c", "1", "-t", "1", targetIP)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return 0, err
	}

	re := regexp.MustCompile(`time=([\d.]+)`)
	matches := re.FindStringSubmatch(string(output))
	if len(matches) > 1 {
		latency, err := strconv.ParseFloat(matches[1], 64)
		if err == nil {
			return int(latency), nil
		}
	}

	return 0, fmt.Errorf("could not parse ping output")
}
