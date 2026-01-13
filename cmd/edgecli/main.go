package main

import (
	"os"
	"os/user"

	rootCmd "github.com/omniedgeio/omniedge/cmd/edgecli/cmd"
	core "github.com/omniedgeio/omniedge/pkg/core"
	log "github.com/sirupsen/logrus"
)

var Env string

func main() {
	core.Env = Env
	if Env == "" {
		core.Env = "dev"
	}

	daemonEnv := os.Getenv("OMNIEDGE_DAEMON")
	log.Debugf("Starting OmniEdge (PID: %d, Env: %s, Daemon: %s)", os.Getpid(), core.Env, daemonEnv)

	username := os.Getenv("SUDO_USER")
	if username != "" {
		u, err := user.Lookup(username)
		if err == nil {
			rootCmd.Option.AuthFileDefaultPath = u.HomeDir + "/.omniedge/auth.json"
			rootCmd.Option.ScanResultDefaultPath = u.HomeDir + "/.omniedge/scan.json"
			log.Debugf("SUDO_USER detected: %s. Using auth path: %s", username, rootCmd.Option.AuthFileDefaultPath)
		}
	}

	log.Debugf("You are in mode: %s", core.Env)
	log.SetFormatter(&log.TextFormatter{
		TimestampFormat:        "2006-01-02T15:04:05",
		FullTimestamp:          true,
		DisableLevelTruncation: true,
	})
	log.SetLevel(log.InfoLevel)
	rootCmd.Execute()
}
