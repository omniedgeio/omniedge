package main

import (
	core "github.com/omniedgeio/omniedge-cli/pkg/core"
	rootCmd "github.com/omniedgeio/omniedge-cli/cmd/edgecli/cmd"
	log "github.com/sirupsen/logrus"
	"os"
	"os/user"
)

var Env string

func main() {
	core.Env = Env
	if Env == "" {
		core.Env = "dev"
	}
	username := os.Getenv("SUDO_USER")
	u, err := user.Lookup(username)
	if err == nil {
		rootCmd.Option.AuthFileDefaultPath = u.HomeDir + "/.omniedge/auth.json"
		rootCmd.Option.ScanResultDefaultPath = u.HomeDir + "/.omniedge/scan.json"
	}

	log.Infof("You are in mode: %s", core.Env)
	log.SetFormatter(&log.TextFormatter{
		TimestampFormat:        "2006-01-02T15:04:05",
		FullTimestamp:          true,
		DisableLevelTruncation: true,
	})
	log.SetLevel(log.InfoLevel)
	rootCmd.Execute()
}
