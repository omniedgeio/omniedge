package cmd

import (
	"fmt"
	"github.com/spf13/cobra"
)

var versionCmd = &cobra.Command{
	Use:     "version",
	Aliases: []string{},
	Short:   "",
	RunE: func(cmd *cobra.Command, args []string) error {
		fmt.Println("Omniedge 0.2.1")
		return nil
	},
}

func init() {
	rootCmd.AddCommand(versionCmd)
}
