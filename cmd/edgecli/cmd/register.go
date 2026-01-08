package cmd

import (
	"fmt"
	api "github.com/omniedgeio/omniedge-cli/pkg/api"
	core "github.com/omniedgeio/omniedge-cli/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var registerCmd = &cobra.Command{
	Use:     "register",
	Aliases: []string{},
	Short:   "",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		core.LoadClientConfig()

		if err := loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		endpointUrl := core.ConfigV.GetString(RestEndpointUrl)
		hardwareId, err := core.RevealHardwareUUID()
		if err != nil {
			log.Errorf("%+v", err)
			return
		}
		httpOption := api.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl: endpointUrl,
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
			log.Errorf("%+v", err)
			return
		}
		viper.Set(keyDevice, device)
		persistAuthFile()
		log.Infof("Successful to register the device")
		log.Infof("Current device detail is %+v", device)
	},
}

func init() {
	var (
		authConfigPath string
	)
	registerCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	//rootCmd.AddCommand(registerCmd)
}
