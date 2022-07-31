BUILD_ENV ?= "dev"

build: go.sum generate
	rm -rf ./out
	GOOS=linux go build -ldflags "-X main.Env=${BUILD_ENV}" -o out/omniedge cmd/edgecli/main.go

build-darwin: go.sum generate
	rm -rf ./out
	GOOS=darwin go build -ldflags "-X main.Env=${BUILD_ENV}" -o out/omniedge cmd/edgecli/main.go


build-riscv64: go.sum generate
	rm -rf ./out
	GOOS=linux go build -ldflags "-X main.Env=${BUILD_ENV}" -o out/omniedge cmd/edgecli/main.go

build-freebsd: go.sum generate
	rm -rf ./out
	GOOS=freebsd go build -ldflags "-X main.Env=${BUILD_ENV}" -o out/omniedge cmd/edgecli/main.go

generate-bindata:
	go get -u github.com/go-bindata/go-bindata/...
	GOOS=linux  go-bindata -pkg edgecli -o bindata.go ./config
	go mod tidy

generate:
	go generate ./...

go.sum: go.mod
	@echo "--> Ensure dependencies have not been modified"
	GO111MODULE=on go mod verify
