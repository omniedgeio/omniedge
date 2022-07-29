# Omniedge Cli

OmniEdge CLi for macOS (Intel, M1/M2 MacBook), Linux Distributions, and ARM, Raspberry PI, Nvidia Jetson Embedded System. 

>Bring the intranet on the internet

[🤝 Website](https://omniedge.io)
[💬 Twitter](https://twitter.com/omniedgeio)
[😇 Discord](https://discord.gg/d4faRPYj)

Main repo: https://github.com/omniedgeio/omniedge

## Install OmniEdge Cli

```bash
curl https://omniedge.io/install/omniedge-install.sh | bash
```

## Install OmniEdge other clients

-   [Android: OmniEdge.apk](https://omniedge.io/install/download/0.2.2/omniedge-release-v0.2.2.apk)
-   [macOS cli](https://omniedge.io/install/download/0.2.3/omniedgecli-macos-latest.zip)
-   [Windows](https://omniedge.io/install/download/0.2.3/omniedge-setup-0.2.3.exe)
-   [Linux Cli](https://github.com/omniedgeio/app-release/releases/tag/v0.2.3)
-   [iOS & M1 Mac on App Store](https://apps.apple.com/us/app/omniedgenew/id1603005893)
-   [Synology](https://omniedge.io/download/synology)
-   [Raspberry Pi, ARM, Nvidia Jetson](https://github.com/omniedgeio/app-release/releases/tag/v0.2.3)


## Cli Command

### Login

- Login By Password

```shell
omniedge login -u xxx@xxx.com
```

-  Login By Secret-Key

You can generate secret-key on omniedge web.

```shell
omniedge login -s xxxxxx
```

### Join

you can just call `omniedge join`, it will automatically prompt 
the available network for you to choose. And you can 
also add one parameter `-n` to specify the network id manually.

And then, enjoy the omniedge network.

```shell
omniedge join 
// or
omniedge join -n "virtual-network-id" 
```


## Resources

- Architecture: https://omniedge.io/docs/article/architecture
- Install: https://omniedge.io/docs/article/install
- Cases: https://omniedge.io/docs/article/cases
- Compare: https://omniedge.io/docs/article/compare
- Performance: https://omniedge.io/docs/article/performance
- Dashboard: https://omniedge.io/docs/article/admin
- [n2n](https://github.com/ntop/n2n)


## Compile for riscv64

```bash
apt-get update
apt-get install -y openssl autoconf build-essential libssl-dev zip wget g++-riscv64-linux-gnu gcc-riscv64-linux-gnu

wget https://go.dev/dl/go1.18.4.linux-amd64.tar.gz
rm -rf /usr/local/go && tar -C /usr/local -xzf go1.18.4.linux-amd64.tar.gz
export PATH=$PATH:/usr/local/go/bin
go version
export GOOS=linux
export GOARCH=riscv64
export CGO_ENABLED=1
export CC=riscv64-linux-gnu-gcc
git clone https://github.com/yongqianme/omniedge-cli.git
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build
```


## Contributing Guildlines

Check the tempalte into .github folder to report an issue or submit a PR: 
1. ISSUE_TEMPLATE.md 
2. PULL_REQUEST_TEMPLATE.md 

## How to get started? 

1. If you only need a convenient connectivity service 
Just visit https://omniedge.io/download and download the apps for your platform. 

# Contributors
  
@ivyxjc
