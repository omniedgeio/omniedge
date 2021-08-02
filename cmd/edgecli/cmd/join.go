package cmd

import (
	"fmt"
	"github.com/manifoldco/promptui"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	edgecli "gitlab.com/omniedge/omniedge-linux-saas-cli"
	"strings"
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
			Token:   fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl: endpointUrl,
		}
		var service = edgecli.VirtualNetworkService{
			JoinOption: joinOption,
		}
		if vnId == "" {
			var resp []edgecli.VirtualNetworkResponse
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
				vnId = resp[0].UUID
			} else {
				vnId, err = prompt(resp)
				if err != nil {
					log.Errorf("%+v", err)
					return
				}
				viper.Set(keyVirtualNetworks, resp)
			}
		}
		joinOption = edgecli.JoinOption{
			Token:            fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl:          endpointUrl,
			VirtualNetworkId: vnId,
			DeviceId:         deviceId,
		}
		service = edgecli.VirtualNetworkService{
			JoinOption: joinOption,
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

func prompt(networks []edgecli.VirtualNetworkResponse) (string, error) {
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
{{ "UUID:" | faint }}	{{ .UUID}}`,
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
	return networks[i].UUID, nil
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
