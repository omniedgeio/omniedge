package main

import (
	log "github.com/sirupsen/logrus"
	rootCmd "gitlab.com/omniedge/omniedge-linux-saas-cli/cmd/edgecli/cmd"
)

func main() {
	log.SetFormatter(&log.TextFormatter{
		TimestampFormat:        "2006-01-02T15:04:05",
		FullTimestamp:          true,
		DisableLevelTruncation: true,
	})
	rootCmd.Execute()
}
