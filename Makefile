generate:
	go generate ./...

build:
	rm -rf ./out
	GOOS=linux  go  build -o out/omniedge cmd/edgecli/main.go

build-darwin:
	rm -rf ./out
	go  build -o out/omniedge cmd/edgecli/main.go


generate-bindata:
	go get -u github.com/go-bindata/go-bindata/...
	go-bindata -pkg edgecli -o bindata.go ./config

