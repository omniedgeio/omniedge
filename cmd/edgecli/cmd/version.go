package cmd

import (
	"fmt"

	"github.com/spf13/cobra"
)

var versionCmd = &cobra.Command{
	Use:     "version",
	Aliases: []string{},
	Short:   "",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("Omniedge 0.3.0")
	},
}

func init() {
	rootCmd.AddCommand(versionCmd)
}
