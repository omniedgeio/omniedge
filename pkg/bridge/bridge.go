package bridge

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
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
	return &Bridge{
		status:       StatusDisconnected,
		baseURL:      core.ConfigV.GetString("rest-endpoint-url"),
		hardwareUUID: uuid,
	}
}

// SetContext stores the Wails runtime context
func (b *Bridge) SetContext(ctx context.Context) {
	b.ctx = ctx

	// Subscribe to msgbus events and emit to frontend
	bus := msgbus.GetBus()
	bus.Subscribe(msgbus.EventStatusChanged, func(e msgbus.Event) {
		runtime.EventsEmit(b.ctx, "status-changed", e.Payload)
	})
	bus.Subscribe(msgbus.EventError, func(e msgbus.Event) {
		runtime.EventsEmit(b.ctx, "error", e.Payload)
	})
}

// GetStatus returns the current connection status
func (b *Bridge) GetStatus() ConnectionStatus {
	return b.status
}

// GetVirtualIP returns the current virtual IP
func (b *Bridge) GetVirtualIP() string {
	return b.virtualIP
}

// LoginResult represents the result of login operation
type LoginResult struct {
	Success      bool   `json:"success"`
	Message      string `json:"message"`
	Token        string `json:"token,omitempty"`
	RefreshToken string `json:"refresh_token,omitempty"`
}

// Login authenticates with OmniEdge using a security key
func (b *Bridge) Login(securityKey string) LoginResult {
	authService := api.AuthService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
		},
	}

	authOpt := &api.AuthOption{
		AuthMethod: api.LoginBySecretKey,
		SecretKey:  securityKey,
	}

	resp, err := authService.Login(authOpt)
	if err != nil {
		return LoginResult{Success: false, Message: err.Error()}
	}

	b.token = resp.Token
	b.refreshToken = resp.RefreshToken

	// Get hardware UUID
	b.hardwareUUID, _ = core.RevealHardwareUUID()

	return LoginResult{
		Success:      true,
		Message:      "Login successful",
		Token:        resp.Token,
		RefreshToken: resp.RefreshToken,
	}
}

// NetworkInfo represents a virtual network
type NetworkInfo struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	IPRange string `json:"ip_range"`
}

// GetNetworks returns available virtual networks
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

	networks, err := vnService.List()
	if err != nil {
		return nil, err
	}

	result := make([]NetworkInfo, len(networks))
	for i, n := range networks {
		result[i] = NetworkInfo{
			ID:      n.ID,
			Name:    n.Name,
			IPRange: n.IPRange,
		}
	}

	return result, nil
}

// Connect joins and starts the VPN connection
func (b *Bridge) Connect(networkID string) error {
	if b.token == "" {
		return fmt.Errorf("not logged in")
	}

	// Check if helper is available
	helper := NewHelperClient()
	if !helper.IsAvailable() {
		return fmt.Errorf("privileged helper not running. Please run: sudo shell/desktop/scripts/install_helper.sh")
	}

	b.status = StatusConnecting
	runtime.EventsEmit(b.ctx, "status-changed", StatusConnecting)

	// Register device
	httpOpt := api.HttpOption{
		Token:   fmt.Sprintf("Bearer %s", b.token),
		BaseUrl: b.baseURL,
	}

	registerService := api.RegisterService{HttpOption: httpOpt}
	device, err := registerService.Register(&api.RegisterOption{
		Name:         core.RevealHostName(),
		HardwareUUID: b.hardwareUUID,
		OS:           core.RevealOS(),
	})
	if err != nil {
		b.status = StatusError
		runtime.EventsEmit(b.ctx, "status-changed", StatusError)
		return err
	}

	// Join network
	vnService := api.VirtualNetworkService{HttpOption: httpOpt}
	joinResp, err := vnService.Join(&api.JoinOption{
		VirtualNetworkId: networkID,
		DeviceId:         device.ID,
	})
	if err != nil {
		b.status = StatusError
		runtime.EventsEmit(b.ctx, "status-changed", StatusError)
		return err
	}

	// Debug log to file
	debugMsg := fmt.Sprintf("TIME: %s, JOIN_RESP: %+v, ARGS: %+v\n", time.Now().Format(time.RFC3339), joinResp, device)
	os.WriteFile("/tmp/omniedge-bridge.log", []byte(debugMsg), 0666)

	log.Infof("Joined network: %+v", joinResp)
	log.Infof("SubnetMask from API: %s", joinResp.SubnetMask)

	// Provide fallback mask if API returns empty
	subnetMask := joinResp.SubnetMask
	if subnetMask == "" {
		subnetMask = "255.255.255.0"
	}

	// Start VPN via privileged helper
	randomMac, _ := core.GenerateRandomMac()
	opt := &core.StartOption{
		Hostname:      device.Name,
		CommunityName: joinResp.CommunityName,
		VirtualIP:     joinResp.VirtualIP,
		SecretKey:     joinResp.SecretKey,
		DeviceMac:     randomMac,
		DeviceMask:    subnetMask,
		SuperNode:     joinResp.Server.Host,
		EnableRouting: false,
		Token:         fmt.Sprintf("Bearer %s", b.token),
		BaseUrl:       b.baseURL,
		HardwareUUID:  b.hardwareUUID,
	}

	optJson, _ := json.Marshal(opt)
	os.WriteFile("/tmp/omniedge-opt.json", optJson, 0666)

	if err := helper.StartVPN(opt); err != nil {
		b.status = StatusError
		runtime.EventsEmit(b.ctx, "status-changed", StatusError)
		runtime.EventsEmit(b.ctx, "error", err.Error())
		return err
	}

	b.status = StatusConnected
	runtime.EventsEmit(b.ctx, "status-changed", StatusConnected)
	return nil
}

// Disconnect stops the VPN connection
func (b *Bridge) Disconnect() {
	helper := NewHelperClient()
	if helper.IsAvailable() {
		helper.StopVPN()
	}

	b.status = StatusDisconnected
	b.virtualIP = ""
	b.communityName = ""
	runtime.EventsEmit(b.ctx, "status-changed", StatusDisconnected)
}
