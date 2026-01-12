package core

import (
	"fmt"
	"net"
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

	switch runtime.GOOS {
	case "linux":
		return setupExitNodeLinux(exitNodeIP, supernodeRouteIP)
	case "darwin":
		return setupExitNodeDarwin(exitNodeIP, supernodeRouteIP)
	case "windows":
		return setupExitNodeWindows(exitNodeIP, supernodeRouteIP)
	default:
		return fmt.Errorf("exit node not supported on %s", runtime.GOOS)
	}
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

func runCmd(name string, args ...string) (string, error) {
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

	_, err = runCmd("ip", "route", "add", supernodeIP, "via", originalGateway)
	if err != nil {
		return err
	}

	_, _ = runCmd("ip", "route", "del", "default")
	_, err = runCmd("ip", "route", "add", "default", "via", exitNodeIP)
	if err != nil {
		restoreExitNodeLinux()
		return err
	}

	isExitNodeActive = true
	return nil
}

func restoreExitNodeLinux() error {
	if originalGateway != "" {
		runCmd("ip", "route", "del", "default")
		runCmd("ip", "route", "add", "default", "via", originalGateway)
	}
	if supernodeRouteIP != "" && originalGateway != "" {
		runCmd("ip", "route", "del", supernodeRouteIP, "via", originalGateway)
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

	_, err = runCmd("route", "-n", "add", "-net", supernodeIP, originalGateway)
	if err != nil {
		return err
	}

	_, _ = runCmd("route", "delete", "default")
	_, err = runCmd("route", "-n", "add", "-net", "0.0.0.0", exitNodeIP)
	if err != nil {
		restoreExitNodeDarwin()
		return err
	}

	isExitNodeActive = true
	return nil
}

func restoreExitNodeDarwin() error {
	runCmd("route", "delete", "-net", "0.0.0.0")
	if originalGateway != "" {
		runCmd("route", "-n", "add", "-net", "0.0.0.0", originalGateway)
	}
	if supernodeRouteIP != "" && originalGateway != "" {
		runCmd("route", "delete", "-net", supernodeRouteIP)
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

	isExitNodeActive = true
	return nil
}

func restoreExitNodeWindows() error {
	runCmd("route", "delete", "0.0.0.0")
	runCmd("route", "ADD", "0.0.0.0", "MASK", "0.0.0.0", originalGateway)
	runCmd("route", "delete", supernodeRouteIP)
	return nil
}
