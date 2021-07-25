package edgecli

import (
	"encoding/json"
	log "github.com/sirupsen/logrus"
	"io/ioutil"
	"net/http"
)

type SuccessResponse struct {
	Code    int         `json:"-"`
	Message string      `json:"message"`
	Data    interface{} `json:"data"`
}

type ErrorResponse struct {
	Code    int         `json:"-"`
	Message string      `json:"message"`
	Errors  interface{} `json:"errors"`
}

func HandleCall(req *http.Request) (error, interface{}) {
	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		log.Errorf("Fail to call backend API [%s]. err is %s\n", req.URL, err.Error())
		return err, nil
	}
	return handle(resp)
}

func handle(resp *http.Response) (error, interface{}) {
	body, code := handleResp(resp)
	if code == 200 {
		successResponse := &SuccessResponse{}
		if err := handleSuccessResp(body, successResponse, resp.Request.URL.String()); err != nil {
			return err, nil
		} else {
			return nil, successResponse
		}
	} else {
		errorResponse := &ErrorResponse{}
		if err := handleErrorResp(body, errorResponse, resp.Request.URL.String()); err != nil {
			return err, nil
		} else {
			return nil, errorResponse
		}
	}
}

func handleResp(resp *http.Response) ([]byte, int) {
	if resp == nil {
		log.Errorf("Fail to call backend API")
		return nil, -1
	}
	if resp.Body == nil {
		return nil, resp.StatusCode
	}
	body, err := ioutil.ReadAll(resp.Body)
	if resp.StatusCode != 200 {
		return body, resp.StatusCode
	}
	if err != nil {
		log.Errorf("Fail to read response of backend API [%s]\n", resp.Request.URL)
		return nil, -1
	}
	return body, 200
}

func handleSuccessResp(body []byte, v *SuccessResponse, url string) error {
	err := json.Unmarshal(body, v)
	if err != nil {
		log.Errorf("Fail to parse the response of API [%s]. Response is %s\n", url, body)
		return err
	}
	return nil
}

func handleErrorResp(body []byte, v *ErrorResponse, url string) error {
	err := json.Unmarshal(body, v)
	if err != nil {
		log.Errorf("Fail to parse the response of API [%s]. Response is %s\n", url, body)
		return err
	}
	return nil
}
