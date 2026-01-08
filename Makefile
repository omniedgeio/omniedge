BUILD_ENV ?= "dev"
VERSION ?= $(shell git describe --tags --always --dirty)
LDFLAGS = -X main.Env=${BUILD_ENV} -X github.com/omniedgeio/omniedge-cli/cmd/edgecli/cmd.Version=$(VERSION)

build: go.sum generate
	rm -rf ./out
	GOOS=linux go build -ldflags "${LDFLAGS}" -o out/omniedge cmd/edgecli/main.go

build-darwin: go.sum
	rm -rf ./out
	CGO_ENABLED=1 GOOS=darwin go generate ./...
	CGO_ENABLED=1 GOOS=darwin go build -ldflags "${LDFLAGS}" -o out/omniedge cmd/edgecli/main.go

build-darwin-amd64: go.sum
	rm -rf ./out
	CGO_ENABLED=1 GOOS=darwin GOARCH=amd64 go generate ./...
	CGO_ENABLED=1 GOOS=darwin GOARCH=amd64 go build -ldflags "${LDFLAGS}" -o out/omniedge cmd/edgecli/main.go

build-darwin-arm64: go.sum
	rm -rf ./out
	CGO_ENABLED=1 GOOS=darwin GOARCH=arm64 go generate ./...
	CGO_ENABLED=1 GOOS=darwin GOARCH=arm64 go build -ldflags "${LDFLAGS}" -o out/omniedge cmd/edgecli/main.go

build-riscv64: go.sum generate
	rm -rf ./out
	CGO_ENABLED=1 GOOS=linux GOARCH=riscv64 go generate ./...
	CGO_ENABLED=1 GOOS=linux GOARCH=riscv64 go build -ldflags "${LDFLAGS}" -o out/omniedge cmd/edgecli/main.go

build-freebsd: go.sum
	rm -rf ./out
	CGO_ENABLED=1 GOOS=freebsd go generate ./...
	CGO_ENABLED=1 GOOS=freebsd go build -ldflags "${LDFLAGS}" -o out/omniedge cmd/edgecli/main.go

generate-bindata:
	go get -u github.com/go-bindata/go-bindata/...
	GOOS=linux  go-bindata -pkg edgecli -o bindata.go ./config
	go mod tidy

generate:
	go generate ./...

go.sum: go.mod
	@echo "--> Ensure dependencies have not been modified"
	GO111MODULE=on go mod verify
