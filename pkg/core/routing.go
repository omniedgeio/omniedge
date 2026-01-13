package core

import (
	"fmt"
	"net"
	"os"
	"os/exec"
	"runtime"
	"strings"

	log "github.com/sirupsen/logrus"
)

var (
	originalGateway   string
	supernodeRouteIPs []string
	isExitNodeActive  bool
	dnsInterface      string
)

// SetupExitNode configures the system to use the specified exit node
func SetupExitNode(exitNodeIP string, supernodeHost string, localVIP string) error {
	if exitNodeIP == "" {
		return nil
	}

	if net.ParseIP(exitNodeIP) == nil {
		return fmt.Errorf("invalid exit node IP: %s", exitNodeIP)
	}

	if isExitNodeActive {
		log.Warn("Exit node is already active, restoring first...")
		RestoreExitNode()
	}

	// Resolve supernode IP for the persistent route
	host, _, err := net.SplitHostPort(supernodeHost)
	if err != nil {
		host = supernodeHost
	}
	addrs, err := net.LookupIP(host)
	if err != nil || len(addrs) == 0 {
		return fmt.Errorf("failed to resolve supernode host %s: %v", host, err)
	}

	supernodeRouteIPs = []string{}
	for _, addr := range addrs {
		supernodeRouteIPs = append(supernodeRouteIPs, addr.String())
	}

	var setupErr error
	switch runtime.GOOS {
	case "linux":
		setupErr = setupExitNodeLinux(exitNodeIP)
	case "darwin":
		setupErr = setupExitNodeDarwin(exitNodeIP)
	case "windows":
		setupErr = setupExitNodeWindows(exitNodeIP)
	default:
		setupErr = fmt.Errorf("exit node not supported on %s", runtime.GOOS)
	}

	if setupErr != nil {
		return setupErr
	}

	isExitNodeActive = true
	return nil
}

// RestoreExitNode restores the system's original routing configuration
func RestoreExitNode() error {
	if !isExitNodeActive {
		return nil
	}

	var err error
	switch runtime.GOOS {
	case "linux":
		err = restoreExitNodeLinux()
	case "darwin":
		err = restoreExitNodeDarwin()
	case "windows":
		err = restoreExitNodeWindows()
	default:
		err = fmt.Errorf("exit node not supported on %s", runtime.GOOS)
	}

	if err == nil {
		isExitNodeActive = false
		originalGateway = ""
		supernodeRouteIPs = []string{}
	}
	return err
}

// EnableExitNodeForwarding enables IP forwarding and NAT for acting as an exit node
func EnableExitNodeForwarding(cidr string) error {
	switch runtime.GOOS {
	case "linux":
		return enableForwardingLinux(cidr)
	case "darwin":
		return enableForwardingDarwin(cidr)
	default:
		return fmt.Errorf("exit node forwarding not supported on %s", runtime.GOOS)
	}
}

// DisableExitNodeForwarding disables IP forwarding and NAT
func DisableExitNodeForwarding(cidr string) error {
	switch runtime.GOOS {
	case "linux":
		return disableForwardingLinux(cidr)
	case "darwin":
		return disableForwardingDarwin(cidr)
	default:
		return nil
	}
}

// SetupDNS configures a public DNS (8.8.8.8) to ensure connectivity through the tunnel
func SetupDNS(localVIP string) error {
	switch runtime.GOOS {
	case "linux":
		return setupDNSLinux(localVIP)
	case "darwin":
		return setupDNSDarwin()
	default:
		return nil
	}
}

// RestoreDNS restores the original DNS settings
func RestoreDNS() error {
	switch runtime.GOOS {
	case "linux":
		return restoreDNSLinux()
	case "darwin":
		return restoreDNSDarwin()
	default:
		return nil
	}
}

