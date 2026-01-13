//go:build !windows
// +build !windows

package core

import (
	"os/exec"
	"syscall"
)

func setDetachAttr(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setsid: true,
	}
}

func dupFD(fd int, target int) {
	syscall.Dup2(fd, target)
}
