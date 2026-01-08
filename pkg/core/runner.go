package core

import (
	"time"

	"github.com/google/uuid"
	omnin2n "github.com/omniedgeio/omniedge-cli/internal"
	"github.com/omniedgeio/omniedge-cli/pkg/api"
	log "github.com/sirupsen/logrus"
)

type StartOption struct {
	Hostname      string `json:"hostname"`
	CommunityName string `json:"community_name"`
	VirtualIP     string `json:"virtual_ip"`
	SecretKey     string `json:"secret_key"`
	DeviceMac     string `json:"device_mac"`
	DeviceMask    string `json:"device_mask"`
	SuperNode     string `json:"super_node"`
	EnableRouting bool   `json:"enable_routing"`
	Token         string `json:"token"`
	BaseUrl       string `json:"base_url"`
	HardwareUUID  string `json:"hardware_uuid"`
}

type StartService struct {
	StartOption
	edge *omnin2n.Edge
}

func (s *StartService) Start() error {
	s.edge = s.createEdge()
	id := uuid.New().String()
	s.edge.Id = id

	if err := s.edge.Configure(); err != nil {
		return err
	}

	if err := s.edge.OpenTunTapDevice(); err != nil {
		return err
	}

	// Start Heartbeat Goroutine
	if s.Token != "" && s.BaseUrl != "" {
		go s.heartbeatLoop()
	}

	log.Info("Starting omniedge")
	log.Infof("Listening address: %s", s.edge.DeviceIP)
	if err := s.edge.Start(); err != nil {
		log.Errorf("fail to start omniedge, error info:\n %s", err.Error())
		return err
	}
	return nil
}

func (s *StartService) Stop() {
	if s.edge != nil {
		s.edge.Stop()
		log.Info("Omniedge stopped")
	}
}

func (s *StartService) heartbeatLoop() {
	heartbeatService := api.HeartbeatService{
		HttpOption: api.HttpOption{
			Token:   s.Token,
			BaseUrl: s.BaseUrl,
		},
	}
	opt := &api.HeartbeatOption{
		HardwareUUID: s.HardwareUUID,
	}

	// Initial heartbeat
	if err := heartbeatService.Heartbeat(opt); err != nil {
		log.Warnf("Initial heartbeat failed: %v", err)
	}

	ticker := time.NewTicker(1 * time.Minute)
	defer ticker.Stop()

	for range ticker.C {
		if err := heartbeatService.Heartbeat(opt); err != nil {
			log.Warnf("Heartbeat failed: %v", err)
		} else {
			log.Debug("Heartbeat sent successfully")
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

	// Use random ports to avoid "address already in use"
	edge.LocalPort = GetRandomPort()
	edge.ManagementPort = GetRandomPort()

	edge.MTU = 1500
	return edge
}
