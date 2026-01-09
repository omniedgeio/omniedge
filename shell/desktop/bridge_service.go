package main

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"runtime"
	"strconv"
	"sync"
	"time"

	"github.com/omniedgeio/omniedge/pkg/api"
	"github.com/omniedgeio/omniedge/pkg/bridge"
	"github.com/omniedgeio/omniedge/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/skratchdot/open-golang/open"
	"github.com/wailsapp/wails/v3/pkg/application"
)

// Token storage file path
var tokenFilePath string

func init() {
	// Use cross-platform app config directory:
	// macOS: ~/Library/Application Support
	// Windows: %AppData%
	// Linux: ~/.config
	configDir, err := os.UserConfigDir()
	if err != nil {
		configDir = os.TempDir() // Fallback to temp
	}
	omniedgeDir := filepath.Join(configDir, "OmniEdge")
	os.MkdirAll(omniedgeDir, 0700)
	tokenFilePath = filepath.Join(omniedgeDir, "tokens.json")
}

// TokenData represents the stored token data
type TokenData struct {
	Token        string `json:"token"`
	RefreshToken string `json:"refresh_token"`
}

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
	mu                   sync.Mutex // Protects concurrent access to token operations
	app                  *application.App
	mainWindow           *application.WebviewWindow // Reference to main window for resizing
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

// SetMainWindow sets the main window reference for dynamic resizing
func (b *BridgeService) SetMainWindow(window *application.WebviewWindow) {
	b.mainWindow = window
}

// ResizeWindow resizes the main window to the specified height (width stays fixed at 320)
func (b *BridgeService) ResizeWindow(height int) {
	if b.mainWindow == nil {
		log.Warn("BridgeService: Cannot resize - mainWindow is nil")
		return
	}
	// Clamp height to reasonable bounds
	if height < 200 {
		height = 200
	} else if height > 800 {
		height = 800
	}
	log.Debugf("BridgeService: Resizing window to 320x%d", height)
	b.mainWindow.SetSize(320, height)
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

	// Save tokens to keychain for persistence
	b.SaveTokens()

	// Check for existing connection after login
	b.CheckExistingConnection()

	return LoginResult{Success: true, Message: "Login successful"}
}

// StartBrowserLogin initiates a session-based browser login flow
func (b *BridgeService) StartBrowserLogin() QRLoginResult {
	sessionService := api.SessionService{
		HttpOption: api.HttpOption{BaseUrl: b.baseURL},
	}

	// 1. Generate a new session
	log.Infof("StartBrowserLogin: Generating session at %s", b.baseURL)
	session, err := sessionService.GenerateSession()
	if err != nil {
		log.Errorf("StartBrowserLogin: Failed to generate session: %v", err)
		return QRLoginResult{Success: false, Message: "Failed to create session: " + err.Error()}
	}

	log.Infof("StartBrowserLogin: Session created: %s, AuthURL: %s", session.ID, session.AuthURL)

	// 2. Launch system browser to AuthURL
	log.Infof("StartBrowserLogin: launching browser with URL: %s", session.AuthURL)
	if err := open.Run(session.AuthURL); err != nil {
		log.Errorf("StartBrowserLogin: failed to open browser: %v", err)
		return QRLoginResult{Success: false, Message: "failed to open browser"}
	}

	// 3. Start background task to wait for tokens
	b.mu.Lock()
	if qrLoginCancel != nil {
		close(qrLoginCancel)
	}
	qrLoginCancel = make(chan bool)
	cancelChan := qrLoginCancel
	b.mu.Unlock()

	go b.waitForSessionToken(session.ID, cancelChan)

	return QRLoginResult{
		Success: true,
		Message: "Login session started",
		Info: &QRLoginInfo{
			SessionID: session.ID,
			AuthURL:   session.AuthURL,
			QRData:    session.AuthURL,
			ExpiresAt: session.ExpiredAt.Format("2006-01-02T15:04:05Z07:00"),
		},
	}
}

