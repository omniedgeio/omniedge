package cmd

import (
	core "github.com/omniedgeio/omniedge/pkg/core"
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
		core.LoadClientConfig()
		var err error
		if err = loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		var randomMac string
		if randomMac, err = core.GenerateRandomMac(); err != nil {
			log.Errorf("%+v", err)
			return
		}

		// Get actual hardware UUID for heartbeat
		hardwareId, _ := core.RevealHardwareUUID()

		var startOption = core.StartOption{
			Hostname:      viper.GetString(keyDeviceName),
			DeviceMac:     randomMac,
			CommunityName: viper.GetString(keyJoinVirtualNetworkCommunityName),
			VirtualIP:     viper.GetString(keyJoinVirtualNetworkVirtualIP),
			SecretKey:     viper.GetString(keyJoinVirtualNetworkSecretKey),
			DeviceMask:    viper.GetString(keyJoinVirtualNetworkNetMask),
			SuperNode:     viper.GetString(keyJoinVirtualNetworkSuperNode),
			EnableRouting: viper.GetBool(cliEnableRouting),
			Token:         viper.GetString(keyAuthResponseToken),
			BaseUrl:       core.ConfigV.GetString(RestEndpointUrl),
			HardwareUUID:  hardwareId,
			ExitNodeIP:    viper.GetString(cliExitNode),
			IsExitNode:    viper.GetBool(cliAsExitNode) || viper.GetBool(keyJoinVirtualNetworkAsExitNode),
			NetworkID:     viper.GetString(keyJoinVirtualNetworkNetworkID),
		}
		var service = core.StartService{
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
		enableRoutine  bool
		exitNode       string
	)
	startCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	startCmd.Flags().BoolVarP(&enableRoutine, cliEnableRouting, "r", false, "enable routing")
	startCmd.Flags().StringVarP(&exitNode, cliExitNode, "e", "", "exit node ip address")
	startCmd.Flags().Bool(cliAsExitNode, false, "enable this device as an exit node")
	viper.BindPFlag(cliEnableRouting, startCmd.Flags().Lookup(cliEnableRouting))
	viper.BindPFlag(cliExitNode, startCmd.Flags().Lookup(cliExitNode))
	viper.BindPFlag(cliAsExitNode, startCmd.Flags().Lookup(cliAsExitNode))
	rootCmd.AddCommand(startCmd)
}
