package api

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"

	"github.com/gorilla/websocket"
	log "github.com/sirupsen/logrus"
)

// SessionResponse represents the response from generating a login session
type SessionResponse struct {
	ID        string    `json:"id"`
	AuthURL   string    `json:"auth_url"`
	ExpiredAt time.Time `json:"expired_at"`
}

// WebSocketTokenResponse represents tokens received via WebSocket
type WebSocketTokenResponse struct {
	Token        string `json:"token"`
	RefreshToken string `json:"refresh_token"`
}

// SessionService handles session-based QR login operations
type SessionService struct {
	HttpOption
}

// GenerateSession creates a new session for QR code login
// The session ID can be used to:
// 1. Display a QR code containing the auth_url
// 2. Connect to WebSocket to receive tokens when authenticated
func (s *SessionService) GenerateSession() (*SessionResponse, error) {
	url := s.BaseUrl + "/auth/login/session"
	req, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("content-type", "application/json")

	resp, err := HandleCall(req)
	if err != nil {
		return nil, err
	}

	log.Tracef("GenerateSession response %+v", resp)

	switch resp := resp.(type) {
	case *SuccessResponse:
		sessionJson, _ := json.Marshal(resp.Data)
		session := SessionResponse{}
		if err := json.Unmarshal(sessionJson, &session); err != nil {
			return nil, fmt.Errorf("failed to unmarshal session response: %w", err)
		}
		log.Debugf("Generated session: %+v", session)
		return &session, nil
	case *ErrorResponse:
		return nil, fmt.Errorf("failed to generate session: %s", resp.Message)
	default:
		return nil, errors.New("unexpected response type from session API")
	}
}

// NotifySession notifies a session with the current user's token
// This is called by the mobile app after scanning QR and logging in
func (s *SessionService) NotifySession(authSessionUUID string) error {
	url := s.BaseUrl + "/auth/login/session/notify"
	body := map[string]string{
		"auth_session_uuid": authSessionUUID,
	}
	postBody, _ := json.Marshal(body)

	req, err := http.NewRequest("POST", url, strings.NewReader(string(postBody)))
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}
	req.Header.Set("content-type", "application/json")
	req.Header.Set("Authorization", s.Token)

	resp, err := HandleCall(req)
	if err != nil {
		return err
	}

	log.Tracef("NotifySession response %+v", resp)

	switch resp := resp.(type) {
	case *SuccessResponse:
		return nil
	case *ErrorResponse:
		return fmt.Errorf("failed to notify session: %s", resp.Message)
	default:
		return errors.New("unexpected response type from notify API")
	}
}

// ConnectAndWaitForToken connects to the WebSocket and waits for authentication tokens
// This blocks until tokens are received, connection closes, or context is cancelled
// Returns the tokens when the mobile app authenticates, or an error
func (s *SessionService) ConnectAndWaitForToken(sessionID string, timeoutSeconds int) (*WebSocketTokenResponse, error) {
	// Convert HTTP URL to WebSocket URL
	wsURL := s.getWebSocketURL(sessionID)
	log.Infof("Connecting to WebSocket: %s", wsURL)

	// Set up WebSocket dialer with timeout
	dialer := websocket.Dialer{
		HandshakeTimeout: 10 * time.Second,
	}

	conn, _, err := dialer.Dial(wsURL, nil)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to WebSocket: %w", err)
	}
	defer conn.Close()

	// Set read deadline
	if timeoutSeconds > 0 {
		conn.SetReadDeadline(time.Now().Add(time.Duration(timeoutSeconds) * time.Second))
	}

	// Wait for token message
	for {
		_, message, err := conn.ReadMessage()
		if err != nil {
			if websocket.IsCloseError(err, websocket.CloseNormalClosure) {
				return nil, errors.New("WebSocket closed without receiving tokens")
			}
			return nil, fmt.Errorf("WebSocket read error: %w", err)
		}

		log.Debugf("Received WebSocket message: %s", string(message))

		// Parse token response
		var tokenResp WebSocketTokenResponse
		if err := json.Unmarshal(message, &tokenResp); err != nil {
			log.Warnf("Failed to parse WebSocket message as token: %v", err)
			continue
		}

		// Validate we got actual tokens
		if tokenResp.Token != "" && tokenResp.RefreshToken != "" {
			log.Info("Received authentication tokens via WebSocket")
			return &tokenResp, nil
		}
	}
}

// getWebSocketURL converts the base HTTP URL to a WebSocket URL for session connection
func (s *SessionService) getWebSocketURL(sessionID string) string {
	// Parse the base URL
	baseURL := s.BaseUrl

	// Replace http:// with ws:// and https:// with wss://
	if strings.HasPrefix(baseURL, "https://") {
		baseURL = "wss://" + strings.TrimPrefix(baseURL, "https://")
	} else if strings.HasPrefix(baseURL, "http://") {
		baseURL = "ws://" + strings.TrimPrefix(baseURL, "http://")
	}

	// Remove /api/v1 or /api/v2 suffix if present
	parsed, err := url.Parse(baseURL)
	if err == nil {
		// The WebSocket endpoint is at /login/session/{id} at the root
		parsed.Path = "/login/session/" + sessionID
		return parsed.String()
	}

	// Fallback: simple string replacement
	return baseURL + "/login/session/" + sessionID
}
