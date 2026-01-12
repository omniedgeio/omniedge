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
	originalGateway  string
	supernodeRouteIP string
	isExitNodeActive bool
)

// SetupExitNode configures the system to use the specified exit node
func SetupExitNode(exitNodeIP string, supernodeHost string) error {
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
	supernodeRouteIP = addrs[0].String()

	var setupErr error
	switch runtime.GOOS {
	case "linux":
		setupErr = setupExitNodeLinux(exitNodeIP, supernodeRouteIP)
	case "darwin":
		setupErr = setupExitNodeDarwin(exitNodeIP, supernodeRouteIP)
	case "windows":
		setupErr = setupExitNodeWindows(exitNodeIP, supernodeRouteIP)
	default:
		setupErr = fmt.Errorf("exit node not supported on %s", runtime.GOOS)
	}

	if setupErr != nil {
		return setupErr
	}

	// Setup DNS to ensure internet access works through the tunnel
	if err := SetupDNS(); err != nil {
		log.Warnf("Failed to setup DNS: %v. Internet access may be limited.", err)
	}

	isExitNodeActive = true
	return nil
}

// RestoreExitNode restores the system's original routing configuration
func RestoreExitNode() error {
	if !isExitNodeActive {
		return nil
	}

	_ = RestoreDNS()

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
		supernodeRouteIP = ""
	}
	return err
}

// EnableExitNodeForwarding enables IP forwarding and NAT for acting as an exit node
func EnableExitNodeForwarding() error {
	switch runtime.GOOS {
	case "linux":
		return enableForwardingLinux()
	case "darwin":
		return enableForwardingDarwin()
	default:
		return fmt.Errorf("exit node forwarding not supported on %s", runtime.GOOS)
	}
}

// DisableExitNodeForwarding disables IP forwarding and NAT
func DisableExitNodeForwarding() error {
	switch runtime.GOOS {
	case "linux":
		return disableForwardingLinux()
	case "darwin":
		return disableForwardingDarwin()
	default:
		return nil
	}
}

