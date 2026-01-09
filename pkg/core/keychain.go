package core

import (
	log "github.com/sirupsen/logrus"
	"github.com/zalando/go-keyring"
)

const (
	keyringService = "io.omniedge.cli"
	keyringAccount = "auth_tokens"
)

// SaveSecureToken saves the token to the OS keychain
func SaveSecureToken(token string) error {
	err := keyring.Set(keyringService, keyringAccount, token)
	if err != nil {
		log.Errorf("Failed to save token to keychain: %v", err)
		return err
	}
	return nil
}

// LoadSecureToken loads the token from the OS keychain
func LoadSecureToken() (string, error) {
	token, err := keyring.Get(keyringService, keyringAccount)
	if err != nil {
		if err == keyring.ErrNotFound {
			return "", nil
		}
		log.Errorf("Failed to load token from keychain: %v", err)
		return "", err
	}
	return token, nil
}

// ClearSecureToken removes the token from the OS keychain
func ClearSecureToken() error {
	err := keyring.Delete(keyringService, keyringAccount)
	if err != nil && err != keyring.ErrNotFound {
		log.Errorf("Failed to delete token from keychain: %v", err)
		return err
	}
	return nil
}
