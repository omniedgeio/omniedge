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

var startCmd = &cobra.Command{
	Use:     "start",
	Aliases: []string{},
	Short:   "Start OmniEdge and connect to a network",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		core.LoadClientConfig()

		// 1. Check Auth and trigger login if needed
		if err := loadAuthFile(); err != nil {
			log.Info("Not logged in. Initiating login flow...")
			loginCmd.Run(cmd, args)
			if err := loadAuthFile(); err != nil {
				log.Fatalf("Login failed: %v", err)
			}
		}

		var vnId = viper.GetString(cliVirtualNetworkId)
		var deviceId = viper.GetString(keyDeviceUUID)
		var deviceName = viper.GetString(keyDeviceName)

		// 3. Refresh Token if needed
		refreshToken := viper.GetString(keyAuthResponseRefreshToken)
		endpointUrl := core.ConfigV.GetString(RestEndpointUrl)
		if refreshToken != "" {
			log.Debug("Attempting to refresh token...")
			authService := api.AuthService{
				HttpOption: api.HttpOption{BaseUrl: endpointUrl},
			}
			if authResp, err := authService.Refresh(&api.RefreshTokenOption{RefreshToken: refreshToken}); err == nil {
				viper.Set(keyAuthResponse, authResp)
				viper.Set(keyAuthResponseToken, authResp.Token)
				viper.Set(keyAuthResponseRefreshToken, authResp.RefreshToken)
				persistAuthFile()
			} else {
				log.Warnf("Token refresh failed: %v. Initiating fresh login...", err)
				loginCmd.Run(cmd, args)
				if err := loadAuthFile(); err != nil {
					log.Fatalf("Login failed: %v", err)
				}
			}
		}

		// 4. Register Device if needed
		getHttpOption := func() api.HttpOption {
			token := viper.GetString(keyAuthResponseToken)
			if !strings.HasPrefix(token, "Bearer ") && token != "" {
				token = "Bearer " + token
			}
			return api.HttpOption{
				Token:   token,
				BaseUrl: endpointUrl,
			}
		}

		var device *api.DeviceResponse
		var err error
		if deviceId == "" || deviceName == "" {
			device, err = register(getHttpOption())
			if err != nil && strings.Contains(err.Error(), "E_UNAUTHORIZED_ACCESS") {
				log.Warn("Session expired or unauthorized. Please login again.")
				loginCmd.Run(cmd, args)
				if err := loadAuthFile(); err != nil {
					log.Fatalf("Login failed: %v", err)
				}
				device, err = register(getHttpOption())
			}
			if err != nil {
				log.Fatalf("Failed to register device: %v", err)
			}
			deviceId = device.ID
			deviceName = device.Name
		} else {
			device = &api.DeviceResponse{ID: deviceId, Name: deviceName}
		}

		persistAuthFile()

		// 6. Select Network if not provided
		vnService := api.VirtualNetworkService{HttpOption: getHttpOption()}
		if vnId == "" {
			networks, err := vnService.List()
			if err != nil {
				log.Errorf("Failed to list networks: %v", err)
				return
			}
			if len(networks) == 0 {
				log.Error("No virtual networks found. Create one on the dashboard.")
				return
			}
			if len(networks) == 1 {
				vnId = networks[0].ID
			} else {
				// Interactive pick
				vnId, err = prompt(networks)
				if err != nil {
					log.Errorf("Prompt failed: %v", err)
					return
				}
			}
		}

		// 6. Join Network
		joinResp, err := vnService.Join(&api.JoinOption{
			VirtualNetworkId: vnId,
			DeviceId:         deviceId,
		})
		if err != nil {
			log.Errorf("Failed to join network: %v", err)
			return
		}

		// Persist state for reconnects
		viper.Set(keyJoinVirtualNetworkCommunityName, joinResp.CommunityName)
		viper.Set(keyJoinVirtualNetworkSecretKey, joinResp.SecretKey)
		viper.Set(keyJoinVirtualNetworkVirtualIP, joinResp.VirtualIP)
		viper.Set(keyJoinVirtualNetworkNetMask, joinResp.SubnetMask)
		viper.Set(keyJoinVirtualNetworkSuperNode, joinResp.Server.Host)
		viper.Set(keyJoinVirtualNetworkNetworkID, vnId)
		viper.Set(keyDeviceUUID, deviceId)
		viper.Set(keyDeviceName, deviceName)

		isExitNode := viper.GetBool(cliAsExitNode)
		enableRouting := viper.GetBool(cliEnableRouting)
		if isExitNode {
			enableRouting = true
		}

		persistAuthFile()

		// 7. Daemonize after all interactive prompts are done
		// Prepare arguments for the potential elevated daemon
		// to ensure it doesn't prompt for network selection again.
		daemonArgs := []string{"start", "-n", vnId}
		if enableRouting {
			daemonArgs = append(daemonArgs, "-r")
		}
		if exitNodeIP := viper.GetString(cliExitNode); exitNodeIP != "" {
			daemonArgs = append(daemonArgs, "-e", exitNodeIP)
		}
		if isExitNode {
			daemonArgs = append(daemonArgs, "--as-exit-node")
		}

		if err := core.Daemonize(daemonArgs...); err != nil {
			log.Fatalf("Failed to daemonize: %v", err)
		}

		// 8. Start Engine
		randomMac, _ := core.GenerateRandomMac()
		hardwareId, _ := core.RevealHardwareUUID()

		startOption := core.StartOption{
			Hostname:      deviceName,
			DeviceMac:     randomMac,
			CommunityName: joinResp.CommunityName,
			VirtualIP:     joinResp.VirtualIP,
			SecretKey:     joinResp.SecretKey,
			DeviceMask:    joinResp.SubnetMask,
			SuperNode:     joinResp.Server.Host,
			EnableRouting: enableRouting,
			Token:         getHttpOption().Token,
			BaseUrl:       endpointUrl,
			HardwareUUID:  hardwareId,
			ExitNodeIP:    viper.GetString(cliExitNode),
			IsExitNode:    isExitNode,
			NetworkID:     vnId,
		}

		service := core.StartService{StartOption: startOption}
		if err := service.Start(); err != nil {
			log.Fatalf("Engine failed to start: %v", err)
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
	return registerService.Register(registerOption)
}

func prompt(networks []api.VirtualNetworkResponse) (string, error) {
	templates := &promptui.SelectTemplates{
		Label:    "Choose the network",
		Active:   "\U0001F336 {{ .Name | cyan }}",
		Inactive: "  {{ .Name | cyan }}",
		Selected: "\U0001F336 {{ .Name | cyan }}",
		Details: `
--------- Virtual Network ----------
{{ "Name:" | faint }}	{{ .Name }}
{{ "Cidr:" | faint }}	{{ .IPRange}}
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
	fmt.Printf("You chose: %s\n", networks[i].Name)
	return networks[i].ID, nil
}

func init() {
	var (
		authConfigPath string
		enableRouting  bool
		exitNode       string
		networkId      string
		username       string
		password       string
	)
	startCmd.Flags().StringVarP(&networkId, cliVirtualNetworkId, "n", "", "network id to join")
	startCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "auth config file path")
	startCmd.Flags().BoolVarP(&enableRouting, cliEnableRouting, "r", false, "enable routing")
	startCmd.Flags().StringVarP(&exitNode, cliExitNode, "e", "", "exit node ip address")
	startCmd.Flags().Bool(cliAsExitNode, false, "act as an exit node")
	startCmd.Flags().StringVarP(&username, cliUsername, "u", "", "username of omniedge")
	startCmd.Flags().StringVarP(&password, cliPassword, "p", "", "password of omniedge")

	viper.BindPFlag(cliVirtualNetworkId, startCmd.Flags().Lookup(cliVirtualNetworkId))
	viper.BindPFlag(cliEnableRouting, startCmd.Flags().Lookup(cliEnableRouting))
	viper.BindPFlag(cliExitNode, startCmd.Flags().Lookup(cliExitNode))
	viper.BindPFlag(cliAsExitNode, startCmd.Flags().Lookup(cliAsExitNode))
	viper.BindPFlag(cliUsername, startCmd.Flags().Lookup(cliUsername))
	viper.BindPFlag(cliPassword, startCmd.Flags().Lookup(cliPassword))
	rootCmd.AddCommand(startCmd)
}
