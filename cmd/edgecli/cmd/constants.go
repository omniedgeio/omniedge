package cmd

type CliOption struct {
	AuthFileDefaultPath   string
	ScanResultDefaultPath string
}

var Option = CliOption{
	AuthFileDefaultPath:   "~/.omniedge/auth.json",
	ScanResultDefaultPath: "~/.omniedge/scan.json",
}

const (
	cliUsername       = "username"
	cliPassword       = "password"
	cliSecretKey      = "secretKey"
	cliAuthConfigFile = "file"

	cliInterface = "interface"

	cliVirtualNetworkId = "network"

	cliScanTimeout   = "timeout"
	cliCidr          = "cidr"
	cliScanResult    = "scan-result"
	cliEnableRouting = "enable-routing"
)

const (
	omniedgeSecretKey = "OMNIEDGE_SECRET_KEY"
)

const (
	RestEndpointUrl = "rest-endpoint-url"
)

var (
	keyAuthResponse             = "authresponse"
	keyAuthResponseToken        = "authresponse.token"
	keyAuthResponseRefreshToken = "authresponse.refreshtoken"
	keyVirtualNetworks          = "virtualNetworks"
	keyHostname                 = "keyHostname"
	keyDevice                   = "device"
	keyDeviceName               = "device.name"
	keyDeviceUUID               = "device.uuid"

	keyJoinVirtualNetwork              = "joinVirtualNetwork"
	keyJoinVirtualNetworkCommunityName = "joinVirtualNetwork.community_name"
	keyJoinVirtualNetworkSecretKey     = "joinVirtualNetwork.secret_key"
	keyJoinVirtualNetworkVirtualIP     = "joinVirtualNetwork.virtual_ip"
	keyJoinVirtualNetworkNetMask       = "joinVirtualNetwork.subnet_mask"
	keyJoinVirtualNetworkSuperNode     = "joinVirtualNetwork.server.host"

	keyScanResult     = "scan.result"
	keyScanIP         = "scan.ip"
	keyScanMacAddress = "scan.mac_address"
	keyScanSubnetMask = "scan.subnet_mask"
)

const (
	CouldNotBindFlags = "Could not bind flags"
)
