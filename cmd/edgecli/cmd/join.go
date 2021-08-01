package cmd

import (
	"fmt"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	edgecli "gitlab.com/omniedge/omniedge-linux-saas-cli"
)

var joinCmd = &cobra.Command{
	Use:     "join",
	Aliases: []string{},
	Short:   "",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		edgecli.LoadClientConfig()
		if err := loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		endpointUrl := edgecli.ConfigV.GetString(RestEndpointUrl)
		var vnId = viper.GetString(cliVirtualNetworkId)
		var deviceId = viper.GetString(KeyDeviceUUID)

		var joinOption = edgecli.JoinOption{
			Token:            fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl:          endpointUrl,
			VirtualNetworkId: vnId,
			DeviceId:         deviceId,
		}
		var service = edgecli.VirtualNetworkService{
			joinOption,
		}
		var resp *edgecli.JoinVirtualNetworkResponse
		var err error
		if resp, err = service.Join(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		viper.Set(keyJoinVirtualNetwork, resp)
		persistAuthFile()
		log.Infof("Success to join virtual network")
	},
}

func init() {
	var (
		networkId      string
		authConfigPath string
	)
	joinCmd.Flags().StringVarP(&networkId, cliVirtualNetworkId, "n", "", "id of the virtual network which you want to join")
	_ = registerCmd.MarkFlagRequired(cliVirtualNetworkId)
	joinCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	rootCmd.AddCommand(joinCmd)
}
