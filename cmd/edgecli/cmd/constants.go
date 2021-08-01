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
	keyVirtualNetworkIds = "virtualnetworkids"
	keyHostname          = "keyHostname"
	keyDevice            = "device"
	KeyDeviceUUID        = "device.uuid"

	keyJoinVirtualNetwork = "joinVirtualNetwork"
)

const (
	CouldNotBindFlags = "Could not bind flags"
)
