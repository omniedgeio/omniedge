package bridge

import (
	"context"
	"fmt"
	"net"
	"os/exec"
	"regexp"
	"strconv"
	"time"

	"github.com/omniedgeio/omniedge-cli/pkg/api"
	"github.com/omniedgeio/omniedge-cli/pkg/core"
	"github.com/omniedgeio/omniedge-cli/pkg/msgbus"
	log "github.com/sirupsen/logrus"
	"github.com/wailsapp/wails/v2/pkg/runtime"
)

// ConnectionStatus represents the VPN connection state
type ConnectionStatus string

const (
	StatusDisconnected ConnectionStatus = "disconnected"
	StatusConnecting   ConnectionStatus = "connecting"
	StatusConnected    ConnectionStatus = "connected"
	StatusError        ConnectionStatus = "error"
)

// Bridge exposes core OmniEdge functionality to the frontend
type Bridge struct {
	ctx           context.Context
	status        ConnectionStatus
	virtualIP     string
	communityName string
	token         string
	refreshToken  string
	baseURL       string
	hardwareUUID  string
}

// NewBridge creates a new Bridge instance
func NewBridge() *Bridge {
	core.Env = "prod"
	core.LoadClientConfig()
	uuid, _ := core.RevealHardwareUUID()
	bridge := &Bridge{
		status:       StatusDisconnected,
		baseURL:      core.ConfigV.GetString("rest-endpoint-url"),
		hardwareUUID: uuid,
	}
	// Initial detection will be triggered in SetContext to ensure ctx is available for events
	return bridge
}

// SetContext sets the Wails context and initializes event handlers
func (b *Bridge) SetContext(ctx context.Context) {
	b.ctx = ctx
	bus := msgbus.GetBus()
	bus.Subscribe(msgbus.EventStatusChanged, func(e msgbus.Event) {
		if newStatus, ok := e.Payload.(string); ok {
			b.status = ConnectionStatus(newStatus)
			runtime.EventsEmit(b.ctx, "status-changed", newStatus)
		}
	})
	bus.Subscribe(msgbus.EventError, func(e msgbus.Event) {
		if err, ok := e.Payload.(string); ok {
			b.status = StatusError
			runtime.EventsEmit(b.ctx, "error", err)
		}
	})

	// Initial check for existing connection
	b.CheckExistingConnection()
}

// CheckExistingConnection scans for active utun interfaces
func (b *Bridge) CheckExistingConnection() {
	ifaces, err := net.Interfaces()
	if err != nil {
		return
	}

	for _, iface := range ifaces {
		// Look for utun interfaces that are up
		if (iface.Flags&net.FlagUp) != 0 && (len(iface.Name) >= 4 && iface.Name[:4] == "utun") {
			addrs, _ := iface.Addrs()
			for _, addr := range addrs {
				if ipnet, ok := addr.(*net.IPNet); ok {
					ip := ipnet.IP.To4()
					if ip != nil && (ip[0] == 100 || ip[0] == 10) {
						b.status = StatusConnected
						b.virtualIP = ip.String()
						log.Infof("Detected existing connection on %s: %s", iface.Name, b.virtualIP)
						if b.ctx != nil {
							runtime.EventsEmit(b.ctx, "status-changed", string(StatusConnected))
						}
						return
					}
				}
			}
		}
	}
}

// LoginResult represents the outcome of a login attempt
type LoginResult struct {
	Success      bool   `json:"success"`
	Message      string `json:"message"`
	Token        string `json:"token"`
	RefreshToken string `json:"refreshToken"`
}

// Login authenticates with a security key
func (b *Bridge) Login(securityKey string) LoginResult {
	authOpt := &api.AuthOption{
		SecretKey:  securityKey,
		AuthMethod: api.LoginBySecretKey,
	}

	authService := api.AuthService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
		},
	}

	resp, err := authService.Login(authOpt)
	if err != nil {
		log.Errorf("Bridge.Login failed: %v", err)
		return LoginResult{Success: false, Message: err.Error()}
	}

	b.token = resp.Token
	b.refreshToken = resp.RefreshToken
	log.Infof("Bridge.Login successful, token retrieved")

	// Get hardware UUID
	b.hardwareUUID, _ = core.RevealHardwareUUID()

	return LoginResult{
		Success:      true,
		Message:      "Login successful",
		Token:        resp.Token,
		RefreshToken: resp.RefreshToken,
	}
}

