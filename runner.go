package edgecli

import (
	"time"

	"github.com/google/uuid"
	omnin2n "github.com/omniedgeio/omniedge-cli/internal"
	log "github.com/sirupsen/logrus"
)

type StartOption struct {
	Hostname      string
	CommunityName string
	VirtualIP     string
	SecretKey     string
	DeviceMac     string
	DeviceMask    string
	SuperNode     string
	EnableRouting bool
	Token         string
	BaseUrl       string
	HardwareUUID  string
}

type StartService struct {
	StartOption
}

func (s *StartService) Start() error {
	edge := s.createEdge()
	id := uuid.New().String()
	edge.Id = id

	if err := edge.Configure(); err != nil {
		return err
	}

	if err := edge.OpenTunTapDevice(); err != nil {
		return err
	}

	// Start Heartbeat Goroutine
	if s.Token != "" && s.BaseUrl != "" {
		go s.heartbeatLoop()
	}

	log.Info("Starting omniedge")
	log.Infof("Listening address: %s", edge.DeviceIP)
	if err := edge.Start(); err != nil {
		log.Errorf("fail to start omniedge, error info:\n %s", err.Error())
	}
	return nil
}

func (s *StartService) heartbeatLoop() {
	heartbeatService := HeartbeatService{
		HttpOption: HttpOption{
			Token:   s.Token,
			BaseUrl: s.BaseUrl,
		},
	}
	opt := &HeartbeatOption{
		HardwareUUID: s.HardwareUUID,
	}

	// Initial heartbeat
	if err := heartbeatService.Heartbeat(opt); err != nil {
		log.Warnf("Initial heartbeat failed: %v", err)
	}

	ticker := time.NewTicker(1 * time.Minute)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			if err := heartbeatService.Heartbeat(opt); err != nil {
				log.Warnf("Heartbeat failed: %v", err)
			} else {
				log.Debug("Heartbeat sent successfully")
			}
		}
	}
}

func (s *StartService) createEdge() *omnin2n.Edge {
	edge := new(omnin2n.Edge)
	edge.AllowRouting = s.EnableRouting
	edge.CommunityName = s.CommunityName
	edge.SuperNodeNum = 0
	edge.RegisterInterval = 20
	edge.DeviceName = s.Hostname
	edge.DeviceIPMode = "static"
	edge.DeviceIP = s.VirtualIP
	edge.DeviceMask = s.DeviceMask
	edge.DisablePMTUDiscovery = true
	edge.EncryptKey = s.SecretKey
	edge.SuperNodeHostPort = s.SuperNode
	edge.TransopId = 2
	edge.DeviceMac = s.DeviceMac
	edge.MTU = 1500
	return edge
}
