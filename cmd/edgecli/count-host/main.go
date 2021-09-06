package main

import (
	"context"
	"fmt"
	"log"
	"time"

	"github.com/Ullaakut/nmap/v2"
	osfamily "github.com/Ullaakut/nmap/v2/pkg/osfamilies"
)

func main() {
	// Equivalent to
	// nmap -F -O 192.168.0.0/24
	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()
	scanner, err := nmap.NewScanner(
		nmap.WithTargets("192.168.32.0/24"),
		//nmap.WithFastMode(),
		//nmap.WithOSDetection(),
		nmap.WithCustomArguments("-sP"),
		nmap.WithContext(ctx),
	)
	if err != nil {
		log.Fatalf("unable to create nmap scanner: %v", err)
	}

	result, _, err := scanner.Run()
	if err != nil {
		log.Fatalf("nmap scan failed: %v", err)
	}

	countByOS(result)
}

func countByOS(result *nmap.Run) {
	var (
		linux, windows int
	)

	// Count the number of each OS for all hosts.
	for _, host := range result.Hosts {
		fmt.Println("===============================")
		for _, addr := range host.Addresses {
			fmt.Printf("address: %s, type: %s, vendor: %s\n", addr.Addr, addr.AddrType, addr.Vendor)
		}
		//fmt.Printf("address:  %+v \n host name: %+v \n", host.Addresses, host.Hostnames)
		fmt.Printf("finger: %+v \n", host.OS.Fingerprints)
		for _, match := range host.OS.Matches {
			fmt.Printf("os: %+v \n", match)
			for _, class := range match.Classes {
				switch class.OSFamily() {
				case osfamily.Linux:
					linux++
				case osfamily.Windows:
					windows++
				}
			}
		}
	}
	fmt.Printf("Discovered %d linux hosts and %d windows hosts out of %d total up hosts.\n", linux, windows, result.Stats.Hosts.Up)
}
