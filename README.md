<p align="center">
<h1 align="center"> OmniEdge </h1>
<p align="center">What happens in intranet, stays in intranet.</p>
</p>

<p align="center">
<a href="https://omniedge.io">
<img alt="Website" src="https://img.shields.io/website?label=omniedge.io&url=https%3A%2F%2Fomniedge.io">
</a>
<a href="https://github.com/omniedgeio/omniedge">
<img src="https://img.shields.io/github/license/omniedgeio/omniedge">
</a>
<a href="https://github.com/omniedgeio/omniedge">
<img src="https://img.shields.io/github/downloads/omniedgeio/app-release/total">
</a>

<a href="https://twitter.com/intent/follow?screen_name=omniedgeio">
<img src="https://img.shields.io/twitter/follow/omniedgeio?label=follows&style=social" />
</a>
  <a href="https://github.com/omniedgeio/omniedge-cli">
    <img src="https://img.shields.io/github/languages/top/omniedgeio/omniedge-cli" />
  </a> 
    <a href="https://github.com/omniedgeio/omniedge-iOS">
    <img src="https://img.shields.io/github/languages/top/omniedgeio/omniedge-iOS" />
  </a>
      <a href="https://github.com/omniedgeio/omniedge-macOS">
    <img src="https://img.shields.io/github/languages/top/omniedgeio/omniedge-macOS" />
  </a> 
      <a href="https://github.com/omniedgeio/omniedge-windows">
    <img src="https://img.shields.io/github/languages/top/omniedgeio/omniedge-windows" />
  </a> 
        <a href="https://github.com/omniedgeio/omniedge-android">
<img src="https://img.shields.io/github/languages/top/omniedgeio/omniedge-android"
</a>
          <a href="https://github.com/omniedgeio/omniedge-synology">
    <img src="https://img.shields.io/github/languages/top/omniedgeio/omniedge-synology" />
  </a> 
</p>

[【English】](README.md) [【繁体中文】](README/README-zh-Hant.md) [【简体中文】](README/README-zh-Hans.md) [【日本语】](README/README-JP.md) [【Español】](README/README-ES.md) [【Italiano】](README/README-IT.md) [【한국어】](README/README-KR.md) 
[【العربي】](README/README-AR.md) [【Tiếng Việt】](README/README-VN.md) [【แบบไทย】](README/README-TH.md)

