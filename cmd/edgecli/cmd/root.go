package cmd

import (
	"errors"
	"fmt"
	edge "github.com/omniedgeio/omniedge-cli"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	"strings"
)

var rootCmd = &cobra.Command{
	Use:           "omniedge",
	Short:         "",
	Long:          ``,
	SilenceErrors: true,
	PersistentPreRun: func(cmd *cobra.Command, args []string) {
		viper.SetEnvPrefix("omniedge")
		viper.SetEnvKeyReplacer(strings.NewReplacer("-", "_", ".", "_"))
	},
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		log.Fatal("Fail to execute the command", err)
	}
}

func bindFlags(cmd *cobra.Command) {
	if err := viper.BindPFlags(cmd.LocalFlags()); err != nil {
		log.Fatal(CouldNotBindFlags)
	}
}

func loadAuthFile() error {
	var authFile = viper.GetString(cliAuthConfigFile)
	if authFile == "" {
		authFile = Option.AuthFileDefaultPath
	}
	handledAuthFile, err := edge.HandleFilePrefix(authFile)
	if err != nil {
		return errors.New("fail to parse the path of the auth file")
	}
	viper.SetConfigFile(handledAuthFile)
	viper.SetConfigType("json")
	if err = viper.ReadInConfig(); err != nil {
		return errors.New(fmt.Sprintf("fail to read omniedge file, please login first. err is %s", err.Error()))
	}
	return nil
}

func persistAuthFile() {
	var authFile = viper.GetString(cliAuthConfigFile)
	if authFile == "" {
		authFile = Option.AuthFileDefaultPath
	}
	handledAuthFile, err := edge.HandleFilePrefix(authFile)
	if err != nil {
		log.Fatalf("Fail to parse the path of the auth file")
	}
	if err = edge.HandleFileStatus(handledAuthFile); err != nil {
		log.Fatalf("Fail to create omniedge file, err is %s", err.Error())
	}
	if err := viper.WriteConfigAs(handledAuthFile); err != nil {
		log.Fatalf("Fail to write config into file, err is %s", err.Error())
	}
}

func loadScanResult() error {
	var scanResult = viper.GetString(cliScanResult)
	if scanResult == "" {
		scanResult = Option.ScanResultDefaultPath
	}
	handledScanResultFile, err := edge.HandleFilePrefix(scanResult)
	if err != nil {
		return errors.New("fail to parse the path of the auth file")
	}
	viper.SetConfigFile(handledScanResultFile)
	viper.SetConfigType("json")
	if err = viper.ReadInConfig(); err != nil {
		return errors.New(fmt.Sprintf("fail to read omniedge scan result, please scan first."))
	}
	return nil
}

func persistScanResult() {
	var scanResult = viper.GetString(cliScanResult)
	if scanResult == "" {
		scanResult = Option.ScanResultDefaultPath
	}
	handledScanResultFile, err := edge.HandleFilePrefix(scanResult)
	if err != nil {
		log.Fatalf("Fail to parse the path of the scan result")
	}
	log.Infof("result %+v", handledScanResultFile)
	if err = edge.HandleFileStatus(handledScanResultFile); err != nil {
		log.Fatalf("Fail to create scan result, err is %s", err.Error())
	}
	if err := viper.WriteConfigAs(handledScanResultFile); err != nil {
		log.Fatalf("Fail to write config into file, err is %s", err.Error())
	}
}
