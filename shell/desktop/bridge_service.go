package main

import (
	"encoding/base64"
	"fmt"
	"net"
	"os"
	"os/exec"
	"regexp"
	"strconv"
	"time"

	"github.com/omniedgeio/omniedge-cli/pkg/api"
	"github.com/omniedgeio/omniedge-cli/pkg/bridge"
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
	app                  *application.App
	systemTray           *application.SystemTray
	iconConnected        []byte
	iconDisconnected     []byte
	status               ConnectionStatus
	virtualIP            string
	communityName        string
	connectedNetworkName string // Actual human-readable network name
	connectedNetworkID   string // Currently connected network ID
	token                string
	refreshToken         string
	baseURL              string
	hardwareUUID         string
	appIcon              []byte
	heartbeatDone        chan bool
	activeService        *core.StartService // Kept for reference, but legacy on macOS
	helper               *bridge.HelperClient
}

// NewBridgeService creates a new BridgeService instance
func NewBridgeService() *BridgeService {
	core.Env = "prod"
	core.LoadClientConfig()
	log.SetLevel(log.DebugLevel)
	uuid, _ := core.RevealHardwareUUID()
	b := &BridgeService{
		status:       StatusDisconnected,
		baseURL:      core.ConfigV.GetString("rest-endpoint-url"),
		hardwareUUID: uuid,
		helper:       bridge.NewHelperClient(),
	}
	// Try to stop any ghost VPNs via the helper on startup
	go b.cleanupGhostVPN()
	return b
}

func (b *BridgeService) cleanupGhostVPN() {
	if b.helper.IsAvailable() {
		log.Info("BridgeService: Privileged helper detected. Performing initial cleanup...")
		b.helper.StopVPN()
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
	go b.startHeartbeat()
}

func (b *BridgeService) startHeartbeat() {
	ticker := time.NewTicker(30 * time.Second)
	b.heartbeatDone = make(chan bool)

	// Immediate first heartbeat
	b.sendHeartbeat()

	for {
		select {
		case <-b.heartbeatDone:
			ticker.Stop()
			return
		case <-ticker.C:
			b.sendHeartbeat()
		}
	}
}

func (b *BridgeService) sendHeartbeat() {
	if b.token == "" {
		return
	}

	hbService := api.HeartbeatService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
			Token:   b.token,
		},
	}
	opt := &api.HeartbeatOption{
		HardwareUUID: b.hardwareUUID,
	}
	err := hbService.Heartbeat(opt)
	if err != nil {
		log.Debugf("BridgeService: Heartbeat failed: %v", err)
	} else {
		log.Trace("BridgeService: Heartbeat sent successfully")
	}
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

	// Check for existing connection after login
	b.CheckExistingConnection()

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
func (b *BridgeService) Connect(networkId string) error {
	if b.token == "" {
		return fmt.Errorf("not logged in")
	}

	// Initial cleanup to ensure utun is available
	if b.helper.IsAvailable() {
		b.helper.StopVPN()
	}

	b.status = StatusConnecting
	if b.app != nil {
		b.app.Event.Emit("status-changed", string(b.status))
	}

	// 1. Register device first (creates or updates the device in the API)
	regService := api.RegisterService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
			Token:   b.token,
		},
	}
	deviceResp, err := regService.Register(&api.RegisterOption{
		Name:         b.GetDeviceName(),
		HardwareUUID: b.hardwareUUID,
		OS:           "darwin",
	})
	if err != nil {
		log.Warnf("BridgeService: Device registration failed: %v", err)
		// Continue anyway - device might already exist
	} else {
		log.Infof("BridgeService: Device registered with ID: %s", deviceResp.ID)
	}

	// 2. Get Join info
	vnService := api.VirtualNetworkService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
			Token:   b.token,
		},
	}

	// Use the registered device UUID if available, otherwise use hardware UUID
	deviceId := b.hardwareUUID
	if deviceResp != nil && deviceResp.ID != "" {
		deviceId = deviceResp.ID
	}
	joinOpt := &api.JoinOption{
		VirtualNetworkId: networkId,
		DeviceId:         deviceId,
	}

	joinResp, err := vnService.Join(joinOpt)
	if err != nil {
		b.status = StatusError
		if b.app != nil {
			b.app.Event.Emit("status-changed", string(b.status))
		}
		return err
	}

	// 2. Generate Random Mac
	randomMac, _ := core.GenerateRandomMac()

	// 3. Prepare Start Option
	startOption := core.StartOption{
		Hostname:      b.GetDeviceName(),
		DeviceMac:     randomMac,
		CommunityName: joinResp.CommunityName,
		VirtualIP:     joinResp.VirtualIP,
		SecretKey:     joinResp.SecretKey,
		DeviceMask:    joinResp.SubnetMask,
		SuperNode:     joinResp.Server.Host,
		EnableRouting: false,
		Token:         b.token,
		BaseUrl:       b.baseURL,
		HardwareUUID:  b.hardwareUUID,
	}

	// 4. Start VPN via Helper
	if b.helper.IsAvailable() {
		log.Infof("BridgeService: Starting VPN via privileged helper for %s", joinResp.CommunityName)
		if err := b.helper.StartVPN(&startOption); err != nil {
			b.status = StatusError
			if b.app != nil {
				b.app.Event.Emit("status-changed", string(b.status))
			}
			return fmt.Errorf("helper failed to start vpn: %w", err)
		}
	} else {
		// Fallback for non-macOS or dev environments (though likely to fail without root)
		log.Warn("BridgeService: Privileged helper not available, attempting in-process start...")
		b.activeService = &core.StartService{
			StartOption: startOption,
		}
		if err := b.activeService.Start(); err != nil {
			b.status = StatusError
			if b.app != nil {
				b.app.Event.Emit("status-changed", string(b.status))
			}
			return err
		}
	}

	b.status = StatusConnected
	b.virtualIP = joinResp.VirtualIP
	b.communityName = joinResp.CommunityName
	b.connectedNetworkID = networkId

	// Lookup actual network name from networks list
	nets, err := b.GetNetworks()
	if err == nil {
		for _, net := range nets {
			if net.ID == networkId {
				b.connectedNetworkName = net.Name
				log.Infof("BridgeService: Connected to network: %s (ID: %s)", net.Name, networkId)
				break
			}
		}
	}
	if b.connectedNetworkName == "" {
		b.connectedNetworkName = "Unknown Network"
	}

	b.updateTrayIcon()

	if b.app != nil {
		b.app.Event.Emit("status-changed", string(b.status))
	}
	return nil
}

