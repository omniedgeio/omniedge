package cmd

import (
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	edge "gitlab.com/omniedge/omniedge-linux-saas-cli"
	"os"
	"strings"
)

var loginCmd = &cobra.Command{
	Use:     "login",
	Aliases: []string{},
	Short:   "",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		edge.LoadClientConfig()
		var username = viper.GetString(cliUsername)
		//var password string
		var secretKey string
		//password = viper.GetString(cliPassword)
		secretKey = viper.GetString(cliSecretKey)
		endpointUrl := edge.ConfigV.GetString(RestEndpointUrl)
		// login by username
		if username != "" {
			log.Errorf("Not support login by username and password, please use login by secret key.")
		} else {
			if secretKey == "" {
				for _, e := range os.Environ() {
					pair := strings.SplitN(e, "=", 2)
					if omniedgeSecretKey == pair[0] {
						secretKey = pair[1]
					}
				}
			}
			if secretKey == "" {
				log.Errorf("Please input secret key or set system variable %s", omniedgeSecretKey)
				return
			}
			authOption := edge.AuthOption{
				BaseUrl:    endpointUrl,
				SecretKey:  secretKey,
				AuthMethod: edge.LoginBySecretKey,
			}
			authService := edge.AuthService{
				AuthOption: authOption,
			}
			authService.Login()
		}
	},
}

func init() {
	var (
		username       string
		password       string
		secretKey      string
		authConfigPath string
	)
	loginCmd.Flags().StringVarP(&username, cliUsername, "u", "", "username of omniedge")
	loginCmd.Flags().StringVarP(&password, cliPassword, "p", "", "password of omniedge ( not recommend text password here)")
	loginCmd.Flags().StringVarP(&secretKey, cliSecretKey, "s", "", "secret-key of omniedge")
	loginCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
	rootCmd.AddCommand(loginCmd)
}
