package edgecli

import (
	"bytes"
	"encoding/base64"
	"errors"
	"github.com/denisbrodbeck/machineid"
	log "github.com/sirupsen/logrus"
	"net"
	"os"
	"os/user"
	"path/filepath"
	"runtime"
	"strings"
)

func LoadClientConfig() {
	data, _ := Asset("config/dev.yml")
	ConfigV.SetConfigType("yaml")
	if err := ConfigV.ReadConfig(bytes.NewReader(data)); err != nil {
		log.Fatalf("Fail to load client config, please cotact team omniedge")
	}
}

/**
parse user input of auth file path
*/
func HandleAuthFile(authFile string) (string, error) {
	// todo fix auth file handle
	upperAuthFile := strings.ToUpper(authFile)
	if strings.HasPrefix(authFile, "~") || strings.HasPrefix(upperAuthFile, "$HOME") {
		usr, err := user.Current()
		if err != nil {
			log.Errorf("Fail to get the current home directory, please input the ")
			return "", err
		}
		if strings.HasPrefix(authFile, "~") {
			return strings.Replace(authFile, "~", usr.HomeDir, 1), nil
		}
		if strings.HasPrefix(upperAuthFile, "$HOME") {
			res := usr.HomeDir + authFile[5:len(authFile)-1]
			return res, nil
		}
	}
	return authFile, nil
}

func GenerateInstanceId() string {
	if addr, err := getMacAddress(); err == nil {
		res := base64.StdEncoding.EncodeToString([]byte(addr))
		return res
	}
	//todo use other as instance id
	return ""
}

func HandleAuthFileStatus(authFile string) error {
	dir := filepath.Dir(authFile)
	if _, err := os.Stat(authFile); err != nil {
		if os.IsNotExist(err) {
			return os.MkdirAll(dir, os.ModePerm)
		} else {
			return err
		}
	}
	return nil
}

func getMacAddress() (string, error) {
	netInterfaces, err := net.Interfaces()
	if err != nil {
		return "", errors.New(FailGetMacAddress)
	}
	mac, err := "", errors.New(FailGetMacAddress)
	for i := 0; i < len(netInterfaces); i++ {
		if (netInterfaces[i].Flags&net.FlagUp) != 0 && (netInterfaces[i].Flags&net.FlagLoopback) == 0 {
			addrs, _ := netInterfaces[i].Addrs()
			for _, address := range addrs {
				ipNet, ok := address.(*net.IPNet)
				if ok && ipNet.IP.IsGlobalUnicast() {
					mac = netInterfaces[i].HardwareAddr.String()
					return mac, nil
				}
			}
		}
	}
	return mac, err
}

func RevealHardwareUUID() (string, error) {
	return machineid.ProtectedID("omniedge")
}

func RevealHostName() string {
	name, err := os.Hostname()
	if err != nil {
		return ""
	}
	return name

}

func RevealOS() string {
	return runtime.GOOS

}
