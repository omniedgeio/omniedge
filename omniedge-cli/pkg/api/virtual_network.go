package api

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"

	log "github.com/sirupsen/logrus"
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

type UploadOption struct {
	IP          string
	MacAddress  string
	SubnetMask  string
	DeviceId    string
	ScanResults []*ScanResult
}

type VirtualNetworkService struct {
	HttpOption
}

type RegisterDeviceSubnetRouteRequest struct {
	IP         string                      `json:"ip" validate:"required,ipv4"`
	MacAddr    string                      `json:"mac_addr" validate:"required,mac"`
	SubnetMask string                      `json:"subnet_mask" validate:"required,ipv4"`
	Devices    []*SubnetRouteDeviceRequest `json:"devices"`
}

type SubnetRouteDeviceRequest struct {
	Name    string `json:"name"`
	IP      string `json:"ip" validate:"required,ipv4"`
	MacAddr string `json:"mac_addr" validate:"required,mac"`
}

func (s *VirtualNetworkService) List() ([]VirtualNetworkResponse, error) {
	var url = s.BaseUrl + "/virtual-networks/all/list"
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
	var url = fmt.Sprintf(s.BaseUrl+"/virtual-networks/%s/devices/%s", opt.VirtualNetworkId, opt.DeviceId)
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

func (s *VirtualNetworkService) Upload(opt *UploadOption) error {
	var url = fmt.Sprintf(s.BaseUrl+"/devices/%s/subnets", opt.DeviceId)
	body := map[string]interface{}{
		"ip":          opt.IP,
		"mac_addr":    opt.MacAddress,
		"subnet_mask": opt.SubnetMask,
		"devices":     make([]map[string]string, 0),
	}
	for _, scan := range opt.ScanResults {
		if scan.MacAddress == "" {
			continue
		}
		d := map[string]string{
			"name":     scan.HostName,
			"ip":       scan.IPv4,
			"mac_addr": scan.MacAddress,
		}
		body["devices"] = append(body["devices"].([]map[string]string), d)
	}
	postBody, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", url, bytes.NewBuffer(postBody))
	req.Header.Set("content-type", "application/json")
	req.Header.Set("Authorization", s.Token)
	resp, _ := HandleCall(req)
	log.Tracef("Upload Subnet response %+v", resp)
	switch resp.(type) {
	case *SuccessResponse:
		return nil
	case *ErrorResponse:
		return errors.New(fmt.Sprintf("Fail to upload, error message: %s", resp.(*ErrorResponse).Message))
	default:
		return errors.New(fmt.Sprint("This client has some unpredictable problems, please contact the omniedge team."))
	}
}
func (s *VirtualNetworkService) GetDevices(networkID string) ([]VirtualNetworkDeviceResponse, error) {
	url := fmt.Sprintf("%s/virtual-networks/%s/devices", s.BaseUrl, networkID)
	log.Infof("GetDevices: Fetching devices from %s", url)
	req, _ := http.NewRequest("GET", url, nil)
	req.Header.Set("content-type", "application/json")
	req.Header.Set("Authorization", s.Token)
	resp, _ := HandleCall(req)
	log.Infof("GetDevices: Response type %T", resp)
	switch resp.(type) {
	case *SuccessResponse:
		dataJson, _ := json.Marshal(resp.(*SuccessResponse).Data)
		log.Infof("GetDevices: Raw data JSON: %s", string(dataJson))

		// API returns {"data": [...devices...], "meta": {...}}
		// The SuccessResponse.Data already contains the inner structure
		// Try parsing as wrapper with "data" field
		var wrapper struct {
			Data []VirtualNetworkDeviceResponse `json:"data"`
			Meta interface{}                    `json:"meta"`
		}
		if err := json.Unmarshal(dataJson, &wrapper); err == nil && len(wrapper.Data) > 0 {
			log.Infof("GetDevices: Parsed %d devices from 'data' field", len(wrapper.Data))
			return wrapper.Data, nil
		}

		// Fallback: try parsing as direct array
		var devices []VirtualNetworkDeviceResponse
		if err := json.Unmarshal(dataJson, &devices); err == nil {
			log.Infof("GetDevices: Parsed %d devices from array", len(devices))
			return devices, nil
		}

		// Final fallback: return empty list
		log.Warn("GetDevices: Could not parse devices, returning empty list")
		return []VirtualNetworkDeviceResponse{}, nil
	case *ErrorResponse:
		return nil, errors.New(fmt.Sprintf("Fail to get devices, error message: %s", resp.(*ErrorResponse).Message))
	default:
		return nil, errors.New(fmt.Sprint("Internal error during devices fetch"))
	}
}
