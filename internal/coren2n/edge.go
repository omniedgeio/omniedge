//go:build !windows
// +build !windows

package coren2n

//go:generate sh -c "rm -rf n2n; git clone https://github.com/omniedgeio/n2n.git; cp make_n2n n2n/; cd n2n; chmod +x make_n2n; git checkout 2.6-stable-omni; if [ \"$(uname)\" = 'Darwin' ] && [ -f ../../patches/tuntap_osx_utun.c ]; then cp ../../patches/tuntap_osx_utun.c tuntap_osx.c; fi; if [ -f ../../patches/n2n_multicast.patch ]; then patch -p0 < ../../patches/n2n_multicast.patch; fi; ./make_n2n"

/*
#cgo CFLAGS: -g3 -Wall
#cgo LDFLAGS: -ln2n -L${SRCDIR}/n2n
#include "edge.h"
*/
import "C"
import (
	"errors"

	log "github.com/sirupsen/logrus"
)

type Edge struct {
	Id         string
	PrivateKey string

	AllowP2P             bool
	AllowRouting         bool
	CommunityName        string // The community. 16 full octets
	DisablePMTUDiscovery bool
	DropMulticast        bool // Multicast ethernet addresses
	EncryptKey           string
	LocalPort            int
	ManagementPort       int
	MTU                  int
	NumberMaxSnPings     int
	SuperNodeNum         int
	TransopId            int
	Tos                  int // TOS for sent packets
	RegisterInterval     int // Interval for supernode registration, also used for UDP NAT hole punching
	RegisterTTL          int // TTL for registration packet when UDP NAT hole punching through supernode
	SuperNodeHostPort    string

	// used for tun/tap device
	DeviceIP     string
	DeviceMac    string
	DeviceMask   string
	DeviceName   string
	DeviceIPMode string // dhcp or static

	cTuntapDevice C.tuntap_dev
	cConf         C.n2n_edge_conf_t
	cKeepRunning  C.int
}

func (e *Edge) getCIntFromGoBool(goBool bool) C.int {
	cInt := C.int(0)
	if goBool {
		cInt = C.int(1)
	}
	return cInt
}

func (e *Edge) Configure() error {
	if err := C.edge_configure(
		&e.cConf,
		C.CString(e.SuperNodeHostPort),
		C.CString(e.PrivateKey),
		e.getCIntFromGoBool(e.AllowP2P),
		e.getCIntFromGoBool(e.AllowRouting),
		C.CString(e.CommunityName),
		e.getCIntFromGoBool(e.DisablePMTUDiscovery),
		e.getCIntFromGoBool(e.DropMulticast),
		C.CString(e.EncryptKey),
		C.int(e.LocalPort),
		C.int(e.ManagementPort),
		C.int(e.SuperNodeNum),
		C.int(e.TransopId),
		C.int(e.Tos),
		C.int(e.RegisterInterval),
		C.int(e.RegisterTTL),
	); int(err) < 0 {
		log.Errorf("Fail to config omniedge, err code is %d", int(err))
		return errors.New("fail to configure internal")
	}
	return nil
}

func (e *Edge) OpenTunTapDevice() error {
	if err := C.tuntap_open(
		&e.cTuntapDevice,
		C.CString(e.DeviceName),
		C.CString(e.DeviceIPMode),
		C.CString(e.DeviceIP),
		C.CString(e.DeviceMask),
		C.CString(e.DeviceMac),
		C.int(e.MTU),
	); int(err) < 0 {
		return errors.New("fail to open TUN/TAP device")
	}
	return nil
}

func (e *Edge) Start() error {
	e.cKeepRunning = C.int(1)

	if errCode := C.edge_start(
		&e.cTuntapDevice,
		&e.cConf,
		&e.cKeepRunning,
	); int(errCode) != 0 {
		return errors.New("could not start internal")
	}
	return nil
}

func (e *Edge) Stop() {
	e.cKeepRunning = C.int(0)
}
