package main

import (
	log "github.com/sirupsen/logrus"
	edgecli "gitlab.com/omniedge/omniedge-linux-saas-cli"
	rootCmd "gitlab.com/omniedge/omniedge-linux-saas-cli/cmd/edgecli/cmd"
)

var Env string

func main() {
	edgecli.Env = Env
	if Env == "" {
		edgecli.Env = "dev"
	}
	log.Infof("You are in mode: %s", edgecli.Env)
	log.SetFormatter(&log.TextFormatter{
		TimestampFormat:        "2006-01-02T15:04:05",
		FullTimestamp:          true,
		DisableLevelTruncation: true,
	})
	log.SetLevel(log.InfoLevel)
	rootCmd.Execute()
}
