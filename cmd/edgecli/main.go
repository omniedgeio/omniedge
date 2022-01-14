package main

import (
	log "github.com/sirupsen/logrus"
	edgecli "gitlab.com/omniedge/omniedge-linux-saas-cli"
	rootCmd "gitlab.com/omniedge/omniedge-linux-saas-cli/cmd/edgecli/cmd"
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
