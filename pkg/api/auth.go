package api

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	log "github.com/sirupsen/logrus"
	"net/http"
)

type AuthResp struct {
	Token        string `json:"token"`
	RefreshToken string `json:"refreshToken"`
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
