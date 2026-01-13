package cmd

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"

	api "github.com/omniedgeio/omniedge/pkg/api"
	core "github.com/omniedgeio/omniedge/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var Version string = "dev"

var rootCmd = &cobra.Command{
	Use:           "omniedge",
	Version:       Version,
	Short:         "",
	Long:          ``,
	SilenceErrors: true,
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		viper.SetEnvPrefix("omniedge")
		viper.SetEnvKeyReplacer(strings.NewReplacer("-", "_", ".", "_"))
		if viper.GetBool("debug") {
			log.SetLevel(log.DebugLevel)
		} else {
			log.SetLevel(log.InfoLevel)
		}
	},
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		log.Fatal("Fail to execute the command", err)
	}
}

func init() {
	rootCmd.PersistentFlags().Bool("debug", false, "Enable debug logging")
	viper.BindPFlag("debug", rootCmd.PersistentFlags().Lookup("debug"))
}

func bindFlags(cmd *cobra.Command) {
	if err := viper.BindPFlags(cmd.LocalFlags()); err != nil {
		log.Fatal(CouldNotBindFlags)
	}
}

func loadAuthFile() error {
	// 1. Try to load from Config File first
	var authFile = viper.GetString(cliAuthConfigFile)
	if authFile == "" {
		authFile = Option.AuthFileDefaultPath
	}
	handledAuthFile, err := core.HandleFilePrefix(authFile)
	var fileErr error
	if err != nil {
		fileErr = errors.New("fail to parse the path of the auth file")
	} else {
		log.Debugf("Loading auth from file: %s", handledAuthFile)
		viper.SetConfigFile(handledAuthFile)
		viper.SetConfigType("json")
		if err = viper.ReadInConfig(); err != nil {
			// Save error, but continue checking Keychain
			fileErr = fmt.Errorf("fail to read omniedge file, please login first. err is %w", err)
		}
	}

	// 2. Try loading from secure keychain and overlay (skip if root)
	if core.IsRoot() {
		log.Debug("Running as root, skipping keychain.")
	} else if secureData, err := core.LoadSecureToken(); err == nil && secureData != "" {
		var authResp api.AuthResp
		if err := json.Unmarshal([]byte(secureData), &authResp); err == nil {
			// Bridge legacy Token field if needed
			if authResp.Token == "" && authResp.AccessToken != "" {
				authResp.Token = authResp.AccessToken
			}

			// Overlay secure data into Viper
			// Only overwrite if the secure data actually has the field, allowing auth.json
			// to "backfill" missing fields (like refresh_token) if keychain is stale.
			if authResp.Token != "" {
				viper.Set(keyAuthResponse, authResp)
				viper.Set(keyAuthResponseToken, authResp.Token)
			}
			if authResp.RefreshToken != "" {
				viper.Set(keyAuthResponseRefreshToken, authResp.RefreshToken)
			}
			// If we laid down a valid token from keychain, we consider auth loaded successfully
			if authResp.Token != "" {
				return nil
			}
		}
	}

	// 3. If Keychain didn't provide a valid session, return the file error
	return fileErr
}

func persistAuthFile() {
	var authFile = viper.GetString(cliAuthConfigFile)
	if authFile == "" {
		authFile = Option.AuthFileDefaultPath
	}
	handledAuthFile, err := core.HandleFilePrefix(authFile)
	if err != nil {
		log.Fatalf("Fail to parse the path of the auth file")
	}
	if err = core.HandleFileStatus(handledAuthFile); err != nil {
		log.Fatalf("Fail to create omniedge file, err is %s", err.Error())
	}

	// Sanitize: do not persist sensitive input flags or transient network keys
	viper.Set(cliSecretKey, "")
	viper.Set(keyJoinVirtualNetwork, nil)

	if err := viper.WriteConfigAs(handledAuthFile); err != nil {
		log.Fatalf("Fail to write config into file, err is %s", err.Error())
	}
	// Secure the file permissions: 0600 (read/write for owner only)
	if err := os.Chmod(handledAuthFile, 0600); err != nil {
		log.Warnf("Failed to set restrictive permissions on auth file: %v", err)
	}
}

func loadScanResult() error {
	var scanResult = viper.GetString(cliScanResult)
	if scanResult == "" {
		scanResult = Option.ScanResultDefaultPath
	}
	handledScanResultFile, err := core.HandleFilePrefix(scanResult)
	if err != nil {
		return errors.New("fail to parse the path of the auth file")
	}
	viper.SetConfigFile(handledScanResultFile)
	viper.SetConfigType("json")
	if err = viper.ReadInConfig(); err != nil {
		return fmt.Errorf("fail to read omniedge scan result, please scan first")
	}
	return nil
}

func persistScanResult() {
	var scanResult = viper.GetString(cliScanResult)
	if scanResult == "" {
		scanResult = Option.ScanResultDefaultPath
	}
	handledScanResultFile, err := core.HandleFilePrefix(scanResult)
	if err != nil {
		log.Fatalf("Fail to parse the path of the scan result")
	}
	log.Infof("result %+v", handledScanResultFile)
	if err = core.HandleFileStatus(handledScanResultFile); err != nil {
		log.Fatalf("Fail to create scan result, err is %s", err.Error())
	}
	if err := viper.WriteConfigAs(handledScanResultFile); err != nil {
		log.Fatalf("Fail to write config into file, err is %s", err.Error())
	}
}