// NetworkInfo represents simplified virtual network data
type NetworkInfo struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	IPRange string `json:"ip_range"`
}

// GetNetworks retrieves the user's virtual networks
func (b *Bridge) GetNetworks() ([]NetworkInfo, error) {
	if b.token == "" {
		return nil, fmt.Errorf("not logged in")
	}

	vnService := api.VirtualNetworkService{
		HttpOption: api.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", b.token),
			BaseUrl: b.baseURL,
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
func (b *Bridge) Connect(networkID string) error {
	if b.token == "" {
		return fmt.Errorf("not logged in")
	}

	b.status = StatusConnecting
	runtime.EventsEmit(b.ctx, "status-changed", string(StatusConnecting))

	// Simulate connection delay
	go func() {
		time.Sleep(2 * time.Second)
		b.status = StatusConnected
		b.virtualIP = "100.100.0.239" // Simulating assigned IP
		runtime.EventsEmit(b.ctx, "status-changed", string(StatusConnected))
	}()

	return nil
}

// GetProfile returns the current user profile
func (b *Bridge) GetProfile() (*api.ProfileResponse, error) {
	log.Infof("Bridge.GetProfile called")
	if b.token == "" {
		return nil, fmt.Errorf("not logged in")
	}

	authService := api.AuthService{
		HttpOption: api.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", b.token),
			BaseUrl: b.baseURL,
		},
	}

	profile, err := authService.Me()
	if err != nil {
		log.Errorf("Bridge.GetProfile failed: %v", err)
		return nil, err
	}
	log.Infof("Bridge.GetProfile successful: %s", profile.Email)
	return profile, nil
}

// GetNetworkDevices returns detailed device list for a network
func (b *Bridge) GetNetworkDevices(networkID string) ([]api.VirtualNetworkDeviceResponse, error) {
	log.Infof("Bridge.GetNetworkDevices called for network: %s", networkID)
	if b.token == "" {
		return nil, fmt.Errorf("not logged in")
	}

	vnService := api.VirtualNetworkService{
		HttpOption: api.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", b.token),
			BaseUrl: b.baseURL,
		},
	}

	devices, err := vnService.GetDevices(networkID)
	if err != nil {
		log.Errorf("Bridge.GetNetworkDevices failed for %s: %v", networkID, err)
		return nil, err
	}

	// DEBUG LOG
	for i, d := range devices {
		log.Infof("Device[%d]: ID=%s, Name=%s, VIP=%s, Online=%v", i, d.ID, d.Name, d.VirtualIP, d.Online)
	}

	log.Infof("Bridge.GetNetworkDevices successful for %s, found %d devices", networkID, len(devices))
	return devices, nil
}

// GetLocalIP returns the primary local IPv4 address
func (b *Bridge) GetLocalIP() string {
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

// Ping returns the latency to a given IP in milliseconds
func (b *Bridge) Ping(targetIP string) (int, error) {
	if targetIP == "" {
		return 0, fmt.Errorf("empty IP")
	}

	// -c 1 (macOS/Linux)
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

// Disconnect stops the VPN connection
func (b *Bridge) Disconnect() {
	// Simulate disconnection
	b.status = StatusDisconnected
	b.virtualIP = ""
	runtime.EventsEmit(b.ctx, "status-changed", string(StatusDisconnected))
}

// GetStatus returns the current connection status
func (b *Bridge) GetStatus() string {
	return string(b.status)
}

// GetVirtualIP returns the assigned virtual IP
func (b *Bridge) GetVirtualIP() string {
	return b.virtualIP
}
