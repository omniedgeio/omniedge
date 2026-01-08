//go:build windows
// +build windows

package omnin2n

// Edge is a stub structure for Windows to satisfy build constraints
type Edge struct {
	Id         string
	PrivateKey string

	AllowP2P             bool
	AllowRouting         bool
	CommunityName        string
	DisablePMTUDiscovery bool
	DropMulticast        bool
	EncryptKey           string
	LocalPort            int
	ManagementPort       int
	MTU                  int
	NumberMaxSnPings     int
	SuperNodeNum         int
	TransopId            int
	Tos                  int
	RegisterInterval     int
	RegisterTTL          int
	SuperNodeHostPort    string

	DeviceIP     string
	DeviceMac    string
	DeviceMask   string
	DeviceName   string
	DeviceIPMode string
}

func (e *Edge) Configure() error {
	return nil
}

func (e *Edge) OpenTunTapDevice() error {
	return nil
}

func (e *Edge) Start() error {
	return nil
}

func (e *Edge) Stop() {
}
