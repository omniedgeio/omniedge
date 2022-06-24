package cmd

import (
	edgecli "github.com/omniedgeio/omniedge-cli"
	log "github.com/sirupsen/logrus"
	"github.com/spf13/cobra"
	"github.com/spf13/viper"
)

var scanCmd = &cobra.Command{
	Use:     "scan",
	Aliases: []string{},
	Short:   "Scan local subnet",
	Example: "scan -c 192.168.32.0/24 -t 20",
	Run: func(cmd *cobra.Command, args []string) {
		bindFlags(cmd)
		edgecli.LoadClientConfig()
		var err error
		var timeout = viper.GetInt64(cliScanTimeout)
		var cidr = viper.GetString(cliCidr)
		var scanOption = edgecli.ScanOption{
			Cidr:    cidr,
			Timeout: timeout,
		}
		var deviceNet *edgecli.DeviceNet
		deviceNet, err = edgecli.GetCurrentDeviceNetStatus(cidr)
		if err != nil {
			log.Errorf("%+v", err)
		}
		var service = edgecli.ScanService{
			ScanOption: scanOption,
		}
		var scanResult *[]edgecli.ScanResult
		log.Printf("%+v", scanOption)

		if scanResult, err = service.Scan(&scanOption); err != nil {
			log.Errorf("%+v", err)
		}
		if deviceNet != nil {
			viper.Set(keyScanIP, deviceNet.IP)
			viper.Set(keyScanMacAddress, deviceNet.MacAddress)
			viper.Set(keyScanSubnetMask, deviceNet.SubnetMask)
			viper.Set(keyScanResult, scanResult)
			log.Infof("scan result %+v", scanResult)
			persistScanResult()
			log.Infof("Success to scan subnet")
		}
	},
}

func init() {
	var (
		timeout    int64
		cidr       string
		scanResult string
	)
	scanCmd.Flags().StringVarP(&scanResult, cliScanResult, "s", "", "path of scan result")
	scanCmd.Flags().StringVarP(&cidr, cliCidr, "c", "", "cidr of subnet")
	scanCmd.Flags().Int64VarP(&timeout, cliScanTimeout, "t", 120, "timeout of scan")
	rootCmd.AddCommand(scanCmd)
}
