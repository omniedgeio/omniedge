package cmd

const (
	authFileDefault   = "/root/.omniedge/auth.json"
	cliUsername       = "username"
	cliPassword       = "password"
	cliSecretKey      = "secretKey"
	cliAuthConfigFile = "file"

	cliInterface = "interface"

	cliVirtualNetworkId = "network"
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
)

const (
	CouldNotBindFlags = "Could not bind flags"
)
