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

func runCmd(name string, args ...string) (string, error) {
	cmd := exec.Command(name, args...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return string(out), fmt.Errorf("command %s %v failed: %v, output: %s", name, args, err, string(out))
	}
	return string(out), nil
}

// Linux
func setupExitNodeLinux(exitNodeIP, supernodeIP string) error {
	// 1. Get current gateway
	out, err := runCmd("sh", "-c", "ip route get 8.8.8.8 | head -n1 | awk '{ print $3 }'")
	if err != nil {
		return err
	}
	originalGateway = strings.TrimSpace(out)
	if originalGateway == "" {
		return fmt.Errorf("could not determine current gateway")
	}

	// 2. Add route to supernode via original gateway
	_, err = runCmd("ip", "route", "add", supernodeIP, "via", originalGateway)
	if err != nil {
		return err
	}

	// 3. Change default gateway
	_, _ = runCmd("ip", "route", "del", "default")

	_, err = runCmd("ip", "route", "add", "default", "via", exitNodeIP)
	if err != nil {
		restoreExitNodeLinux() // Try to cleanup
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

// Darwin
func setupExitNodeDarwin(exitNodeIP, supernodeIP string) error {
	// 1. Get current gateway
	out, err := runCmd("sh", "-c", "route -n get default | grep gateway | awk '{print $2}'")
	if err != nil {
		return err
	}
	originalGateway = strings.TrimSpace(out)
	if originalGateway == "" {
		return fmt.Errorf("could not determine current gateway")
	}

	// 2. Add route to supernode via original gateway
	_, err = runCmd("route", "-n", "add", "-net", supernodeIP, originalGateway)
	if err != nil {
		return err
	}

	// 3. Change default gateway
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

// Windows
func setupExitNodeWindows(exitNodeIP, supernodeIP string) error {
	// Windows implementation usually requires administrative privileges
	// 1. Get current gateway (simplified, ideally use netsh or powershell)
	out, err := runCmd("sh", "-c", "route print 0.0.0.0 | findstr 0.0.0.0")
	if err != nil {
		// Try alternative for standard windows shell if 'sh' is missing
		out, err = runCmd("cmd", "/c", "route print 0.0.0.0")
	}
	// Note: Parsing Windows route print output is complex.
	// This is a placeholder for the logic.
	log.Debugf("Windows route print output: %s", out)

	// Placeholder logic
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