func setupDNSLinux(localVIP string) error {
	iface, err := findInterfaceByIP(localVIP)
	if err == nil {
		dnsInterface = iface
		// 1. Try resolvectl (Modern systemd-based distros)
		if _, err := exec.LookPath("resolvectl"); err == nil {
			log.Infof("Using resolvectl to set DNS for %s (Linux)", iface)
			_, err1 := RunCmd("sudo", "resolvectl", "dns", iface, "8.8.8.8")
			_, err2 := RunCmd("sudo", "resolvectl", "domain", iface, "~.")
			if err1 == nil && err2 == nil {
				return nil
			}
			log.Warnf("resolvectl failed: %v, %v. Falling back...", err1, err2)
		}

		// 2. Try resolvconf (Common on Debian/Ubuntu non-systemd or mixed)
		if _, err := exec.LookPath("resolvconf"); err == nil {
			log.Infof("Using resolvconf to set DNS for %s (Linux)", iface)
			cmd := fmt.Sprintf("echo \"nameserver 8.8.8.8\" | sudo resolvconf -a %s", iface)
			if _, err := RunCmd("sh", "-c", cmd); err == nil {
				return nil
			}
		}
	}

	// 3. Last Resort: direct /etc/resolv.conf modification (Destructive)
	log.Warn("No DNS manager found (resolvectl/resolvconf). Falling back to direct /etc/resolv.conf modification.")

	// Backup /etc/resolv.conf if it's a file
	_, err = RunCmd("sh", "-c", "test -f /etc/resolv.conf && ! test -L /etc/resolv.conf")
	if err == nil {
		_, _ = RunCmd("sudo", "cp", "/etc/resolv.conf", "/etc/resolv.conf.omniedge_bak")
	}

	// Set nameserver 8.8.8.8
	log.Info("Setting DNS to 8.8.8.8 (Linux)")
	_, err = RunCmd("sh", "-c", "echo \"nameserver 8.8.8.8\" | sudo tee /etc/resolv.conf")
	return err
}

func findInterfaceByIP(ip string) (string, error) {
	ifaces, err := net.Interfaces()
	if err != nil {
		return "", err
	}
	for _, iface := range ifaces {
		addrs, err := iface.Addrs()
		if err != nil {
			continue
		}
		for _, addr := range addrs {
			if strings.Contains(addr.String(), ip) {
				return iface.Name, nil
			}
		}
	}
	return "", fmt.Errorf("interface not found for IP %s", ip)
}

func restoreDNSLinux() error {
	if dnsInterface != "" {
		if _, err := exec.LookPath("resolvectl"); err == nil {
			log.Infof("Reverting DNS for %s (Linux resolvectl)", dnsInterface)
			_, _ = RunCmd("sudo", "resolvectl", "revert", dnsInterface)
		}
		if _, err := exec.LookPath("resolvconf"); err == nil {
			log.Infof("Removing DNS for %s (Linux resolvconf)", dnsInterface)
			_, _ = RunCmd("sudo", "resolvconf", "-d", dnsInterface)
		}
		dnsInterface = ""
	}

	_, err := RunCmd("sh", "-c", "test -f /etc/resolv.conf.omniedge_bak")
	if err == nil {
		log.Info("Restoring DNS from backup (Linux)")
		_, _ = RunCmd("sudo", "mv", "/etc/resolv.conf.omniedge_bak", "/etc/resolv.conf")
	}
	return nil
}

func setupDNSDarwin() error {
	// 1. Get primary service
	out, err := RunCmd("sh", "-c", "networksetup -listallnetworkservices | grep -v '*' | head -n 1")
	if err != nil {
		return err
	}
	service := strings.TrimSpace(out)
	if service == "" {
		return fmt.Errorf("no network service found")
	}

	// 2. Set DNS
	log.Infof("Setting DNS to 8.8.8.8 for service %s (macOS)", service)
	_, err = RunCmd("sudo", "networksetup", "-setdnsservers", service, "8.8.8.8")
	return err
}

func restoreDNSDarwin() error {
	out, err := RunCmd("sh", "-c", "networksetup -listallnetworkservices | grep -v '*' | head -n 1")
	if err == nil {
		service := strings.TrimSpace(out)
		if service != "" {
			log.Infof("Restoring DNS to Empty (DHCP default) for service %s (macOS)", service)
			_, _ = RunCmd("sudo", "networksetup", "-setdnsservers", service, "Empty")
		}
	}
	return nil
}

func RunCmd(name string, args ...string) (string, error) {
	if name == "sudo" {
		// If we are already root, skip sudo
		if os.Getuid() == 0 {
			name = args[0]
			args = args[1:]
		} else {
			// Check if sudo is available
			if _, err := exec.LookPath("sudo"); err != nil {
				log.Warn("sudo not found in PATH, attempting to run without it")
				name = args[0]
				args = args[1:]
			}
		}
	}
	cmd := exec.Command(name, args...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("command %s %v failed: %v, output: %s", name, args, err, string(out))
	}
	return string(out), nil
}

// --- Linux Forwarding & NAT ---

