package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"time"

	api "github.com/omniedgeio/omniedge/pkg/api"
	core "github.com/omniedgeio/omniedge/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"golang.org/x/crypto/ssh/terminal"
)

var loginCmd = &cobra.Command{
	Use:     "login",
	Aliases: []string{},
	Short:   "Login Omniedge network",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		core.LoadClientConfig()
		var username = viper.GetString(cliUsername)
		var password string
		var secretKey string
		password = viper.GetString(cliPassword)
		secretKey = viper.GetString(cliSecretKey)
		endpointUrl := core.ConfigV.GetString(RestEndpointUrl)

		httpOption := api.HttpOption{
			BaseUrl: endpointUrl,
		}
		authService := api.AuthService{
			HttpOption: httpOption,
		}

		var authResp *api.AuthResp
		var err error

		if username != "" {
			if password == "" {
				fmt.Print("Enter Password:")
				bytePassword, err := terminal.ReadPassword(0)
				if err != nil {
					log.Panic(err)
				}
				password = string(bytePassword)
				fmt.Println()
			}
			authOption := &api.AuthOption{
				Username:   username,
				Password:   password,
				AuthMethod: api.LoginByPassword,
			}
			authResp, err = authService.Login(authOption)
		} else if secretKey != "" || os.Getenv(omniedgeSecretKey) != "" {
			if secretKey == "" {
				secretKey = os.Getenv(omniedgeSecretKey)
			}
			authOption := &api.AuthOption{
				SecretKey:  secretKey,
				AuthMethod: api.LoginBySecretKey,
			}
			authResp, err = authService.Login(authOption)
		} else {
			// OAuth 2.0 Device Flow
			fmt.Println("Initiating browser-based login...")
			clientId := "omniedge-cli" // Default CLI client ID
			deviceResp, err := authService.DeviceFlowInit(clientId, "openid profile email offline_access")
			if err != nil {
				log.Errorf("Failed to initiate login: %v", err)
				return
			}

			fmt.Printf("\nPlease visit: %s\n", deviceResp.VerificationUri)
			fmt.Printf("And enter the code: %s\n\n", deviceResp.UserCode)

			// Try to open browser automatically
			// open.Run(deviceResp.VerificationUriComplete)

			fmt.Println("Waiting for authorization...")

			interval := deviceResp.Interval
			if interval <= 0 {
				interval = 5
			}

			for {
				authResp, err = authService.DeviceFlowToken(clientId, deviceResp.DeviceCode)
				if err == nil {
					break
				}

				errMsg := err.Error()
				if strings.Contains(errMsg, "authorization_pending") {
					// Keep polling
				} else if strings.Contains(errMsg, "slow_down") {
					interval += 5
				} else {
					log.Errorf("Login failed: %v", err)
					return
				}

				time.Sleep(time.Duration(interval) * time.Second)
			}
		}
		if err != nil {
			log.Errorf("%+v", err)
			return
		}

		// Bridge for legacy code if needed (redundant now but safe)
		if authResp.Token == "" && authResp.AccessToken != "" {
			authResp.Token = authResp.AccessToken
		}

		viper.Set(cliSecretKey, "")
		viper.Set(keyAuthResponse, authResp)
		viper.Set(keyAuthResponseToken, authResp.Token)
		viper.Set(keyAuthResponseRefreshToken, authResp.RefreshToken)
		persistAuthFile()

		// Securely save to keychain
		authJSON, _ := json.Marshal(authResp)
		_ = core.SaveSecureToken(string(authJSON))

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
