package cmd

const (
	authFileDefault   = "/root/.omniedge/auth.json"
	scanResultDefault = "/root/.omniedge/scan.json"
	cliUsername       = "username"
	cliPassword       = "password"
	cliSecretKey      = "secretKey"
	cliAuthConfigFile = "file"

	cliInterface = "interface"

	cliVirtualNetworkId = "network"

	cliScanTimeout = "timeout"
	cliCidr        = "cidr"
	cliScanResult  = "scan-result"
)

const (
	omniedgeSecretKey = "OMNIEDGE_SECRET_KEY"
)

const (
	RestEndpointUrl = "rest-endpoint-url"
)

var (
	keyAuthResponse      = "authresponse"
	keyAuthResponseToken = "authresponse.token"
	keyVirtualNetworks   = "virtualNetworks"
	keyHostname          = "keyHostname"
	keyDevice            = "device"
	keyDeviceName        = "device.name"
	KeyDeviceUUID        = "device.uuid"

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