func enableForwardingLinux(cidr string) error {
	if cidr == "" {
		cidr = "100.100.0.0/16" // Fallback
	}
	// 1. Enable IP Forwarding
	_, err := RunCmd("sudo", "sysctl", "-w", "net.ipv4.ip_forward=1")
	if err != nil {
		log.Warnf("Failed to enable IP forwarding via sysctl: %v. VPN may still work if already enabled.", err)
	}

	// 2. Detect external interface
	out, err := RunCmd("sh", "-c", "ip route get 8.8.8.8 | head -n1 | awk '{print $5}'")
	if err != nil {
		return fmt.Errorf("failed to detect external interface: %v", err)
	}
	extIf := strings.TrimSpace(out)
	if extIf == "" {
		return fmt.Errorf("could not determine external interface")
	}

	// 3. Add NAT Masquerade rule (idempotent check)
	checkCmd := fmt.Sprintf("sudo iptables -t nat -C POSTROUTING -s %s -o %s -j MASQUERADE", cidr, extIf)
	_, err = RunCmd("sh", "-c", checkCmd)
	if err != nil {
		// Rule doesn't exist, add it
		addCmd := fmt.Sprintf("sudo iptables -t nat -A POSTROUTING -s %s -o %s -j MASQUERADE", cidr, extIf)
		_, err = RunCmd("sh", "-c", addCmd)
		if err != nil {
			return fmt.Errorf("failed to add iptables NAT rule: %v", err)
		}
	}

	log.Infof("Exit node forwarding enabled for %s on %s", cidr, extIf)
	return nil
}

func disableForwardingLinux(cidr string) error {
	if cidr == "" {
		cidr = "100.100.0.0/16"
	}
	out, err := RunCmd("sh", "-c", "ip route get 8.8.8.8 | head -n1 | awk '{print $5}'")
	if err == nil {
		extIf := strings.TrimSpace(out)
		if extIf != "" {
			delCmd := fmt.Sprintf("sudo iptables -t nat -D POSTROUTING -s %s -o %s -j MASQUERADE", cidr, extIf)
			_, _ = RunCmd("sh", "-c", delCmd)
		}
	}
	return nil
}

// --- Darwin Forwarding & NAT ---

func enableForwardingDarwin(cidr string) error {
	if cidr == "" {
		cidr = "100.100.0.0/16"
	}
	// 1. Enable IP Forwarding
	_, err := RunCmd("sudo", "sysctl", "-w", "net.inet.ip.forwarding=1")
	if err != nil {
		log.Warnf("Failed to enable IP forwarding via sysctl: %v", err)
	}

	// 2. Detect external interface
	out, err := RunCmd("sh", "-c", "route -n get 8.8.8.8 | grep interface | awk '{print $2}'")
	if err != nil {
		return fmt.Errorf("failed to detect external interface: %v", err)
	}
	extIf := strings.TrimSpace(out)
	if extIf == "" {
		return fmt.Errorf("could not determine external interface")
	}

	// 3. Configure PF NAT
	pfRule := fmt.Sprintf("nat on %s from %s to any -> (%s)", extIf, cidr, extIf)
	pfConfig := fmt.Sprintf("echo \"%s\" | sudo pfctl -a omniedge -f -", pfRule)

	_, err = RunCmd("sh", "-c", pfConfig)
	if err != nil {
		return fmt.Errorf("failed to configure pfctl NAT: %v", err)
	}

	_, _ = RunCmd("sudo", "pfctl", "-e") // Ensure PF is enabled

	log.Infof("Exit node forwarding enabled for %s on %s (macOS PF)", cidr, extIf)
	return nil
}

func disableForwardingDarwin(cidr string) error {
	_, _ = RunCmd("sudo", "pfctl", "-a", "omniedge", "-F", "all")
	return nil
}

// Linux Exit Node Setup
func setupExitNodeLinux(exitNodeIP string) error {
	out, err := RunCmd("sh", "-c", "ip route get 8.8.8.8 | head -n1 | awk '{ print $3 }'")
	if err != nil {
		return err
	}
	originalGateway = strings.TrimSpace(out)
	if originalGateway == "" {
		return fmt.Errorf("could not determine current gateway")
	}

	for _, ip := range supernodeRouteIPs {
		_, _ = RunCmd("sudo", "ip", "route", "add", ip, "via", originalGateway)
	}

	// Clamp MSS to avoid fragmentation issues over the tunnel
	_, _ = RunCmd("sudo", "iptables", "-t", "mangle", "-A", "FORWARD", "-p", "tcp", "--tcp-flags", "SYN,RST", "SYN", "-j", "TCPMSS", "--set-mss", "1360")
	_, _ = RunCmd("sudo", "iptables", "-t", "mangle", "-A", "OUTPUT", "-p", "tcp", "--tcp-flags", "SYN,RST", "SYN", "-j", "TCPMSS", "--set-mss", "1360")

	_, _ = RunCmd("sudo", "ip", "route", "del", "default")
	_, err = RunCmd("sudo", "ip", "route", "add", "default", "via", exitNodeIP)
	if err != nil {
		restoreExitNodeLinux()
		return err
	}

	return nil
}

