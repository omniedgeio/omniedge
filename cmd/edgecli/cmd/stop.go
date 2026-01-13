package cmd

import (
	"fmt"
	"os"
	"os/exec"
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
		if err := process.Signal(os.Interrupt); err != nil {
			// Permission error? Try sudo
			if strings.Contains(err.Error(), "operation not permitted") {
				fmt.Printf("Permission denied. Attempting to stop with sudo...\n")
				cmd := exec.Command("sudo", "kill", "-SIGINT", strconv.Itoa(pid))
				cmd.Stdout = os.Stdout
				cmd.Stderr = os.Stderr
				if err := cmd.Run(); err != nil {
					log.Errorf("Failed to stop process with sudo: %v", err)
				} else {
					fmt.Println("OmniEdge stopped via sudo.")
				}
			} else {
				log.Errorf("Failed to signal process: %v", err)
			}
		} else {
			fmt.Println("OmniEdge stopped.")
		}

		// Clean up PID file just in case the daemon didn't
		os.Remove(pidFile)
	},
}

func init() {
	rootCmd.AddCommand(stopCmd)
}
