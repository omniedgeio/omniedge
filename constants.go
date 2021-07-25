package edgecli

import "github.com/spf13/viper"

var ConfigV = viper.New()

const (
	FailGetMacAddress = "fail to get mac address"
)

const (
	RestJoinOmniEdge = "client-join-url"
	GraphqlEndpoint  = "graphql-endpoint"
)

const (
	ContentType   = "Content-Type"
	ContentJson   = "application/json"
	Authorization = "authorization"
)

const ()
