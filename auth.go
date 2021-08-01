package edgecli

import (
	"bytes"
	"encoding/json"
	log "github.com/sirupsen/logrus"
	"net/http"
)

type AuthResp struct {
	Token string `json:"token"`
}

type AuthMethod string

const (
	LoginBySecretKey AuthMethod = "LoginBySecretKey"
	LoginByPassword  AuthMethod = "LoginByPassword"
)

type AuthOption struct {
	BaseUrl    string
	Username   string
	Password   string
	SecretKey  string
	AuthMethod AuthMethod
}

type AuthService struct {
	AuthOption
}

func (s *AuthService) Login() {
	var url string
	var body map[string]string
	if s.AuthMethod == LoginByPassword {
		url = s.BaseUrl + "/auth/login/password"
		body = map[string]string{
			"email":    s.Username,
			"password": s.Password,
		}
	}
	if s.AuthMethod == LoginBySecretKey {
		url = s.BaseUrl + "/auth/login/security-key"
		body = map[string]string{
			"key": s.SecretKey,
		}
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	_, resp := HandleCall(req)
	log.Tracef("LoginByPassword response %+v", resp)
	switch resp.(type) {
	case *SuccessResponse:
		authJson, _ := json.Marshal(resp.(*SuccessResponse).Data)
		auth := AuthResp{}
		if err := json.Unmarshal(authJson, &auth); err != nil {
			log.Errorf("Fail to unmarshal response's data ,err is %+v", err)
		}
		log.Debugf("auth token is %+v", auth)
		log.Infof("successful to login")
	case *ErrorResponse:
		log.Errorf("Fail to login, error message: %s", resp.(*ErrorResponse).Message)
	default:
		log.Error("This client has some unpredictable problems, please contact the omniedge team.")
	}
}
