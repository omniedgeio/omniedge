package cmd

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	core "github.com/omniedgeio/omniedge/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
)

var stopCmd = &cobra.Command{
	Use:   "stop",
	Short: "Stop the background OmniEdge process",
	Run: func(cmd *cobra.Command, args []string) {
		pidFile := core.GetPidFile()
		data, err := os.ReadFile(pidFile)
		if err != nil {
			log.Errorf("OmniEdge is not running (could not read %s)", pidFile)
			return
		}

		pid, err := strconv.Atoi(strings.TrimSpace(string(data)))
		if err != nil {
			log.Errorf("Invalid PID in %s", pidFile)
			return
		}

		process, err := os.FindProcess(pid)
		if err != nil {
			log.Errorf("Process %d not found", pid)
			os.Remove(pidFile)
			return
		}

		fmt.Printf("Stopping OmniEdge (PID: %d)...\n", pid)
		// Signal clean exit
		err = process.Signal(os.Interrupt)
		if err != nil {
			// Fallback to Kill if Interrupt fails
			process.Kill()
		}

		os.Remove(pidFile)
		fmt.Println("OmniEdge stopped.")
	},
}

func init() {
	rootCmd.AddCommand(stopCmd)
}
