package edgecli

import (
	"bytes"
	"crypto/rand"
	"encoding/base64"
	"encoding/hex"
	"errors"
	"fmt"
	"github.com/denisbrodbeck/machineid"
	"github.com/google/uuid"
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
	id, err := machineid.ID()
	if err != nil {
		return "", errors.New(fmt.Sprintf("Fail to generate hardware id, err is %+v", err))
	}
	id = strings.ToLower(strings.Replace(id, "-", "", -1))
	idBytes, err := hex.DecodeString(id)
	if err != nil {
		return "", errors.New(fmt.Sprintf("Fail to generate hardware id, err is %+v", err))
	}
	hardwareUUID, err := uuid.FromBytes(idBytes)
	if err != nil {
		return "", errors.New(fmt.Sprintf("Fail to generate hardware id, err is %+v", err))
	}
	return hardwareUUID.String(), nil
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

func GenerateRandomMac() (string, error) {
	buf := make([]byte, 6)
	_, err := rand.Read(buf)
	if err != nil {
		return "", errors.New(fmt.Sprintf("Fail to generate random buf, err: %+v", err))
	}
	// Set the local bit
	buf[0] |= 2
	return fmt.Sprintf("%02x:%02x:%02x:%02x:%02x:%02x", buf[0], buf[1], buf[2], buf[3], buf[4], buf[5]), nil
}
