package core

import (
	"net"
	"time"

	"github.com/google/uuid"
	omnin2n "github.com/omniedgeio/omniedge/internal"
	"github.com/omniedgeio/omniedge/pkg/api"
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
	ExitNodeIP    string `json:"exit_node_ip"`
	IsExitNode    bool   `json:"is_exit_node"`
	NetworkID     string `json:"network_id"`
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

	if s.ExitNodeIP != "" {
		log.Infof("Setting up exit node: %s", s.ExitNodeIP)
		if err := SetupExitNode(s.ExitNodeIP, s.SuperNode); err != nil {
			log.Errorf("Fail to setup exit node: %v", err)
		}
	}

	log.Info("Starting omniedge")
	log.Infof("Listening address: %s", s.edge.DeviceIP)
	if err := s.edge.Start(); err != nil {
		log.Errorf("fail to start omniedge, error info:\n %s", err.Error())
		RestoreExitNode()
		return err
	}
	return nil
}

func (s *StartService) SetExitNode(exitNodeIP string) error {
	if exitNodeIP == "" {
		log.Info("Deselecting exit node, restoring routes")
		return RestoreExitNode()
	}
	log.Infof("Selecting exit node: %s", exitNodeIP)
	return SetupExitNode(exitNodeIP, s.SuperNode)
}

func (s *StartService) Stop() {
	RestoreExitNode()
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
		IsExitNode:   s.IsExitNode,
	}

	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	// Initial heartbeat
	if resp, err := heartbeatService.Heartbeat(opt); err != nil {
		log.Errorf("Initial heartbeat failed: %v", err)
	} else {
		s.handleHeartbeatResponse(resp)
	}

	for range ticker.C {
		if resp, err := heartbeatService.Heartbeat(opt); err != nil {
			log.Warnf("Heartbeat failed: %v", err)
		} else {
			s.handleHeartbeatResponse(resp)
		}
	}
}

func (s *StartService) handleHeartbeatResponse(resp *api.HeartbeatResponse) {
	if s.NetworkID == "" || resp.ExitNodes == nil {
		return
	}

	newExitNodeIP := resp.ExitNodes[s.NetworkID]
	if newExitNodeIP != s.ExitNodeIP {
		if newExitNodeIP != "" {
			ip := net.ParseIP(newExitNodeIP)
			myIP := net.ParseIP(s.VirtualIP)
			maskIP := net.ParseIP(s.DeviceMask)
			if ip != nil && myIP != nil && maskIP != nil {
				mask := net.IPMask(maskIP.To4())
				myNet := net.IPNet{IP: myIP.Mask(mask), Mask: mask}
				if !myNet.Contains(ip) {
					log.Errorf("Security Warning: received exit node IP %s which is outside our virtual network. Ignoring.", newExitNodeIP)
					return
				}
			}
		}

		log.Infof("Exit node changed from %s to %s", s.ExitNodeIP, newExitNodeIP)
		s.ExitNodeIP = newExitNodeIP
		if err := s.SetExitNode(s.ExitNodeIP); err != nil {
			log.Errorf("Failed to update exit node: %v", err)
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
