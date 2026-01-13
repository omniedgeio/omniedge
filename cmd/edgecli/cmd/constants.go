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
	cliSecretKey        = "secretKey"
	cliAuthConfigFile   = "file"
	cliVirtualNetworkId = "network"
	cliScanTimeout      = "timeout"
	cliCidr             = "cidr"
	cliScanResult       = "scan-result"
	cliEnableRouting    = "enable-routing"
	cliExitNode         = "exit-node"
	cliAsExitNode       = "as-exit-node"
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
	keyAuthResponseRefreshToken = "authresponse.refresh_token"
	keyVirtualNetworks          = "virtualNetworks"
	keyDevice                   = "device"
	keyDeviceName               = "device.name"
	keyDeviceUUID               = "device.uuid"

	keyJoinVirtualNetwork              = "joinVirtualNetwork"
	keyJoinVirtualNetworkCommunityName = "joinVirtualNetwork.community_name"
	keyJoinVirtualNetworkSecretKey     = "joinVirtualNetwork.secret_key"
	keyJoinVirtualNetworkVirtualIP     = "joinVirtualNetwork.virtual_ip"
	keyJoinVirtualNetworkNetMask       = "joinVirtualNetwork.subnet_mask"
	keyJoinVirtualNetworkSuperNode     = "joinVirtualNetwork.server.host"
	keyJoinVirtualNetworkNetworkID     = "joinVirtualNetwork.network_id"
	keyJoinVirtualNetworkAsExitNode    = "joinVirtualNetwork.as_exit_node"

	keyScanResult     = "scan.result"
	keyScanIP         = "scan.ip"
	keyScanMacAddress = "scan.mac_address"
	keyScanSubnetMask = "scan.subnet_mask"
)

const (
	CouldNotBindFlags = "Could not bind flags"
)
