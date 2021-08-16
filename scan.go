package edgecli

import (
	"context"
	"errors"
	"fmt"
	"github.com/Ullaakut/nmap/v2"
	log "github.com/sirupsen/logrus"
	"time"
)

type ScanOption struct {
	Cidr    string
	Timeout int64
}
type ScanService struct {
	ScanOption
}

type ScanResult struct {
	HostName   string
	IPv4       string
	IPv6       string
	MacAddress string
	Vendor     string
	OS         string
}

func (s *ScanService) Scan(option *ScanOption) (*[]ScanResult, error) {
	ctx, cancel := context.WithTimeout(context.Background(), time.Duration(option.Timeout)*time.Second)
	defer cancel()

	scanner, err := nmap.NewScanner(
		nmap.WithTargets(option.Cidr),
		nmap.WithCustomArguments("-sP"),
		nmap.WithContext(ctx),
	)
	if err != nil {
		return nil, errors.New(fmt.Sprintf("unable to create nmap scanner: %v", err))
	}
	result, _, err := scanner.Run()
	if err != nil {
		return nil, errors.New(fmt.Sprintf("nmap scan failed: %v", err))
	}
	res := handleScanNmapResult(result)
	return res, nil
}

func handleScanNmapResult(result *nmap.Run) *[]ScanResult {
	res := []ScanResult{}
	for _, host := range result.Hosts {
		sr := ScanResult{}
		for _, addr := range host.Addresses {
			log.Infof("scan result addr: %s, type: %s, vendor: %s", addr.Addr, addr.AddrType, addr.Vendor)
			if addr.AddrType == "ipv4" && sr.IPv4 == "" {
				sr.IPv4 = addr.Addr
			}
			if addr.AddrType == "mac" && sr.MacAddress == "" {
				sr.MacAddress = addr.Addr
			}
			if addr.AddrType == "ipv6" && sr.IPv6 == "" {
				sr.IPv6 = addr.Addr
			}
			if addr.Vendor != "" && sr.Vendor == "" {
				sr.Vendor = addr.Vendor
			}
		}
		res = append(res, sr)
	}
	return &res
}