// QRLoginInfo contains information for QR code login display
type QRLoginInfo struct {
	SessionID string `json:"session_id"`
	AuthURL   string `json:"auth_url"`
	QRData    string `json:"qr_data"` // URL to encode in QR code
	ExpiresAt string `json:"expires_at"`
}

// QRLoginResult represents the result of a QR login attempt
type QRLoginResult struct {
	Success bool         `json:"success"`
	Message string       `json:"message"`
	Info    *QRLoginInfo `json:"info,omitempty"`
}

// qrLoginCancel is used to cancel pending QR login
var qrLoginCancel chan bool

// StartQRLogin initiates a QR code login session
func (b *BridgeService) StartQRLogin() QRLoginResult {
	b.mu.Lock()
	defer b.mu.Unlock()

	// Cancel any existing QR login session
	if qrLoginCancel != nil {
		close(qrLoginCancel)
	}

	sessionService := api.SessionService{
		HttpOption: api.HttpOption{BaseUrl: b.baseURL},
	}

	// Generate a new session
	session, err := sessionService.GenerateSession()
	if err != nil {
		log.Errorf("BridgeService.StartQRLogin: Failed to generate session: %v", err)
		return QRLoginResult{Success: false, Message: err.Error()}
	}

	log.Infof("BridgeService.StartQRLogin: Session created: %s", session.ID)

	// Create cancel channel for this session
	qrLoginCancel = make(chan bool)
	cancelChan := qrLoginCancel

	// Start WebSocket listener in background
	go b.waitForSessionToken(session.ID, cancelChan)

	return QRLoginResult{
		Success: true,
		Message: "QR login session started",
		Info: &QRLoginInfo{
			SessionID: session.ID,
			AuthURL:   session.AuthURL,
			QRData:    session.AuthURL,
			ExpiresAt: session.ExpiredAt.Format("2006-01-02T15:04:05Z07:00"),
		},
	}
}

func (b *BridgeService) waitForSessionToken(sessionID string, cancelChan chan bool) {
	defer func() {
		b.mu.Lock()
		if qrLoginCancel == cancelChan {
			qrLoginCancel = nil
		}
		b.mu.Unlock()
	}()

	sessionService := api.SessionService{
		HttpOption: api.HttpOption{BaseUrl: b.baseURL},
	}

	// Wait for tokens via WebSocket (15 minutes timeout)
	log.Infof("waitForSessionToken: Connecting to WebSocket for session %s...", sessionID)
	tokenResp, err := sessionService.ConnectAndWaitForToken(sessionID, 900)

	select {
	case <-cancelChan:
		log.Infof("waitForSessionToken: login cancelled for session %s", sessionID)
		return
	default:
	}

	if err != nil {
		log.Warnf("waitForSessionToken: error waiting for token for session %s: %v", sessionID, err)
		if b.app != nil {
			b.app.Event.Emit("login-failed", "Authentication failed: "+err.Error())
		}
		return
	}

	log.Infof("waitForSessionToken: Successfully received tokens for session %s", sessionID)
	b.mu.Lock()
	b.token = "Bearer " + tokenResp.Token
	b.refreshToken = tokenResp.RefreshToken
	b.mu.Unlock()

	log.Infof("waitForSessionToken: Login successful for session %s", sessionID)

	// Save tokens
	b.SaveTokens()

	// Securely save to keychain
	authJSON, _ := json.Marshal(tokenResp)
	_ = core.SaveSecureToken(string(authJSON))

	// Check for existing connection
	b.CheckExistingConnection()

	// Notify frontend
	if b.app != nil {
		b.app.Event.Emit("login-success", "Login successful")
	}
}

// CancelQRLogin cancels any pending QR login session
func (b *BridgeService) CancelQRLogin() {
	b.mu.Lock()
	defer b.mu.Unlock()

	if qrLoginCancel != nil {
		log.Info("BridgeService.CancelQRLogin: Cancelling QR login")
		close(qrLoginCancel)
		qrLoginCancel = nil
	}
}

