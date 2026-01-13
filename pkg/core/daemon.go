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

func Daemonize() error {
	if os.Getenv("OMNIEDGE_DAEMON") == "1" {
		return nil
	}

	executable, err := os.Executable()
	if err != nil {
		return err
	}

	args := os.Args[1:]
	// Ensure we don't keep adding the flag if it was passed, 
	// though for 'start' we will always daemonize now.
	
	cmd := exec.Command(executable, args...)
	cmd.Env = append(os.Environ(), "OMNIEDGE_DAEMON=1")
	
	logFile, err := os.OpenFile(GetLogFile(), os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0644)
	if err != nil {
		return fmt.Errorf("failed to open log file: %v", err)
	}
	
	cmd.Stdout = logFile
	cmd.Stderr = logFile
	
	if err := cmd.Start(); err != nil {
		return fmt.Errorf("failed to start daemon: %v", err)
	}

	pidFile := GetPidFile()
	if err := os.MkdirAll(filepath.Dir(pidFile), 0755); err != nil {
		return err
	}
	if err := os.WriteFile(pidFile, []byte(fmt.Sprintf("%d", cmd.Process.Pid)), 0644); err != nil {
		return err
	}

	fmt.Printf("OmniEdge started in background (PID: %d)\n", cmd.Process.Pid)
	fmt.Printf("Logs: %s\n", GetLogFile())
	os.Exit(0)
	return nil
}
