build:
	rm -rf ./out
	GOOS=linux  go  build -o out/omniedge cmd/edgecli/main.go

generate-bindata:
	go get -u github.com/go-bindata/go-bindata/...
	GOOS=linux  go-bindata -pkg edgecli -o bindata.go ./config

generate:
	go generate ./...

