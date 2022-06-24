package main

import (
	edgecli "github.com/omniedgeio/omniedge-cli"
	rootCmd "github.com/omniedgeio/omniedge-cli/cmd/edgecli/cmd"
	log "github.com/sirupsen/logrus"
	"os"
	"os/user"
)

var Env string

func main() {
	edgecli.Env = Env
	if Env == "" {
		edgecli.Env = "dev"
	}
	username := os.Getenv("SUDO_USER")
	u, err := user.Lookup(username)
	if err == nil {
		rootCmd.Option.AuthFileDefaultPath = u.HomeDir + "/.omniedge/auth.json"
		rootCmd.Option.ScanResultDefaultPath = u.HomeDir + "/.omniedge/scan.json"
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
