package edgecli

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	log "github.com/sirupsen/logrus"
	"net/http"
)

type RegisterOption struct {
	Token        string
	BaseUrl      string
	Name         string
	HardwareUUID string
	OS           string
}

type RegisterService struct {
	RegisterOption
}

func (s *RegisterService) Register() (*DeviceResponse, error) {
	var url string
	var body map[string]string
	url = s.BaseUrl + "/devices/register"

	body = map[string]string{
		"name":          s.Name,
		"hardware_uuid": s.HardwareUUID,
		"os":            s.OS,
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	req.Header.Set("Authorization", s.Token)
	resp, _ := HandleCall(req)
	log.Tracef("Register response %+v", resp)
	switch resp.(type) {
	case *SuccessResponse:
		deviceJson, _ := json.Marshal(resp.(*SuccessResponse).Data)
		device := DeviceResponse{}
		if err := json.Unmarshal(deviceJson, &device); err != nil {
			return nil, errors.New(fmt.Sprintf("Fail to unmarshal response's data ,err is %+v", err))
		}
		log.Debugf("Registerdevice result is %+v", device)
		return &device, nil
	case *ErrorResponse:
		return nil, errors.New(fmt.Sprintf("Fail to register device, error message: %s", resp.(*ErrorResponse).Message))
	default:
		return nil, errors.New(fmt.Sprint("This client has some unpredictable problems, please contact the omniedge team."))
	}
}