func restoreExitNodeLinux() error {
	_, _ = RunCmd("sudo", "iptables", "-t", "mangle", "-D", "FORWARD", "-p", "tcp", "--tcp-flags", "SYN,RST", "SYN", "-j", "TCPMSS", "--set-mss", "1360")
	_, _ = RunCmd("sudo", "iptables", "-t", "mangle", "-D", "OUTPUT", "-p", "tcp", "--tcp-flags", "SYN,RST", "SYN", "-j", "TCPMSS", "--set-mss", "1360")

	if originalGateway != "" {
		_, _ = RunCmd("sudo", "ip", "route", "del", "default")
		_, _ = RunCmd("sudo", "ip", "route", "add", "default", "via", originalGateway)
	}
	for _, ip := range supernodeRouteIPs {
		_, _ = RunCmd("sudo", "ip", "route", "del", ip, "via", originalGateway)
	}
	return nil
}

// Darwin Exit Node Setup
func setupExitNodeDarwin(exitNodeIP string) error {
	out, err := RunCmd("sh", "-c", "route -n get default | grep gateway | awk '{print $2}'")
	if err != nil {
		return err
	}
	originalGateway = strings.TrimSpace(out)
	if originalGateway == "" {
		return fmt.Errorf("could not determine current gateway")
	}

	for _, ip := range supernodeRouteIPs {
		_, _ = RunCmd("sudo", "route", "-n", "add", "-net", ip, originalGateway)
	}

	// Clamp MSS on macOS using PF
	pfRule := "scrub on any all reassemble tcp max-mss 1360"
	pfConfig := fmt.Sprintf("echo \"%s\" | sudo pfctl -a omniedge-mss -f -", pfRule)
	_, _ = RunCmd("sh", "-c", pfConfig)
	_, _ = RunCmd("sudo", "pfctl", "-e")

	_, _ = RunCmd("sudo", "route", "delete", "default")
	_, err = RunCmd("sudo", "route", "-n", "add", "-net", "0.0.0.0", exitNodeIP)
	if err != nil {
		restoreExitNodeDarwin()
		return err
	}

	return nil
}

func restoreExitNodeDarwin() error {
	_, _ = RunCmd("sudo", "pfctl", "-a", "omniedge-mss", "-F", "all")
	_, _ = RunCmd("sudo", "route", "delete", "-net", "0.0.0.0")
	if originalGateway != "" {
		_, _ = RunCmd("sudo", "route", "-n", "add", "-net", "0.0.0.0", originalGateway)
	}
	for _, ip := range supernodeRouteIPs {
		_, _ = RunCmd("sudo", "route", "delete", "-net", ip)
	}
	return nil
}

// Windows Exit Node Setup
func setupExitNodeWindows(exitNodeIP string) error {
	// Use powershell to get the default gateway IP reliably
	out, err := RunCmd("powershell", "-Command", "Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | Select-Object -ExpandProperty NextHop -First 1")
	if err != nil {
		return fmt.Errorf("failed to get windows gateway: %v", err)
	}
	originalGateway = strings.TrimSpace(out)
	if originalGateway == "" || originalGateway == "0.0.0.0" {
		return fmt.Errorf("could not determine windows gateway")
	}

	for _, ip := range supernodeRouteIPs {
		_, _ = RunCmd("route", "ADD", ip, "MASK", "255.255.255.255", originalGateway, "METRIC", "1")
	}

	_, _ = RunCmd("route", "DELETE", "0.0.0.0", "MASK", "0.0.0.0")
	_, err = RunCmd("route", "ADD", "0.0.0.0", "MASK", "0.0.0.0", exitNodeIP, "METRIC", "1")

	return err
}

func restoreExitNodeWindows() error {
	_, _ = RunCmd("route", "DELETE", "0.0.0.0", "MASK", "0.0.0.0")
	if originalGateway != "" {
		_, _ = RunCmd("route", "ADD", "0.0.0.0", "MASK", "0.0.0.0", originalGateway, "METRIC", "1")
	}
	for _, ip := range supernodeRouteIPs {
		_, _ = RunCmd("route", "DELETE", ip)
	}
	return nil
}