// SaveTokens persists auth tokens to a file
func (b *BridgeService) SaveTokens() error {
	data := TokenData{
		Token:        b.token,
		RefreshToken: b.refreshToken,
	}
	jsonData, err := json.Marshal(data)
	if err != nil {
		log.Warnf("Failed to marshal tokens: %v", err)
		return err
	}
	if err := os.WriteFile(tokenFilePath, jsonData, 0600); err != nil {
		log.Warnf("Failed to save tokens to file: %v", err)
		return err
	}
	log.Info("BridgeService: Tokens saved to file")
	return nil
}

// LoadTokens loads auth tokens from a file
func (b *BridgeService) LoadTokens() error {
	jsonData, err := os.ReadFile(tokenFilePath)
	if err != nil {
		log.Debug("No token file found")
		return fmt.Errorf("no tokens found")
	}
	var data TokenData
	if err := json.Unmarshal(jsonData, &data); err != nil {
		log.Warnf("Failed to unmarshal tokens: %v", err)
		return err
	}
	b.token = data.Token
	b.refreshToken = data.RefreshToken
	log.Info("BridgeService: Tokens loaded from file")
	return nil
}

// ClearTokens removes auth tokens file
func (b *BridgeService) ClearTokens() error {
	os.Remove(tokenFilePath)
	b.token = ""
	b.refreshToken = ""
	log.Info("BridgeService: Tokens cleared")
	return nil
}

// TryAutoLogin attempts to restore session from saved tokens
func (b *BridgeService) TryAutoLogin() LoginResult {
	b.mu.Lock()
	defer b.mu.Unlock()

	// If already logged in, don't try again
	if b.token != "" {
		log.Debug("BridgeService: Already logged in, skipping auto-login")
		return LoginResult{Success: true, Message: "Already logged in"}
	}

	log.Info("BridgeService: Attempting auto-login from saved tokens")

	// Load tokens from file
	if err := b.LoadTokens(); err != nil {
		log.Debug("BridgeService: No saved tokens, auto-login failed")
		return LoginResult{Success: false, Message: "No saved session"}
	}

	// Validate that we have a refresh token
	if b.refreshToken == "" {
		log.Debug("BridgeService: No refresh token, auto-login failed")
		return LoginResult{Success: false, Message: "No refresh token"}
	}

	// Try to refresh the token
	authService := api.AuthService{
		HttpOption: api.HttpOption{
			BaseUrl: b.baseURL,
		},
	}
	resp, err := authService.Refresh(&api.RefreshTokenOption{
		RefreshToken: b.refreshToken,
	})
	if err != nil {
		log.Warnf("BridgeService: Token refresh failed: %v", err)
		b.ClearTokens()
		return LoginResult{Success: false, Message: "Session expired"}
	}

	// Update tokens
	b.token = "Bearer " + resp.Token
	b.refreshToken = resp.RefreshToken
	b.SaveTokens()

	log.Info("BridgeService: Auto-login successful via token refresh")

	// Check for existing connection
	b.CheckExistingConnection()

	return LoginResult{Success: true, Message: "Session restored"}
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
		OS:           runtime.GOOS,
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

// Quit disconnects the VPN and quits the application
func (b *BridgeService) Quit() {
	log.Info("BridgeService: Quit called - disconnecting and exiting")
	b.Disconnect()
	if b.app != nil {
		b.app.Quit()
	}
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
	Online    bool   `json:"online"` // Calculated field - true if last_seen within 5 minutes
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
	now := time.Now()
	for i, d := range devs {
		// Calculate online status: device is online if last_seen within 5 minutes
		isOnline := now.Sub(d.LastSeen) < 5*time.Minute
		log.Debugf("BridgeService: Device %d: ID=%s, Name=%s, VirtualIP=%s, LastSeen=%v, Online=%v",
			i, d.ID, d.Name, d.VirtualIP, d.LastSeen, isOnline)
		result[i] = DeviceWithNetwork{
			VirtualNetworkDeviceResponse: d,
			NetworkID:                    networkID,
			Online:                       isOnline,
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
