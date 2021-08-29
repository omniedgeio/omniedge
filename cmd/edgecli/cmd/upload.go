package cmd

import (
	"fmt"
	"github.com/mitchellh/mapstructure"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
	edgecli "gitlab.com/omniedge/omniedge-linux-saas-cli"
)

var uploadCmd = &cobra.Command{
	Use:     "upload",
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
		httpOption := edgecli.HttpOption{
			Token:   fmt.Sprintf("Bearer %s", viper.GetString(keyAuthResponseToken)),
			BaseUrl: endpointUrl,
		}

		deviceID := viper.GetString(KeyDeviceUUID)
		if err := loadScanResult(); err != nil {
			log.Errorf("%+v", err)
			return
		}

		scanResultArray := viper.Get(keyScanResult)
		if scanResultArray == nil {
			log.Errorf("No subnets in scan result, please scan first")
		}

		var scanResults []*edgecli.ScanResult
		for _, v := range scanResultArray.([]interface{}) {
			data := &edgecli.ScanResult{}
			if err := mapstructure.Decode(v, data); err != nil {
				log.Errorf("%+v\nFail to decode scan result, please contact omniedge team", err)
			}
			scanResults = append(scanResults, data)
		}

		var uploadService = &edgecli.VirtualNetworkService{
			HttpOption: httpOption,
		}
		uploadOption := &edgecli.UploadOption{
			DeviceId:    deviceID,
			ScanResults: scanResults,
		}
		uploadService.Upload(uploadOption)
		log.Infof("Success to upload subnet")
	},
}

func init() {
	rootCmd.AddCommand(uploadCmd)
}
