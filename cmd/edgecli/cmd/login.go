package cmd

import (
	"fmt"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	edge "gitlab.com/omniedge/omniedge-linux-saas-cli"
	"golang.org/x/crypto/ssh/terminal"
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
		var password string
		var secretKey string
		password = viper.GetString(cliPassword)
		secretKey = viper.GetString(cliSecretKey)
		endpointUrl := edge.ConfigV.GetString(RestEndpointUrl)
		// login by username
		var authResp *edge.AuthResp
		var err error
		if username != "" {
			if password = viper.GetString(cliPassword); password == "" {
				fmt.Print("Enter Password:")
				bytePassword, err := terminal.ReadPassword(0)
				if err != nil {
					log.Panic(err)
				}
				password = string(bytePassword)
				fmt.Println()
			}
			httpOption := edge.HttpOption{
				BaseUrl: endpointUrl,
			}
			authOption := &edge.AuthOption{
				Username:   username,
				Password:   password,
				AuthMethod: edge.LoginByPassword,
			}
			authService := edge.AuthService{
				HttpOption: httpOption,
			}
			authResp, err = authService.Login(authOption)
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
			httpOption := edge.HttpOption{
				BaseUrl: endpointUrl,
			}
			authOption := &edge.AuthOption{
				SecretKey:  secretKey,
				AuthMethod: edge.LoginBySecretKey,
			}
			authService := edge.AuthService{
				HttpOption: httpOption,
			}
			authResp, err = authService.Login(authOption)
		}
		if err != nil {
			log.Errorf("%+v", err)
			return
		}
		viper.Set(cliSecretKey, "")
		viper.Set(keyAuthResponse, authResp)
		persistAuthFile()
		log.Infof("successful to login")
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
