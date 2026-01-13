package cmd

import (
	"fmt"
	"os"
	"strings"

	core "github.com/omniedgeio/omniedge/pkg/core"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var statusCmd = &cobra.Command{
	Use:   "status",
	Short: "Show OmniEdge running status",
	Run: func(cmd *cobra.Command, args []string) {
		pidFile := core.GetPidFile()
		data, err := os.ReadFile(pidFile)

		isRunning := false
		pid := ""
		if err == nil {
			pid = strings.TrimSpace(string(data))
			// Check if process exists
			if out, err := core.RunCmd("ps", "-p", pid); err == nil && !strings.Contains(out, "PID") {
				// ps might just return header on some systems or error on others
				isRunning = true
			} else if err == nil {
				isRunning = true
			}
		}

		if !isRunning {
			fmt.Println("Status: Offline")
			return
		}

		fmt.Println("Status: Online")
		fmt.Printf("PID:    %s\n", pid)

		core.LoadClientConfig()
		loadAuthFile()

		vip := viper.GetString(keyJoinVirtualNetworkVirtualIP)
		if vip != "" {
			fmt.Printf("IP:     %s\n", vip)
		}

		netId := viper.GetString(keyJoinVirtualNetworkNetworkID)
		if netId != "" {
			fmt.Printf("Network ID: %s\n", netId)
		}

		fmt.Printf("Log:    %s\n", core.GetLogFile())
	},
}

func init() {
	rootCmd.AddCommand(statusCmd)
}
