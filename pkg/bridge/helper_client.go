package bridge

import (
	"encoding/json"
	"fmt"
	"net"

	"github.com/omniedgeio/omniedge-cli/pkg/core"
)

const HelperSocketPath = "/var/run/omniedge-helper.sock"

// HelperRequest represents a command to the helper
type HelperRequest struct {
	Command string      `json:"command"`
	Args    interface{} `json:"args"`
}

// HelperResponse represents the helper's response
type HelperResponse struct {
	Success bool   `json:"success"`
	Message string `json:"message"`
	Data    string `json:"data,omitempty"`
}

// HelperClient communicates with the privileged helper
type HelperClient struct {
	socketPath string
}

// NewHelperClient creates a new helper client
func NewHelperClient() *HelperClient {
	return &HelperClient{
		socketPath: HelperSocketPath,
	}
}

// IsAvailable checks if the helper daemon is running
func (c *HelperClient) IsAvailable() bool {
	conn, err := net.Dial("unix", c.socketPath)
	if err != nil {
		return false
	}
	conn.Close()
	return true
}

// Ping tests connection to the helper
func (c *HelperClient) Ping() error {
	resp, err := c.sendCommand("ping", nil)
	if err != nil {
		return err
	}
	if !resp.Success {
		return fmt.Errorf("ping failed: %s", resp.Message)
	}
	return nil
}

// StartVPN starts the VPN via the helper using core.StartOption
func (c *HelperClient) StartVPN(opt *core.StartOption) error {
	resp, err := c.sendCommand("start_vpn", opt)
	if err != nil {
		return err
	}
	if !resp.Success {
		return fmt.Errorf("start_vpn failed: %s", resp.Message)
	}
	return nil
}

// StopVPN stops the VPN via the helper
func (c *HelperClient) StopVPN() error {
	resp, err := c.sendCommand("stop_vpn", nil)
	if err != nil {
		return err
	}
	if !resp.Success {
		return fmt.Errorf("stop_vpn failed: %s", resp.Message)
	}
	return nil
}

// GetStatus returns the VPN status from the helper
func (c *HelperClient) GetStatus() (string, error) {
	resp, err := c.sendCommand("status", nil)
	if err != nil {
		return "", err
	}
	return resp.Message, nil
}

func (c *HelperClient) sendCommand(command string, args interface{}) (*HelperResponse, error) {
	conn, err := net.Dial("unix", c.socketPath)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to helper: %w", err)
	}
	defer conn.Close()

	req := HelperRequest{
		Command: command,
		Args:    args,
	}

	encoder := json.NewEncoder(conn)
	if err := encoder.Encode(req); err != nil {
		return nil, fmt.Errorf("failed to send request: %w", err)
	}

	var resp HelperResponse
	decoder := json.NewDecoder(conn)
	if err := decoder.Decode(&resp); err != nil {
		return nil, fmt.Errorf("failed to read response: %w", err)
	}

	return &resp, nil
}
