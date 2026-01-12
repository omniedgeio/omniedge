package core

import (
	"strings"

	log "github.com/sirupsen/logrus"
	"github.com/zalando/go-keyring"
)

const (
	keyringService = "io.omniedge.cli"
	keyringAccount = "auth_tokens"
)

func SaveSecureToken(token string) error {
	err := keyring.Set(keyringService, keyringAccount, token)
	if err != nil {
		if strings.Contains(err.Error(), "org.freedesktop.secrets") || strings.Contains(err.Error(), "dbus-launch") {
			log.Debugf("Secret service not available, skipping keychain save (will fallback to file): %v", err)
			return nil
		}
		log.Errorf("Failed to save token to keychain: %v", err)
		return err
	}
	return nil
}

func LoadSecureToken() (string, error) {
	token, err := keyring.Get(keyringService, keyringAccount)
	if err != nil {
		if err == keyring.ErrNotFound {
			return "", nil
		}
		if strings.Contains(err.Error(), "org.freedesktop.secrets") || strings.Contains(err.Error(), "dbus-launch") {
			log.Debugf("Secret service not available, skipping keychain load: %v", err)
			return "", nil
		}
		return "", err
	}
	return token, nil
}

func ClearSecureToken() error {
	err := keyring.Delete(keyringService, keyringAccount)
	if err != nil && err != keyring.ErrNotFound {
		if strings.Contains(err.Error(), "org.freedesktop.secrets") || strings.Contains(err.Error(), "dbus-launch") {
			log.Debugf("Secret service not available, skipping keychain delete: %v", err)
			return nil
		}
		log.Errorf("Failed to delete token from keychain: %v", err)
		return err
	}
	return nil
}
