package cmd

//
//import (
//	log "github.com/sirupsen/logrus"
//	"github.com/spf13/cobra"
//	"github.com/spf13/viper"
//	edgecli "gitlab.com/omniedge/omniedge-linux-saas-cli"
//	omnin2n "gitlab.com/omniedge/omniedge-linux-saas-cli/internal"
//	"os"
//)
//
//var joinCmd = &cobra.Command{
//	Use:     "join",
//	Aliases: []string{},
//	Short:   "",
//	Run: func(cmd *cobra.Command, args []string) {
//		bindFlags(cmd)
//		edgecli.LoadClientConfig()
//		instanceId := edgecli.GenerateInstanceId()
//		var authFile = viper.GetString(cliAuthConfigFile)
//		if authFile == "" {
//			authFile = authFileDefault
//		}
//		handledAuthFile, err := edgecli.HandleAuthFile(authFile)
//		if err != nil {
//			log.Fatalln("Fail to parse the path of the auth file")
//			return
//		}
//		viper.SetConfigFile(handledAuthFile)
//		viper.SetConfigType("json")
//		if err = viper.ReadInConfig(); err != nil {
//			log.Fatalf("Fail to read omniedge file, please login first. err is %s", err.Error())
//			return
//		}
//		virtualNetworkIds := viper.GetStringSlice(keyVirtualNetworkIds)
//		hostname := viper.GetString(keyHostname)
//		if virtualNetworkIds == nil || len(virtualNetworkIds) == 0 {
//			log.Fatalf("Fail to get VirtualNetwork. Please run login first\n")
//		}
//		if hostname == "" {
//			hostname, _ = os.Hostname()
//		}
//		joinReq := &backend.JoinRequest{
//			InstanceID:       instanceId,
//			VirtualNetworkID: virtualNetworkIds[0],
//			Name:             hostname,
//			UserAgent:        "Linux",
//			Description:      "",
//			PublicKey:        "",
//		}
//		joinResp, errCode := backend.JoinOmniEdge(joinReq, viper.GetString(keyAuthResponseIdToken))
//		if errCode == 401 {
//			log.Info("Try to refresh the token...")
//			option := edgecli.NewOption()
//			handle := &edgecli.CognitoHandle{
//				Option: option,
//				Response: &edgecli.AuthResponse{
//					RefreshToken: viper.GetString(keyAuthResponseRefreshToken),
//				},
//			}
//			if r := handle.RefreshToken(); r != nil {
//				joinResp, errCode = backend.JoinOmniEdge(joinReq, r.IdToken)
//				if errCode == 401 {
//					log.Fatalf("Not authorized or the refresh token was expired. Please login")
//					return
//				}
//				if errCode != 200 {
//					log.Fatalf("Backend status code is not 200. Real status code is %d", errCode)
//					return
//				}
//				r.RefreshToken = viper.GetString(keyAuthResponseRefreshToken)
//				viper.Set(keyAuthResponse, r)
//				persistAuthFile()
//			}
//		}
//		if errCode != 200 {
//			log.Fatalf("Backend status code is not 200. Real status code is %d", errCode)
//			return
//		}
//		edge := createEdge(joinResp, hostname)
//		if edge != nil {
//			if err := edgecli.Create(edge); err != nil {
//				log.Fatalf("Fail to open tun/tap device.")
//			}
//		}
//	},
//}
//
//func createEdge(joinResp *backend.JoinResponse, hostname string) *omnin2n.Edge {
//	edge := new(omnin2n.Edge)
//	edge.CommunityName = joinResp.CommunityName
//	edge.SuperNodeNum = 0
//	edge.RegisterInterval = 20
//	edge.DeviceName = hostname
//	edge.DeviceIPMode = "static"
//	edge.DeviceIP = joinResp.VirtualIP
//	edge.DeviceMask = "255.255.255.0"
//	edge.DisablePMTUDiscovery = true
//	edge.EncryptKey = joinResp.SecretKey
//	edge.SuperNodeHostPort = joinResp.Addr
//	edge.TransopId = 2
//	edge.DeviceMac = joinResp.InstanceID
//	edge.MTU = 1500
//	return edge
//}
//
//func init() {
//	var (
//		authConfigPath string
//	)
//	joinCmd.Flags().StringVarP(&authConfigPath, cliAuthConfigFile, "f", "", "position to store the auth and config")
//	rootCmd.AddCommand(joinCmd)
//}
