package cmd

const (
	authFileDefault   = "/root/.omniedge/auth.json"
	cliUsername       = "username"
	cliPassword       = "password"
	cliSecretKey      = "secretKey"
	cliAuthConfigFile = "file"

	cliInterface = "interface"
)

const (
	omniedgeSecretKey = "OMNIEDGE_SECRET_KEY"
)

const (
	RestEndpointUrl = "rest-endpoint-url"
)

var (
	keyAuthResponse             = "authresponse"
	keyAuthResponseIdToken      = "authresponse.idtoken"
	keyAuthResponseRefreshToken = "authresponse.refreshtoken"
	keyVirtualNetworkIds        = "virtualnetworkids"
	keyHostname                 = "keyHostname"
)

const (
	CouldNotBindFlags = "Could not bind flags"
)
