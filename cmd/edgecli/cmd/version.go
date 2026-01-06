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
		fmt.Printf("Omniedge %s\n", Version)
	},
}

func init() {
	rootCmd.AddCommand(versionCmd)
}
