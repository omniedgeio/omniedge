//go:build !windows
// +build !windows

package core

import (
	"os/exec"
	"syscall"

	"golang.org/x/sys/unix"
)

func setDetachAttr(cmd *exec.Cmd) {
	cmd.SysProcAttr = &syscall.SysProcAttr{
		Setsid: true,
	}
}

func dupFD(fd int, target int) {
	unix.Dup2(fd, target)
}