// SetupDNS configures a public DNS (8.8.8.8) to ensure connectivity through the tunnel
func SetupDNS() error {
	switch runtime.GOOS {
	case "linux":
		return setupDNSLinux()
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

func setupDNSLinux() error {
	// 1. Backup /etc/resolv.conf if it's a file
	_, err := runCmd("sh", "-c", "test -f /etc/resolv.conf && ! test -L /etc/resolv.conf")
	if err == nil {
		_, _ = runCmd("sudo", "cp", "/etc/resolv.conf", "/etc/resolv.conf.omniedge_bak")
	}

	// 2. Set nameserver 8.8.8.8
	log.Info("Setting DNS to 8.8.8.8 (Linux)")
	_, err = runCmd("sh", "-c", "echo \"nameserver 8.8.8.8\" | sudo tee /etc/resolv.conf")
	return err
}

func restoreDNSLinux() error {
	_, err := runCmd("sh", "-c", "test -f /etc/resolv.conf.omniedge_bak")
	if err == nil {
		log.Info("Restoring DNS from backup (Linux)")
		_, _ = runCmd("sudo", "mv", "/etc/resolv.conf.omniedge_bak", "/etc/resolv.conf")
	}
	return nil
}

func setupDNSDarwin() error {
	// 1. Get primary service
	out, err := runCmd("sh", "-c", "networksetup -listallnetworkservices | grep -v '*' | head -n 1")
	if err != nil {
		return err
	}
	service := strings.TrimSpace(out)
	if service == "" {
		return fmt.Errorf("no network service found")
	}

	// 2. Set DNS
	log.Infof("Setting DNS to 8.8.8.8 for service %s (macOS)", service)
	_, err = runCmd("sudo", "networksetup", "-setdnsservers", service, "8.8.8.8")
	return err
}

func restoreDNSDarwin() error {
	out, err := runCmd("sh", "-c", "networksetup -listallnetworkservices | grep -v '*' | head -n 1")
	if err == nil {
		service := strings.TrimSpace(out)
		if service != "" {
			log.Infof("Restoring DNS to Empty (DHCP default) for service %s (macOS)", service)
			_, _ = runCmd("sudo", "networksetup", "-setdnsservers", service, "Empty")
		}
	}
	return nil
}

func runCmd(name string, args ...string) (string, error) {
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

func enableForwardingLinux() error {
	// 1. Enable IP Forwarding
	_, err := runCmd("sudo", "sysctl", "-w", "net.ipv4.ip_forward=1")
	if err != nil {
		log.Warnf("Failed to enable IP forwarding via sysctl: %v. VPN may still work if already enabled.", err)
	}

	// 2. Detect external interface
	out, err := runCmd("sh", "-c", "ip route get 8.8.8.8 | head -n1 | awk '{print $5}'")
	if err != nil {
		return fmt.Errorf("failed to detect external interface: %v", err)
	}
	extIf := strings.TrimSpace(out)
	if extIf == "" {
		return fmt.Errorf("could not determine external interface")
	}

	// 3. Add NAT Masquerade rule (idempotent check)
	checkCmd := fmt.Sprintf("sudo iptables -t nat -C POSTROUTING -s 100.100.0.0/16 -o %s -j MASQUERADE", extIf)
	_, err = runCmd("sh", "-c", checkCmd)
	if err != nil {
		// Rule doesn't exist, add it
		addCmd := fmt.Sprintf("sudo iptables -t nat -A POSTROUTING -s 100.100.0.0/16 -o %s -j MASQUERADE", extIf)
		_, err = runCmd("sh", "-c", addCmd)
		if err != nil {
			return fmt.Errorf("failed to add iptables NAT rule: %v", err)
		}
	}

	log.Infof("Exit node forwarding enabled on %s", extIf)
	return nil
}

func disableForwardingLinux() error {
	out, err := runCmd("sh", "-c", "ip route get 8.8.8.8 | head -n1 | awk '{print $5}'")
	if err == nil {
		extIf := strings.TrimSpace(out)
		if extIf != "" {
			delCmd := fmt.Sprintf("sudo iptables -t nat -D POSTROUTING -s 100.100.0.0/16 -o %s -j MASQUERADE", extIf)
			_, _ = runCmd("sh", "-c", delCmd)
		}
	}
	return nil
}

// --- Darwin Forwarding & NAT ---

func enableForwardingDarwin() error {
	// 1. Enable IP Forwarding
	_, err := runCmd("sudo", "sysctl", "-w", "net.inet.ip.forwarding=1")
	if err != nil {
		log.Warnf("Failed to enable IP forwarding via sysctl: %v", err)
	}

	// 2. Detect external interface
	out, err := runCmd("sh", "-c", "route -n get 8.8.8.8 | grep interface | awk '{print $2}'")
	if err != nil {
		return fmt.Errorf("failed to detect external interface: %v", err)
	}
	extIf := strings.TrimSpace(out)
	if extIf == "" {
		return fmt.Errorf("could not determine external interface")
	}

	// 3. Configure PF NAT
	pfRule := fmt.Sprintf("nat on %s from 100.100.0.0/16 to any -> (%s)", extIf, extIf)
	pfConfig := fmt.Sprintf("echo \"%s\" | sudo pfctl -a omniedge -f -", pfRule)

	_, err = runCmd("sh", "-c", pfConfig)
	if err != nil {
		return fmt.Errorf("failed to configure pfctl NAT: %v", err)
	}

	_, _ = runCmd("sudo", "pfctl", "-e") // Ensure PF is enabled

	log.Infof("Exit node forwarding enabled on %s (macOS PF)", extIf)
	return nil
}

func disableForwardingDarwin() error {
	_, _ = runCmd("sudo", "pfctl", "-a", "omniedge", "-F", "nat")
	return nil
}

// Linux Exit Node Setup
func setupExitNodeLinux(exitNodeIP, supernodeIP string) error {
	out, err := runCmd("sh", "-c", "ip route get 8.8.8.8 | head -n1 | awk '{ print $3 }'")
	if err != nil {
		return err
	}
	originalGateway = strings.TrimSpace(out)
	if originalGateway == "" {
		return fmt.Errorf("could not determine current gateway")
	}

	_, err = runCmd("sudo", "ip", "route", "add", supernodeIP, "via", originalGateway)
	if err != nil {
		return err
	}

	_, _ = runCmd("sudo", "ip", "route", "del", "default")
	_, err = runCmd("sudo", "ip", "route", "add", "default", "via", exitNodeIP)
	if err != nil {
		restoreExitNodeLinux()
		return err
	}

	return nil
}

func restoreExitNodeLinux() error {
	if originalGateway != "" {
		_, _ = runCmd("sudo", "ip", "route", "del", "default")
		_, _ = runCmd("sudo", "ip", "route", "add", "default", "via", originalGateway)
	}
	if supernodeRouteIP != "" && originalGateway != "" {
		_, _ = runCmd("sudo", "ip", "route", "del", supernodeRouteIP, "via", originalGateway)
	}
	return nil
}

// Darwin Exit Node Setup
func setupExitNodeDarwin(exitNodeIP, supernodeIP string) error {
	out, err := runCmd("sh", "-c", "route -n get default | grep gateway | awk '{print $2}'")
	if err != nil {
		return err
	}
	originalGateway = strings.TrimSpace(out)
	if originalGateway == "" {
		return fmt.Errorf("could not determine current gateway")
	}

	_, err = runCmd("sudo", "route", "-n", "add", "-net", supernodeIP, originalGateway)
	if err != nil {
		return err
	}

	_, _ = runCmd("sudo", "route", "delete", "default")
	_, err = runCmd("sudo", "route", "-n", "add", "-net", "0.0.0.0", exitNodeIP)
	if err != nil {
		restoreExitNodeDarwin()
		return err
	}

	return nil
}

func restoreExitNodeDarwin() error {
	_, _ = runCmd("sudo", "route", "delete", "-net", "0.0.0.0")
	if originalGateway != "" {
		_, _ = runCmd("sudo", "route", "-n", "add", "-net", "0.0.0.0", originalGateway)
	}
	if supernodeRouteIP != "" && originalGateway != "" {
		_, _ = runCmd("sudo", "route", "delete", "-net", supernodeRouteIP)
	}
	return nil
}

// Windows Exit Node Setup
func setupExitNodeWindows(exitNodeIP, supernodeIP string) error {
	out, err := runCmd("sh", "-c", "route print 0.0.0.0 | findstr 0.0.0.0")
	if err != nil {
		out, err = runCmd("cmd", "/c", "route print 0.0.0.0")
	}
	log.Debugf("Windows route print output: %s", out)

	_, _ = runCmd("route", "delete", "0.0.0.0")
	_, err = runCmd("route", "ADD", supernodeIP, "MASK", "255.255.255.255", originalGateway)
	_, err = runCmd("route", "ADD", "0.0.0.0", "MASK", "0.0.0.0", exitNodeIP)

	return nil
}

func restoreExitNodeWindows() error {
	_, _ = runCmd("route", "delete", "0.0.0.0")
	_, _ = runCmd("route", "ADD", "0.0.0.0", "MASK", "0.0.0.0", originalGateway)
	_, _ = runCmd("route", "delete", supernodeRouteIP)
	return nil
}
