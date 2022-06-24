package cmd

import (
	edgecli "github.com/omniedgeio/omniedge-cli"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var startCmd = &cobra.Command{
	Use:     "start",
	Aliases: []string{},
	Short:   "",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		edgecli.LoadClientConfig()
		var err error
		if err = loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		var randomMac string
		if randomMac, err = edgecli.GenerateRandomMac(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		var startOption = edgecli.StartOption{
			Hostname:      viper.GetString(keyDeviceName),
			DeviceMac:     randomMac,
			CommunityName: viper.GetString(keyJoinVirtualNetworkCommunityName),
			VirtualIP:     viper.GetString(keyJoinVirtualNetworkVirtualIP),
			SecretKey:     viper.GetString(keyJoinVirtualNetworkSecretKey),
			DeviceMask:    viper.GetString(keyJoinVirtualNetworkNetMask),
			SuperNode:     viper.GetString(keyJoinVirtualNetworkSuperNode),
		}
		var service = edgecli.StartService{
			StartOption: startOption,
		}
		if err := service.Start(); err != nil {
			log.Fatalf("%+v", err)
		}
	},
}

func init() {
	var (
		authConfigPath string
	)
	startCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	//rootCmd.AddCommand(startCmd)
}