// Disconnect terminates the current VPN connection
func (b *BridgeService) Disconnect() error {
	log.Info("BridgeService: Disconnect called")

	// 1. Stop via helper if available
	if b.helper.IsAvailable() {
		log.Info("BridgeService: Stopping VPN via helper")
		if err := b.helper.StopVPN(); err != nil {
			log.Warnf("BridgeService: StopVPN via helper failed: %v", err)
		}
	}

	// 2. Stop active service if running in-process
	if b.activeService != nil {
		log.Info("BridgeService: Stopping active service")
		b.activeService.Stop()
		b.activeService = nil
	}

	// Note: Removed pkill command as it was too broad and killed the Node.js frontend server
	// The helper.StopVPN() should handle proper edge termination

	b.status = StatusDisconnected
	b.virtualIP = ""
	b.communityName = ""
	b.connectedNetworkName = ""
	b.connectedNetworkID = ""
	b.updateTrayIcon()

	if b.app != nil {
		b.app.Event.Emit("status-changed", string(b.status))
	}
	log.Info("BridgeService: Disconnect completed")
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

// GetDeviceName returns the name of this device (from config or API)
func (b *BridgeService) GetDeviceName() string {
	name := core.ConfigV.GetString("device-name")
	if name == "" {
		// Fallback to hostname
		hostname, _ := os.Hostname()
		return hostname
	}
	return name
}

// GetConnectedNetworkName returns the name of the currently joined network
func (b *BridgeService) GetConnectedNetworkName() string {
	if b.status != StatusConnected {
		return "" // Return empty when not connected
	}
	return b.connectedNetworkName
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

type DeviceWithNetwork struct {
	api.VirtualNetworkDeviceResponse
	NetworkID string `json:"network_id"`
}

// GetNetworkDevices returns devices in a specific network
func (b *BridgeService) GetNetworkDevices(networkID string) ([]DeviceWithNetwork, error) {
	log.Infof("BridgeService: GetNetworkDevices called with networkID: %s", networkID)
	if b.token == "" {
		return nil, fmt.Errorf("not logged in")
	}
	vnService := api.VirtualNetworkService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
			Token:   b.token,
		},
	}
	devs, err := vnService.GetDevices(networkID)
	if err != nil {
		log.Errorf("BridgeService: GetDevices API error: %v", err)
		return nil, err
	}
	log.Infof("BridgeService: GetDevices returned %d devices", len(devs))

	result := make([]DeviceWithNetwork, len(devs))
	for i, d := range devs {
		log.Debugf("BridgeService: Device %d: ID=%s, Name=%s, VirtualIP=%s", i, d.ID, d.Name, d.VirtualIP)
		result[i] = DeviceWithNetwork{
			VirtualNetworkDeviceResponse: d,
			NetworkID:                    networkID,
		}
	}
	return result, nil
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
