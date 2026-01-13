package core

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
)

func GetPidFile() string {
	if runtime.GOOS == "windows" {
		return filepath.Join(os.Getenv("APPDATA"), "omniedge", "omniedge.pid")
	}
	return "/tmp/omniedge.pid"
}

func GetLogFile() string {
	if runtime.GOOS == "windows" {
		return filepath.Join(os.Getenv("APPDATA"), "omniedge", "omniedge.log")
	}
	return "/tmp/omniedge.log"
}

func Daemonize(customArgs ...string) error {
	// Level 3: Real Background Daemon
	if os.Getenv("OMNIEDGE_DAEMON") == "1" {
		// We are already the daemon.
		// Redirect standard FDs to log file to catch all output (including C/n2n)
		logPath := GetLogFile()
		logFile, err := os.OpenFile(logPath, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0644)
		if err == nil {
			// Redirect stdout/stderr at the FD level
			dupFD(int(logFile.Fd()), 1)
			dupFD(int(logFile.Fd()), 2)
		}
		return nil
	}

	executable, err := os.Executable()
	if err != nil {
		return err
	}

	args := os.Args[1:]
	if len(customArgs) > 0 {
		args = customArgs
	}

	// Level 1: Initial User Process -> Elevate to Root if needed
	// On Unix-like systems, if not root, use sudo to elevate
	if runtime.GOOS != "windows" && os.Geteuid() != 0 {
		fmt.Println("Elevation required to manage network interfaces. Please enter your password:")
		sudoArgs := append([]string{"-E", executable}, args...)
		cmd := exec.Command("sudo", sudoArgs...)
		// We need to keep stdin/stdout/stderr connected so the user can see/respond to the sudo prompt
		cmd.Stdin = os.Stdin
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr

		// This will stay in foreground until sudo is done
		err := cmd.Run()
		if err != nil {
			return fmt.Errorf("elevation failed: %v", err)
		}
		// If sudo finished successfully, this process (Level 1) can exit
		os.Exit(0)
	}

	// Level 2: Root process (either started as root or via sudo) -> Background into Level 3
	// Start the background process.
	daemonCmd := exec.Command(executable, args...)
	daemonCmd.Env = append(os.Environ(), "OMNIEDGE_DAEMON=1")

	// Detach from terminal
	setDetachAttr(daemonCmd)

	logFile, err := os.OpenFile(GetLogFile(), os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0644)
	if err != nil {
		return fmt.Errorf("failed to open log file: %v", err)
	}
	daemonCmd.Stdout = logFile
	daemonCmd.Stderr = logFile

	if err := daemonCmd.Start(); err != nil {
		return fmt.Errorf("failed to start daemon: %v", err)
	}

	pidFile := GetPidFile()
	if err := os.MkdirAll(filepath.Dir(pidFile), 0755); err != nil {
		return err
	}
	if err := os.WriteFile(pidFile, []byte(fmt.Sprintf("%d", daemonCmd.Process.Pid)), 0644); err != nil {
		return err
	}

	fmt.Printf("OmniEdge started in background (PID: %d)\n", daemonCmd.Process.Pid)
	fmt.Printf("Logs: %s\n", GetLogFile())
	os.Exit(0)
	return nil
}
