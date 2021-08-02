package edgecli

import (
	"encoding/json"
	log "github.com/sirupsen/logrus"
	"io/ioutil"
	"net/http"
)

func HandleCall(req *http.Request) (interface{}, error) {
	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		log.Errorf("Fail to call backend API [%s]. err is %s\n", req.URL, err.Error())
		return nil, err
	}
	return handle(resp)
}

func handle(resp *http.Response) (interface{}, error) {
	body, code := handleResp(resp)
	if code == 200 {
		successResponse := &SuccessResponse{}
		if err := handleSuccessResp(body, successResponse, resp.Request.URL.String()); err != nil {
			return nil, err
		} else {
			return successResponse, nil
		}
	} else {
		errorResponse := &ErrorResponse{}
		if err := handleErrorResp(body, errorResponse, resp.Request.URL.String()); err != nil {
			return nil, err
		} else {
			return errorResponse, nil
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
