package core

import (
	"encoding/hex"
	"net"
)

type CIDR struct {
	ip    net.IP
	ipnet *net.IPNet
}

func (c CIDR) Mask() string {
	mask, _ := hex.DecodeString(c.ipnet.Mask.String())
	return net.IP([]byte(mask)).String()
}

func (c CIDR) Contains(ip string) bool {
	i := net.ParseIP(ip)
	return c.ipnet.Contains(i)
}

func (c CIDR) Ip() string {
	return c.ip.String()
}

func ParseCIDR(s string) (*CIDR, error) {
	i, n, err := net.ParseCIDR(s)
	if err != nil {
		return nil, err
	}
	return &CIDR{ip: i, ipnet: n}, nil
}
