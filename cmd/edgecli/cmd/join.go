package cmd

import (
	"fmt"
	"github.com/manifoldco/promptui"
	edge "github.com/omniedgeio/omniedge-cli"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"strings"
)

var joinCmd = &cobra.Command{
	Use:     "join",
	Aliases: []string{},
	Short:   "Join omniedge network",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		edge.LoadClientConfig()
		if err := loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		endpointUrl := edge.ConfigV.GetString(RestEndpointUrl)
		var vnId = viper.GetString(cliVirtualNetworkId)
		var deviceId = viper.GetString(keyDeviceUUID)
		var deviceName = viper.GetString(keyDeviceName)

		var device *edge.DeviceResponse
		var err error

		refreshToken := viper.GetString(keyAuthResponseRefreshToken)
		if refreshToken != "" {
			refreshTokenOption := &edge.RefreshTokenOption{
				RefreshToken: refreshToken,
			}
			var refreshTokenHttpOption = edge.HttpOption{
				BaseUrl: endpointUrl,
			}
			authService := edge.AuthService{
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

		var httpOption = edge.HttpOption{
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
			device = &edge.DeviceResponse{
				Name: deviceName,
				ID:   deviceId,
			}
		}
		deviceId = device.ID

		var service = edge.VirtualNetworkService{
			HttpOption: httpOption,
		}
		if vnId == "" {
			var resp []edge.VirtualNetworkResponse
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
		var joinOption = &edge.JoinOption{
			VirtualNetworkId: vnId,
			DeviceId:         deviceId,
		}
		service = edge.VirtualNetworkService{
			HttpOption: httpOption,
		}
		var joinResp *edge.JoinVirtualNetworkResponse
		if joinResp, err = service.Join(joinOption); err != nil {
			log.Errorf("%+v", err)
			return
		}
		// not persist cliVirtualNetworkId
		viper.Set(cliVirtualNetworkId, "")
		// not to persist join response (for security issue)
		//viper.Set(keyJoinVirtualNetwork, joinResp)
		viper.Set(keyDeviceUUID, deviceId)
		persistAuthFile()
		log.Infof("Success to join virtual network")
		log.Infof("Start to connect omniedge")
		if err = start(device, joinResp); err != nil {
			log.Errorf("%+v", err)
			return
		}
	},
}

func register(httpOption edge.HttpOption) (*edge.DeviceResponse, error) {
	hardwareId, err := edge.RevealHardwareUUID()
	if err != nil {
		return nil, err
	}
	registerOption := &edge.RegisterOption{
		Name:         edge.RevealHostName(),
		HardwareUUID: hardwareId,
		OS:           edge.RevealOS(),
	}
	registerService := edge.RegisterService{
		HttpOption: httpOption,
	}
	var device *edge.DeviceResponse
	if device, err = registerService.Register(registerOption); err != nil {
		return nil, err
	}
	return device, err
}

func start(device *edge.DeviceResponse, joinResponse *edge.JoinVirtualNetworkResponse) error {
	var randomMac string
	var err error
	if randomMac, err = edge.GenerateRandomMac(); err != nil {
		return err
	}
	var startOption = edge.StartOption{
		Hostname:      device.Name,
		DeviceMac:     randomMac,
		CommunityName: joinResponse.CommunityName,
		VirtualIP:     joinResponse.VirtualIP,
		SecretKey:     joinResponse.SecretKey,
		DeviceMask:    joinResponse.SubnetMask,
		SuperNode:     joinResponse.Server.Host,
	}
	var service = edge.StartService{
		StartOption: startOption,
	}
	if err := service.Start(); err != nil {
		return err
	}
	return nil
}

func prompt(networks []edge.VirtualNetworkResponse) (string, error) {
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
	rootCmd.AddCommand(joinCmd)
}
