package cmd

import (
	"fmt"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	edge "gitlab.com/omniedge/omniedge-linux-saas-cli"
)

var registerCmd = &cobra.Command{
	Use:     "register",
	Aliases: []string{},
	Short:   "",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		edge.LoadClientConfig()

		if err := loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			return
		}
		endpointUrl := edge.ConfigV.GetString(RestEndpointUrl)
		hardwareId, err := edge.RevealHardwareUUID()
		if err != nil {
			log.Errorf("%+v", err)
			return
		}
		httpOption := edge.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl: endpointUrl,
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
	rootCmd.AddCommand(registerCmd)
}
