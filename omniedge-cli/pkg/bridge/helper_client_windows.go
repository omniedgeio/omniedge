//go:build windows

package bridge

// Windows uses named pipes for IPC
const HelperSocketPath = `\\.\pipe\omniedge-helper`
