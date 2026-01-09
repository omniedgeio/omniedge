package api

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"

	log "github.com/sirupsen/logrus"
)

type AuthResp struct {
	Token        string `json:"token"`
	RefreshToken string `json:"refreshToken"`
	AccessToken  string `json:"access_token"`
	IdToken      string `json:"id_token"`
	ExpiresIn    int    `json:"expires_in"`
}

type DeviceCodeResp struct {
	DeviceCode              string `json:"device_code"`
	UserCode                string `json:"user_code"`
	VerificationUri         string `json:"verification_uri"`
	VerificationUriComplete string `json:"verification_uri_complete"`
	ExpiresIn               int    `json:"expires_in"`
	Interval                int    `json:"interval"`
}

type AuthMethod string

const (
	LoginBySecretKey AuthMethod = "LoginBySecretKey"
	LoginByPassword  AuthMethod = "LoginByPassword"
)

type AuthOption struct {
	Username   string
	Password   string
	SecretKey  string
	AuthMethod AuthMethod
}

type RefreshTokenOption struct {
	RefreshToken string
}

type AuthService struct {
	HttpOption
}

func (s *AuthService) Login(opt *AuthOption) (*AuthResp, error) {
	var url string
	var body map[string]string
	if opt.AuthMethod == LoginByPassword {
		url = s.BaseUrl + "/auth/login/password"
		body = map[string]string{
			"email":    opt.Username,
			"password": opt.Password,
		}
	}
	if opt.AuthMethod == LoginBySecretKey {
		url = s.BaseUrl + "/auth/login/security-key"
		body = map[string]string{
			"key": opt.SecretKey,
		}
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	resp, _ := HandleCall(req)
	log.Tracef("LoginByPassword response %+v", resp)
	switch resp.(type) {
	case *SuccessResponse:
		authJson, _ := json.Marshal(resp.(*SuccessResponse).Data)
		auth := AuthResp{}
		if err := json.Unmarshal(authJson, &auth); err != nil {
			return nil, errors.New(fmt.Sprintf("Fail to unmarshal response's data ,err is %+v", err))
		}
		log.Debugf("auth token is %+v", auth)
		return &auth, nil
	case *ErrorResponse:
		return nil, errors.New(fmt.Sprintf("Fail to login, error message: %s", resp.(*ErrorResponse).Message))
	default:
		return nil, errors.New(fmt.Sprint("This client has some unpredictable problems, please contact the omniedge team."))
	}
}

func (s *AuthService) Refresh(opt *RefreshTokenOption) (*AuthResp, error) {
	var url string
	var body map[string]string
	url = s.BaseUrl + "/auth/refresh"
	body = map[string]string{
		"refresh_token": opt.RefreshToken,
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	resp, _ := HandleCall(req)
	log.Tracef("RefreshToken response %+v", resp)
	switch resp.(type) {
	case *SuccessResponse:
		authJson, _ := json.Marshal(resp.(*SuccessResponse).Data)
		auth := AuthResp{}
		if err := json.Unmarshal(authJson, &auth); err != nil {
			return nil, errors.New(fmt.Sprintf("Fail to unmarshal response's data ,err is %+v", err))
		}
		log.Debugf("auth token is %+v", auth)
		return &auth, nil
	case *ErrorResponse:
		return nil, errors.New(fmt.Sprintf("Fail to login, error message: %s", resp.(*ErrorResponse).Message))
	default:
		return nil, errors.New(fmt.Sprint("This client has some unpredictable problems, please contact the omniedge team."))
	}
}
func (s *AuthService) Me() (*ProfileResponse, error) {
	url := s.BaseUrl + "/profile"
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("content-type", "application/json")
	req.Header.Set("Authorization", s.Token)
	resp, _ := HandleCall(req)
	switch resp := resp.(type) {
	case *SuccessResponse:
		profileJson, _ := json.Marshal(resp.Data)
		profile := ProfileResponse{}
		if err := json.Unmarshal(profileJson, &profile); err != nil {
			return nil, fmt.Errorf("Fail to unmarshal response's data ,err is %+v", err)
		}
		return &profile, nil
	case *ErrorResponse:
		return nil, fmt.Errorf("Fail to get profile, error message: %s", resp.Message)
	default:
		return nil, fmt.Errorf("Internal error during profile fetch")
	}
}

func (s *AuthService) DeviceFlowInit(clientId string, scope string) (*DeviceCodeResp, error) {
	url := s.BaseUrl + "/oauth/device/code"
	body := map[string]string{
		"client_id": clientId,
		"scope":     scope,
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	resp, _ := HandleCall(req)

	switch resp := resp.(type) {
	case *SuccessResponse:
		dataJson, _ := json.Marshal(resp.Data)
		deviceCode := DeviceCodeResp{}
		if err := json.Unmarshal(dataJson, &deviceCode); err != nil {
			return nil, err
		}
		return &deviceCode, nil
	case *ErrorResponse:
		return nil, errors.New(resp.Error())
	default:
		return nil, errors.New("unexpected response")
	}
}

func (s *AuthService) DeviceFlowToken(clientId string, deviceCode string) (*AuthResp, error) {
	url := s.BaseUrl + "/oauth/token"
	body := map[string]string{
		"client_id":   clientId,
		"device_code": deviceCode,
		"grant_type":  "urn:ietf:params:oauth:grant-type:device_code",
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	resp, _ := HandleCall(req)

	switch resp := resp.(type) {
	case *SuccessResponse:
		authJson, _ := json.Marshal(resp.Data)
		auth := AuthResp{}
		if err := json.Unmarshal(authJson, &auth); err != nil {
			return nil, err
		}
		// Bridge legacy Token field if necessary
		if auth.Token == "" && auth.AccessToken != "" {
			auth.Token = auth.AccessToken
		}
		return &auth, nil
	case *ErrorResponse:
		return nil, errors.New(resp.Error())
	default:
		return nil, errors.New("unexpected response")
	}
}

func (s *AuthService) GetTokenByAuthCode(clientId string, code string, verifier string, redirectUri string) (*AuthResp, error) {
	url := s.BaseUrl + "/oauth/token"
	body := map[string]string{
		"client_id":     clientId,
		"code":          code,
		"code_verifier": verifier,
		"redirect_uri":  redirectUri,
		"grant_type":    "authorization_code",
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	resp, _ := HandleCall(req)

	switch resp := resp.(type) {
	case *SuccessResponse:
		authJson, _ := json.Marshal(resp.Data)
		auth := AuthResp{}
		if err := json.Unmarshal(authJson, &auth); err != nil {
			return nil, err
		}
		// Bridge legacy Token field if necessary
		if auth.Token == "" && auth.AccessToken != "" {
			auth.Token = auth.AccessToken
		}
		return &auth, nil
	case *ErrorResponse:
		return nil, errors.New(resp.Error())
	default:
		return nil, errors.New("unexpected response")
	}
}
