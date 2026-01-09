package api

import (
	"crypto/sha256"
	"encoding/base64"
	"math/rand"
	"strings"
	"time"
)

type PKCEInfo struct {
	Verifier  string
	Challenge string
	State     string
}

func init() {
	rand.Seed(time.Now().UnixNano())
}

func GeneratePKCE() (*PKCEInfo, error) {
	// 1. Generate State
	state := randomString(32)

	// 2. Generate Verifier
	verifier := randomString(64)

	// 3. Generate Challenge (S256)
	hash := sha256.Sum256([]byte(verifier))
	challenge := base64.RawURLEncoding.EncodeToString(hash[:])
	challenge = strings.TrimRight(challenge, "=")

	return &PKCEInfo{
		Verifier:  verifier,
		Challenge: challenge,
		State:     state,
	}, nil
}

func randomString(n int) string {
	const charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-._~"
	b := make([]byte, n)
	for i := range b {
		b[i] = charset[rand.Intn(len(charset))]
	}
	return string(b)
}
