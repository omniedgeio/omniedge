package cmd

import (
	"fmt"

	api "github.com/omniedgeio/omniedge/pkg/api"
	core "github.com/omniedgeio/omniedge/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var joinCmd = &cobra.Command{
	Use:     "join",
	Aliases: []string{},
	Short:   "Join omniedge network",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		core.LoadClientConfig()
		if err := loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		endpointUrl := core.ConfigV.GetString(RestEndpointUrl)
		var vnId = viper.GetString(cliVirtualNetworkId)
		var deviceId = viper.GetString(keyDeviceUUID)
		var deviceName = viper.GetString(keyDeviceName)

		var device *api.DeviceResponse
		var err error

		refreshToken := viper.GetString(keyAuthResponseRefreshToken)
		if refreshToken != "" {
			refreshTokenOption := &api.RefreshTokenOption{
				RefreshToken: refreshToken,
			}
			var refreshTokenHttpOption = api.HttpOption{
				BaseUrl: endpointUrl,
			}
			authService := api.AuthService{
				HttpOption: refreshTokenHttpOption,
			}
			if authResp, err := authService.Refresh(refreshTokenOption); err != nil {
				log.Errorf("%+v", err)
				return
			} else {
				viper.Set(keyAuthResponse, authResp)
				viper.Set(keyAuthResponseToken, authResp.Token)
				if authResp.RefreshToken != "" {
					viper.Set(keyAuthResponseRefreshToken, authResp.RefreshToken)
				}
			}
		}

		var httpOption = api.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl: endpointUrl,
		}
		//check device id exists in config
		if deviceId == "" || deviceName == "" {
			if device, err = register(httpOption); err != nil {
				log.Errorf("%+v", err)
				return
			}
		} else {
			device = &api.DeviceResponse{
				Name: deviceName,
				ID:   deviceId,
			}
		}
		deviceId = device.ID

		var service = api.VirtualNetworkService{
			HttpOption: httpOption,
		}
		if vnId == "" {
			var resp []api.VirtualNetworkResponse
			var err error
			if resp, err = service.List(); err != nil {
				log.Errorf("%+v", err)
				return
			}
			if cap(resp) == 0 {
				log.Errorf("You do not have omniedge network")
				return
			}
			if cap(resp) == 1 {
				vnId = resp[0].ID
			} else {
				vnId, err = prompt(resp)
				if err != nil {
					log.Errorf("%+v", err)
					return
				}
				viper.Set(keyVirtualNetworks, resp)
			}
		}
		var joinOption = &api.JoinOption{
			VirtualNetworkId: vnId,
			DeviceId:         deviceId,
		}
		service = api.VirtualNetworkService{
			HttpOption: httpOption,
		}
		var joinResp *api.JoinVirtualNetworkResponse
		if joinResp, err = service.Join(joinOption); err != nil {
			log.Errorf("%+v", err)
			return
		}
		// Persist join response for reconnect
		viper.Set(keyJoinVirtualNetworkCommunityName, joinResp.CommunityName)
		viper.Set(keyJoinVirtualNetworkSecretKey, joinResp.SecretKey)
		viper.Set(keyJoinVirtualNetworkVirtualIP, joinResp.VirtualIP)
		viper.Set(keyJoinVirtualNetworkNetMask, joinResp.SubnetMask)
		viper.Set(keyJoinVirtualNetworkSuperNode, joinResp.Server.Host)
		viper.Set(keyJoinVirtualNetworkNetworkID, vnId)

		isExitNode := viper.GetBool(cliAsExitNode)
		viper.Set(keyJoinVirtualNetworkAsExitNode, isExitNode)

		// If acting as exit node, automatically enable routing
		enableRouting := viper.GetBool(cliEnableRouting)
		if isExitNode {
			enableRouting = true
			log.Info("Acting as exit node: automatically enabling routing")
		}

		// Sync exit node selection with backend if specified
		exitNodeIP := viper.GetString(cliExitNode)
		if exitNodeIP != "" {
			log.Infof("Selecting exit node: %s", exitNodeIP)
			// Find device ID for this IP in the network
			devs, err := service.GetDevices(vnId)
			if err == nil {
				var targetDeviceID string
				for _, d := range devs {
					if d.VirtualIP == exitNodeIP {
						targetDeviceID = d.ID
						break
					}
				}
				if targetDeviceID != "" {
					if err := service.SelectExitNode(vnId, deviceId, targetDeviceID); err != nil {
						log.Warnf("Failed to sync exit node selection to backend: %v", err)
					} else {
						log.Info("Successfully synced exit node selection to backend")
					}
				} else {
					log.Warnf("Could not find device with IP %s in network %s for backend sync", exitNodeIP, vnId)
				}
			}
		}

		viper.Set(keyDeviceUUID, deviceId)
		log.Infof("Success to join virtual network")
		log.Infof("Start to connect omniedge")

		// Set flags for startCmd and run it
		viper.Set(cliVirtualNetworkId, vnId)
		viper.Set(cliEnableRouting, enableRouting)
		viper.Set(cliExitNode, exitNodeIP)
		viper.Set(cliAsExitNode, isExitNode)

		startCmd.Run(cmd, args)
	},
}

func init() {
	var (
		networkId      string
		authConfigPath string
		enableRouting  bool
	)
	joinCmd.Flags().StringVarP(&networkId, cliVirtualNetworkId, "n", "", "id of the virtual network which you want to join")

	_ = registerCmd.MarkFlagRequired(cliVirtualNetworkId)
	joinCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	joinCmd.Flags().BoolVarP(&enableRouting, cliEnableRouting, "r", false, "enable routing (automatically enabled with --as-exit-node)")
	joinCmd.Flags().StringP(cliExitNode, "e", "", "exit node ip address")
	joinCmd.Flags().Bool(cliAsExitNode, false, "enable this device to act as an exit node (implies -r)")
	viper.BindPFlag(cliEnableRouting, joinCmd.Flags().Lookup(cliEnableRouting))
	viper.BindPFlag(cliExitNode, joinCmd.Flags().Lookup(cliExitNode))
	viper.BindPFlag(cliAsExitNode, joinCmd.Flags().Lookup(cliAsExitNode))
	rootCmd.AddCommand(joinCmd)
}
