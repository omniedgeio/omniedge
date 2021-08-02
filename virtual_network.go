package edgecli

import (
	"encoding/json"
	"errors"
	"fmt"
	log "github.com/sirupsen/logrus"
	"net/http"
)

type VirtualNetwork struct {
	UUID string `json:"uuid"`
	Name string `json:"name"`
	OS   string `json:"os"`
}

type JoinOption struct {
	VirtualNetworkId string
	DeviceId         string
}

type VirtualNetworkService struct {
	HttpOption
}

func (s *VirtualNetworkService) List() ([]VirtualNetworkResponse, error) {
	var url = s.BaseUrl + "/virtual-networks"
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("content-type", "application/json")
	req.Header.Set("Authorization", s.Token)
	resp, _ := HandleCall(req)
	log.Infof("List Virtual Network response %+v", resp)
	switch resp.(type) {
	case *SuccessResponse:
		vnJson, _ := json.Marshal(resp.(*SuccessResponse).Data)
		var vnResp []VirtualNetworkResponse
		if err := json.Unmarshal(vnJson, &vnResp); err != nil {
			return nil, errors.New(fmt.Sprintf("Fail to unmarshal response's data ,err is %+v", err))
		}
		return vnResp, nil
	case *ErrorResponse:
		return nil, errors.New(fmt.Sprintf("Fail to list user's virtual network, error message: %s", resp.(*ErrorResponse).Message))
	default:
		return nil, errors.New(fmt.Sprint("This client has some unpredictable problems, please contact the omniedge team."))
	}

}

func (s *VirtualNetworkService) Join(opt *JoinOption) (*JoinVirtualNetworkResponse, error) {
	var url = fmt.Sprintf(s.BaseUrl+"/virtual-networks/%s/devices/%s/join", opt.VirtualNetworkId, opt.DeviceId)
	req, _ := http.NewRequest("POST", url, nil)
	req.Header.Set("content-type", "application/json")
	req.Header.Set("Authorization", s.Token)
	resp, _ := HandleCall(req)
	log.Tracef("JoinVitualNetwork response %+v", resp)
	switch resp.(type) {
	case *SuccessResponse:
		joinVNJson, _ := json.Marshal(resp.(*SuccessResponse).Data)
		joinVNResp := JoinVirtualNetworkResponse{}
		if err := json.Unmarshal(joinVNJson, &joinVNResp); err != nil {
			return nil, errors.New(fmt.Sprintf("Fail to unmarshal response's data ,err is %+v", err))
		}
		return &joinVNResp, nil
	case *ErrorResponse:
		return nil, errors.New(fmt.Sprintf("Fail to join, error message: %s", resp.(*ErrorResponse).Message))
	default:
		return nil, errors.New(fmt.Sprint("This client has some unpredictable problems, please contact the omniedge team."))
	}
}
