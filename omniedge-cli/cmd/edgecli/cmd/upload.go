package cmd

import (
	"fmt"

	"github.com/mitchellh/mapstructure"
	api "github.com/omniedgeio/omniedge-cli/pkg/api"
	core "github.com/omniedgeio/omniedge-cli/pkg/core"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var uploadCmd = &cobra.Command{
	Use:     "upload",
	Aliases: []string{},
	Short:   "Upload local subnet to omniedge",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		core.LoadClientConfig()

		if err := loadAuthFile(); err != nil {
			log.Errorf("%+v", err)
			log.Error("Please try login first")
			return
		}
		endpointUrl := core.ConfigV.GetString(RestEndpointUrl)
		httpOption := api.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl: endpointUrl,
		}

		deviceID := viper.GetString(keyDeviceUUID)
		if deviceID == "" {
			log.Errorf("Please run join first")
			return
		}

		if err := loadScanResult(); err != nil {
			log.Errorf("%+v", err)
			return
		}

		scanResultArray := viper.Get(keyScanResult)
		if scanResultArray == nil {
			log.Errorf("No subnets in scan result, please scan first")
		}

		var scanResults []*api.ScanResult
		for _, v := range scanResultArray.([]interface{}) {
			data := &api.ScanResult{}
			if err := mapstructure.Decode(v, data); err != nil {
				log.Errorf("%+v\nFail to decode scan result, please contact omniedge team", err)
			}
			scanResults = append(scanResults, data)
		}

		var uploadService = &api.VirtualNetworkService{
			HttpOption: httpOption,
		}
		uploadOption := &api.UploadOption{
			IP:          viper.GetString(keyScanIP),
			MacAddress:  viper.GetString(keyScanMacAddress),
			SubnetMask:  viper.GetString(keyScanSubnetMask),
			DeviceId:    deviceID,
			ScanResults: scanResults,
		}
		err := uploadService.Upload(uploadOption)
		if err == nil {
			log.Infof("Success to upload subnet")
		} else {
			log.Fatalf("%+v", err)
		}
	},
}

func init() {
	rootCmd.AddCommand(uploadCmd)
}
