package cmd

import (
	"fmt"
	"github.com/google/uuid"
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
		registerOption := edge.RegisterOption{
			Token:        fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl:      endpointUrl,
			Name:         edge.RevealHostName(),
			HardwareUUID: uuid.NewString(),
			OS:           edge.RevealOS(),
		}
		registerService := edge.RegisterService{
			RegisterOption: registerOption,
		}
		var device *edge.DeviceResponse
		var err error
		if device, err = registerService.Register(); err != nil {
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
		username       string
		password       string
		secretKey      string
		authConfigPath string
	)
	registerCmd.Flags().StringVarP(&username, cliInterface, "i", "", "the interface")
	//registerCmd.MarkFlagRequired(cliInterface)
	registerCmd.Flags().StringVarP(&password, cliPassword, "p", "", "password of omniedge ( not recommend text password here)")
	registerCmd.Flags().StringVarP(&secretKey, cliSecretKey, "s", "", "secret-key of omniedge")
	registerCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	rootCmd.AddCommand(registerCmd)
}
