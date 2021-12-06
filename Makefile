build: go.sum generate
	rm -rf ./out
	GOOS=linux go build -o out/omniedge cmd/edgecli/main.go

build-darwin: go.sum generate
	rm -rf ./out
	GOOS=darwin go build -o out/omniedge cmd/edgecli/main.go

generate-bindata:
	go get -u github.com/go-bindata/go-bindata/...
	GOOS=linux  go-bindata -pkg omniedge -o bindata.go ./config

generate:
	go generate ./...

go.sum: go.mod
	@echo "--> Ensure dependencies have not been modified"
	GO111MODULE=on go mod verify
