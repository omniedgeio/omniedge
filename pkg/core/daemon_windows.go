//go:build windows
// +build windows

package core

import (
	"os/exec"
)

func setDetachAttr(cmd *exec.Cmd) {
	// On Windows, we can use CREATE_NO_WINDOW if we want to be invisible,
	// but the default exec.Command behavior with Start() should be sufficient for backgrounding.
}

func dupFD(fd int, target int) {
	// Systems like Dup2 are not directly available on Windows in the same way.
	// For Windows CLI, we primarily rely on standard Go redirection.
}
