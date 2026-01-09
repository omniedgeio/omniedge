package api

import (
	"time"
)

type SuccessResponse struct {
	Code    int         `json:"-"`
	Message string      `json:"message"`
	Data    interface{} `json:"data"`
}

func (err SuccessResponse) Error() string {
	return err.Message
}

func NewSuccessResponse(message string, data ...interface{}) error {
	res := SuccessResponse{
		Code:    200,
		Message: message,
	}
	if len(data) == 1 {
		res.Data = data[0]
	} else if len(data) > 1 {
		res.Data = data
	} else {
		res.Data = nil
	}
	return res
}

type ErrorResponse struct {
	Code    int         `json:"-"`
	Message string      `json:"message"`
	Errors  interface{} `json:"errors"`
	// OAuth error field for OAuth 2.0 error responses
	OAuthError string `json:"error"`
}

func (err ErrorResponse) Error() string {
	// Return OAuth error if message is empty (for OAuth endpoints)
	if err.Message == "" && err.OAuthError != "" {
		return err.OAuthError
	}
	return err.Message
}

type AuthTokenResponse struct {
	Token string `json:"token"`
}

type ResetPasswordResponse struct {
	Email string `json:"email"`
}

type IdentityResponse struct {
	Provider string            `json:"provider"`
	Enabled  bool              `json:"enabled"`
	MetaData map[string]string `json:"metadata"`
}

type ProfileResponse struct {
	ID         string              `json:"id"`
	Name       string              `json:"name"`
	Email      string              `json:"email"`
	Picture    string              `json:"picture"`
	Identities []*IdentityResponse `json:"identities"`
}

type DeviceResponse struct {
	ID              string                          `json:"id"`
	Name            string                          `json:"name"`
	OS              string                          `json:"os"`
	VirtualNetworks []*DeviceVirtualNetworkResponse `json:"virtual_networks,omitempty"`
	Subnets         []*DeviceSubnetRouteResponse    `json:"subnets,omitempty"`
}

type DeviceVirtualNetworkResponse struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	VirtualIP string    `json:"virtual_ip"`
	LastSeen  time.Time `json:"last_seen"`
	Online    bool      `json:"online"`
}

type DeviceSubnetRouteResponse struct {
	ID         string                       `json:"id"`
	IP         string                       `json:"ip"`
	MacAddr    string                       `json:"mac_addr"`
	SubnetMask string                       `json:"subnet_mask"`
	Devices    []*SubnetRouteDeviceResponse `json:"devices"`
}

type SubnetRouteDeviceResponse struct {
	ID           string `json:"id"`
	Name         string `json:"name"`
	IP           string `json:"ip"`
	MacAddr      string `json:"mac_addr"`
	Manufacturer string `json:"manufacturer"`
}

type ServerResponse struct {
	ID      string `json:"id,omitempty"`
	Name    string `json:"name"`
	Country string `json:"country"`
	Host    string `json:"host,omitempty"`
}

type VirtualNetworkResponse struct {
	ID      string                          `json:"id"`
	Name    string                          `json:"name"`
	IPRange string                          `json:"ip_range"`
	Role    int                             `json:"role"`
	Server  *ServerResponse                 `json:"server,omitempty"`
	Devices []*VirtualNetworkDeviceResponse `json:"devices,omitempty"`
	Users   []*VirtualNetworkUserResponse   `json:"users,omitempty"`
}

type VirtualNetworkDeviceResponse struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	VirtualIP string    `json:"virtual_ip"`
	LastSeen  time.Time `json:"last_seen"`
	Online    bool      `json:"online"`
}

type JoinVirtualNetworkResponse struct {
	CommunityName string          `json:"community_name"`
	SecretKey     string          `json:"secret_key"`
	VirtualIP     string          `json:"virtual_ip"`
	SubnetMask    string          `json:"subnet_mask"`
	Server        *ServerResponse `json:"server"`
}

type SecurityKeyResponse struct {
	UUID      string    `json:"uuid"`
	Key       string    `json:"key"`
	KeyType   int16     `json:"key_type"`
	ExpiredAt time.Time `json:"expired_at"`
	CreatedAt time.Time `json:"created_at"`
}

type VirtualNetworkUserResponse struct {
	UUID     string    `json:"uuid"`
	Email    string    `json:"email"`
	Name     string    `json:"name"`
	Role     int       `json:"role,omitempty"`
	JoinedAt time.Time `json:"joined_at,omitempty"`
}

type InvitationResponse struct {
	UUID           string                      `json:"uuid"`
	User           *VirtualNetworkUserResponse `json:"user,omitempty"`
	InvitedBy      string                      `json:"invited_by"`
	InvitedAt      time.Time                   `json:"invited_at"`
	VirtualNetwork string                      `json:"virtual_network"`
}

type ScanResult struct {
	HostName   string `mapstructure:"hostName"`
	IPv4       string `mapstructure:"ipv4"`
	IPv6       string `mapstructure:"ipv6"`
	MacAddress string `mapstructure:"macAddress"`
	Vendor     string `mapstructure:"vendor"`
	OS         string `mapstructure:"os"`
}