OmniEdge is an Open source p2p layer 2 VPN infrastructure based on [n2n](https://github.com/ntop/n2n) protocol, a traditional VPN alternative. No central server, easy to scale with less maintenance. What happens in intranet, stays in in intranet. 
   
![OmniEdge-clients](OmniEdge-clients.png)

## Key features:

||||
|----|----|----|
|Dashboard administration management| :fire: Mesh VPNs|Desktop GUI apps for MacOS(menubar) and Windows(systray)|
| :fire: Multi virtual networks| :fire: Site-to-Site VPNs|Command line cli apps for Linux,FreeBSD, Raspbian and MacOS|
|Multi users|Unlimited data transfer|Command line cli apps for armv7,arm64,RISC-V64,x86_64 and amd64|
|Multi devices|Encrypted peer-to-peer connection|Mobile apps for iOS and Android|
| :fire: Self-hosted Supernode |Encrypted connection relay|Tablet apps for iPad, Android Tablet and Android TV|
| :fire: Sharing virtual network|Hybrid-cloud support|NAS App for Synology|
|Security Keys| :fire: Zero-Config|Automatic public supernode allocation|
| :fire: [Remote Device Control](https://omniedge.io/docs/article/Cases/VNC)|[Drop Files remotely](https://omniedge.io/docs/article/Cases/landrop) |Automatic IP allocation|


You can find more features in the [Pricing](https://omniedge.io/pricing) Page for Enterprise.

## Get Started in 5 minutes 

1. Sign up your account: [Sign up](https://omniedge.io/register)
2. [Download](https://omniedge.io/download) OmniEdge apps for your platform 
3. Or run the following command if you want to use cli version:
```bash
curl https://omniedge.io/install/omniedge-install.sh | bash
```
4. Login with your Email and password, select your virutal network, connect! 

You are all set! 

And if you want to login with **security key**, or manage your devices, go and check [Documenation](https://omniedge.io/docs) for more.

## Compile

### OmniEdge Cli

1. Environment: Golang 1.16.6
2. Compile: 

- 2.1. Ubuntu /linux

```bash
sudo -E apt-get -y update
sudo -E apt-get install -y openssl
sudo -E apt-get install -y build-essential
sudo -E apt-get install -y libssl-dev
sudo -E apt-get install -y zip
git clone https://github.com/omniedgeio/omniedge-cli
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build
```

- 2.2. macOS

```bash
brew install autoconf automake libtool
git clone https://github.com/omniedgeio/omniedge-cli
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build-darwin
```

- 2.3. freebsd

```bash
su
pkg update && pkg install go gmake git openssl zip autoconf automake libtool
git clone https://github.com/omniedgeio/omniedge-cli
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build-freebsd
```

3. Cross Compile

- 3.1 RISC-V 

Host OS: Ubuntu 20.04

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
git clone https://github.com/omniedgeio/omniedge-cli.git
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build-riscv64
```

The compiled omniedge-cli will be found in **/out/**

### OmniEdge Android

1. Download Android Studio: https://developer.android.com/studio
2. Get the repo and compile

```bash
git clone https://github.com/omniedgeio/omniedge-android.git`
./gradlew test --stacktrace
./gradlew assembleDebug --stacktrace
```

We have also prepared the CI for Github and Gitlab for building automatically. 

1. Github: https://github.com/omniedgeio/omniedge-android/blob/main/.github/workflows/build.yml
2. GitLab: https://github.com/omniedgeio/omniedge-android/blob/main/.gitlab-ci.yml


### OmniEdge iOS

1. Download and install Xcode
2. Get the repo and compile

```bash
git clone https://github.com/omniedgeio/omniedge-iOS.git
cd omniedge-iOS
open OmniEdgeNew/OmniEdgeNew.xcworkspace
```

Xcode will open automatically, you have to set your developer account to start the compile. We recommend compiling the package on your devices separately, specially the **Tunnel** package. 

<img width="902" alt="image" src="https://user-images.githubusercontent.com/93888/180374544-0ae0fbd8-3413-427f-8e9b-ec0c49249f0e.png">

### OmniEdge-macOS

1. Download and install Xcode
2. Get the repo and compile

```bash
git clone https://github.com/omniedgeio/omniedge-macOS.git
cd omniedge-macOS
open Omniedge.xcodeproj
```

Xcode will open automatically, you have to set your developer account to start the compile.

### OmniEdge-windows

1. Download and install QT
2. Get the repo and compile

```bash
git clone https://github.com/omniedgeio/omniedge-windows.git
cd omniedge-windows
```

open **OmniEdge.pro** and start to compile.


## Usage

- [Virtual Network, Devices, Security Key, and Settings](https://omniedge.io/docs/article/admin)
- [Windows 7,10,11 for Intel or Arm](https://omniedge.io/docs/article/Install/windows)
- [Android](https://omniedge.io/docs/article/Install/android)
- [Linux Cli for raspberry Pi, Nvidia Jeston,and more](https://omniedge.io/docs/article/Install/cli)
- [MacOS Cli](https://omniedge.io/docs/article/Install/macoscli)
- [Synology](https://omniedge.io/docs/article/Install/synology)
- [iOS](https://omniedge.io/docs/article/Install/ios)
- [Setup custom supernode](https://omniedge.io/docs/article/Install/customize-supernode)

## Use Cases

> Tell us your use-case, so we can share to others

- [Remote connect windows without exposing public IP with Omniedge](https://omniedge.io/docs/article/Cases/RDP)
- [Display and control macOS, Linux and Windows ](https://omniedge.io/docs/article/Cases/VNC)
- [Keep connection with your AI based Project on Jetson](https://omniedge.io/docs/article/Cases/jetson)
- [Display and control your Android device with Omniedge from anywhere on MacOS, Windows and Linux](https://omniedge.io/docs/article/Cases/android-remote)
- [Talk to your family and share photos in a LAN on the internet](https://omniedge.io/docs/article/Cases/lan-messenger)
- [Air Drop Any Files between MacOS, Windows, Routers, Linux and Android with Omniedge from anywhere](https://omniedge.io/docs/article/Cases/landrop)

## Compare

- [VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/vpn-vs-omniedge)
- [Express VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/expressvpn-vs-omniedge)
- [frp/ngrok vs. OmniEdge](https://omniedge.io/docs/article/compare/frp-ngrok-vs-omniedge)
- [ZeroTier vs. OmniEdge](https://omniedge.io/docs/article/compare/zerotier-vs-omniedge)
- [n2n vs. OmniEdge](https://omniedge.io/docs/article/compare/n2n-vs-omniedge)

## Who are talking about us

- [Founded by a Single Tweet Startup OmniEdge’s effort to let connect without concern](https://threat.technology/founded-by-a-single-tweet-startup-omniedges-effort-to-let-connect-without-concern/)
- [voonze: OmniEdge, to access your Intranet from the Internet using P2P](https://voonze.com/omniedge-to-access-your-intranet-from-the-internet-using-p2p/)
- [wwwhatsnew: OMNIEDGE, PARA ACCEDER A TU INTRANET DESDE INTERNET USANDO P2P](https://wwwhatsnew.com/2022/03/03/omniedge-para-acceder-a-tu-intranet-desde-internet-usando-p2p/)
- [l'Entrepreneur: OmniEdge, pour accéder à votre Intranet depuis Internet en P2P](https://lentrepreneur.co/style/technologie/omniedge-pour-acceder-a-votre-intranet-depuis-internet-en-p2p-04032022)
- [RunaCapital: Awesome OSS alternatives](https://github.com/RunaCapital/awesome-oss-alternatives)
- [OmniEdge 虚拟组网工具使用及原理简介](https://einverne.github.io/post/2021/11/omniedge-usage.html)
- [群晖新套件：OmniEdge 轻松连接任何平台上的所有设备](https://imnks.com/5768.html)
- [发了一条消息，我创建了一个服务全球26个国家用户的开源项目](https://zhuanlan.zhihu.com/p/535614999)

>feel free to tell us about any posts related us via issue or PR. 

----

If you have more questions, feel free to talk to us at [Discord](https://discord.gg/d4faRPYj).
