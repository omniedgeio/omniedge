package cmd

import (
	"fmt"
	"strings"

	"github.com/manifoldco/promptui"
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
				viper.Set(keyAuthResponseRefreshToken, authResp.RefreshToken)
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
		viper.Set(keyJoinVirtualNetworkAsExitNode, viper.GetBool(cliAsExitNode))

		viper.Set(keyDeviceUUID, deviceId)
		persistAuthFile()
		log.Infof("Success to join virtual network")
		log.Infof("Start to connect omniedge")
		if err = start(device, joinResp, viper.GetBool(cliEnableRouting), viper.GetString(cliExitNode), vnId, viper.GetBool(cliAsExitNode)); err != nil {
			log.Errorf("%+v", err)
			return
		}
	},
}

func register(httpOption api.HttpOption) (*api.DeviceResponse, error) {
	hardwareId, err := core.RevealHardwareUUID()
	if err != nil {
		return nil, err
	}
	registerOption := &api.RegisterOption{
		Name:         core.RevealHostName(),
		HardwareUUID: hardwareId,
		OS:           core.RevealOS(),
	}
	registerService := api.RegisterService{
		HttpOption: httpOption,
	}
	var device *api.DeviceResponse
	if device, err = registerService.Register(registerOption); err != nil {
		return nil, err
	}
	return device, err
}

func start(device *api.DeviceResponse, joinResponse *api.JoinVirtualNetworkResponse, enableRouting bool, exitNodeIP string, networkId string, isExitNode bool) error {
	var randomMac string
	var err error
	if randomMac, err = core.GenerateRandomMac(); err != nil {
		return err
	}

	// Get actual hardware UUID for heartbeat
	hardwareId, _ := core.RevealHardwareUUID()

	var startOption = core.StartOption{
		Hostname:      device.Name,
		DeviceMac:     randomMac,
		CommunityName: joinResponse.CommunityName,
		VirtualIP:     joinResponse.VirtualIP,
		SecretKey:     joinResponse.SecretKey,
		DeviceMask:    joinResponse.SubnetMask,
		SuperNode:     joinResponse.Server.Host,
		EnableRouting: enableRouting,
		Token:         fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
		BaseUrl:       core.ConfigV.GetString(RestEndpointUrl),
		HardwareUUID:  hardwareId,
		ExitNodeIP:    exitNodeIP,
		IsExitNode:    isExitNode,
		NetworkID:     networkId,
	}
	var service = core.StartService{
		StartOption: startOption,
	}
	if err := service.Start(); err != nil {
		return err
	}
	return nil
}

func prompt(networks []api.VirtualNetworkResponse) (string, error) {
	templates := &promptui.SelectTemplates{
		Label:    "choose the network",
		Active:   "\U0001F336 {{ .Name | cyan }}",
		Inactive: "  {{ .Name | cyan }}",
		Selected: "\U0001F336 {{ .Name | red | cyan }}",
		Details: `
--------- Virtual Network ----------
{{ "Name:" | faint }}	{{ .Name }}
{{ "Cidr:" | faint }}	{{ .IPRange}}
{{ "Role:" | faint }}	{{ .Role}}
{{ "ID:" | faint }}	{{ .ID}}`,
	}

	searcher := func(input string, index int) bool {
		network := networks[index]
		name := strings.Replace(strings.ToLower(network.Name), " ", "", -1)
		input = strings.Replace(strings.ToLower(input), " ", "", -1)

		return strings.Contains(name, input)
	}

	prompt := promptui.Select{
		Label:     "Choose Virtual Network",
		Items:     networks,
		Templates: templates,
		Size:      6,
		Searcher:  searcher,
	}

	i, _, err := prompt.Run()

	if err != nil {
		return "", err
	}
	fmt.Printf("You choose number %d: %s\n", i+1, networks[i].Name)
	return networks[i].ID, nil
}

func init() {
	var (
		networkId      string
		authConfigPath string
	)
	joinCmd.Flags().StringVarP(&networkId, cliVirtualNetworkId, "n", "", "id of the virtual network which you want to join")

	_ = registerCmd.MarkFlagRequired(cliVirtualNetworkId)
	joinCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	joinCmd.Flags().BoolP(cliEnableRouting, "r", false, "enable routing")
	joinCmd.Flags().StringP(cliExitNode, "e", "", "exit node ip address")
	joinCmd.Flags().Bool(cliAsExitNode, false, "enable this device as an exit node")
	viper.BindPFlag(cliEnableRouting, joinCmd.Flags().Lookup(cliEnableRouting))
	viper.BindPFlag(cliExitNode, joinCmd.Flags().Lookup(cliExitNode))
	viper.BindPFlag(cliAsExitNode, joinCmd.Flags().Lookup(cliAsExitNode))
	rootCmd.AddCommand(joinCmd)
}
