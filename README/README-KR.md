
![OmniEdge](https://user-images.githubusercontent.com/93888/185755146-a79ad5d6-7901-4855-9efb-ae108dbdcdf6.png)

<div align="center">
  <h1>OmniEdge</h1>
<a href="https://omniedge.io"><img alt="Website" src="https://img.shields.io/website?label=omniedge.io&url=https%3A%2F%2Fomniedge.io"></a>
<a href="https://github.com/omniedgeio/omniedge"><img src="https://img.shields.io/github/workflow/status/omniedgeio/omniedge/sync"></a>
<a href="https://github.com/omniedgeio/omniedge"><img src="https://img.shields.io/github/license/omniedgeio/omniedge"></a>
<a href="https://github.com/omniedgeio/omniedge/releases"><img src="https://img.shields.io/github/v/release/omniedgeio/omniedge"></a>
<a href="https://hub.docker.com/r/omniedge/omniedge"><img src="https://img.shields.io/docker/v/omniedge/omniedge?label=Docker"></a>
<a href="https://hub.docker.com/r/omniedge/omniedge"><img src="https://img.shields.io/docker/image-size/omniedge/omniedge?label=Docker%20image%20size"></a>




  <br />
  <br />
  <a href="https://omniedge.io/docs/article/install#get-started">Get Started</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://omniedge.io/">Website</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://omniedge.io/docs">Docs</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://omniedge.io/docs/article/development">Development</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://omniedge.io/docs/article/cases/">Examples Cases</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://discord.gg/FY6Yd6jcPu">Discord</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://twitter.com/omniedgeio">Twitter</a>
  <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
  <a href="https://github.com/omniedgeio/omniedge">OmniEdge</a>
  <br />
  <hr />
</div>

[【English】](../README.md) [【繁体中文】](README-zh-Hant.md) [【简体中文】](README-zh-Hans.md) [【日本语】](README-JP.md) [【Español】](README-ES.md) [【Italiano】](README-IT.md) [【한국어】](README-KR.md) 
[【العربي】](README-AR.md) [【Tiếng Việt】](README-VN.md) [【แบบไทย】](README-TH.md)

OmniEdge는 기존 VPN의 대안인 [n2n](https://github.com/ntop/n2n) 프로토콜을 기반으로 하는 오픈 소스 p2p 레이어 2VPN 인프라입니다. 중앙 서버가 없고 유지 보수가 적고 쉽게 확장할 수 있습니다. 인트라넷에서 일어나는 일은 인트라넷에 남아 있습니다.

![OmniEdge-clients](../OmniEdge-clients.png)

## 주요 특징들:

||||
|----|----|----|
|대시보드 관리 관리|메시 VPN|MacOS(메뉴 모음) 및 Windows(시스템 트레이)용 데스크탑 GUI 앱|
|다중 가상 네트워크|사이트 간 VPN|Linux, FreeBSD, Raspbian 및 MacOS용 명령줄 cli 앱|
|다중 사용자|무제한 데이터 전송|armv7,arm64,RISC-V64,x86_64 및 amd64용 명령줄 cli 앱|
|다중 장치|암호화된 P2P 연결|iOS 및 Android용 모바일 앱|
|자체 호스팅 슈퍼노드 |암호화된 연결 릴레이|iPad, Android 태블릿 및 Android TV용 태블릿 앱|
|가상 네트워크 공유|하이브리드 클라우드 지원|Synology용 NAS 앱|
|보안 키| Zero-Config|자동 공개 슈퍼노드 할당|
|[Remote Device Control](https://omniedge.io/docs/article/Cases/VNC)|[Drop Files remote](https://omniedge.io/docs/article/Cases/landdrop) |자동 IP 할당 |


기업용 [가격](https://omniedge.io/pricing) 페이지에서 더 많은 기능을 찾을 수 있습니다.

## 5분으로 시작하자

1. 계정에 가입합니다. 가입(https://omniedge.io/register)
2. [다운로드](https://omniedge.io/download) 플랫폼용 OmniEdge 앱
3. 또는 cli 버전을 사용하는 경우 다음 명령을 실행합니다.
```bash
curl https://omniedge.io/install/omniedge-install.sh | bash
``
4. 이메일 주소와 암호로 로그인하고 가상 네트워크를 선택하여 연결합니다.

준비 만단입니다!

또한 **보안 키**로 로그인하거나 기기를 관리하려면 문서(https://omniedge.io/docs)로 이동하여 자세한 내용을 확인하세요.


## 컴파일

### OmniEdge Cli

1. 환경: Golang 1.16.6
2. 컴파일:

- 2.1. Ubuntu /linux

```bash
sudo apt-get -y update
sudo apt-get install -y openssl build-essential libssl-dev zip autoconf
git clone git clone https://github.com/omniedgeio/omniedge-cli
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
#freebsd
su
pkg update && pkg install go gmake git openssl zip autoconf automake libtool
https://github.com/omniedgeio/omniedge-cli
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build-freebsd
```
  
3. 크로스 컴파일

- 3.1 RISC-V 

호스트 OS: Ubuntu 20.04

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

컴파일된 omniedge-cli는 **/out/**에 있습니다.


### OmniEdge Android

1. Android Studio 다운로드: https://developer.android.com/studio
2. 리포지토리를 검색하고 컴파일합니다.

```bash
git clone https://github.com/omniedgeio/omniedge-android.git`
./gradlew test --stacktrace
./gradlew assembleDebug --stacktrace
```

또한 Github과 Gitlab의 CI를 자동으로 빌드하기 위해 준비했습니다.

- Github: https://github.com/omniedgeio/omniedge-android/blob/main/.github/workflows/build.yml
- GitLab: https://github.com/omniedgeio/omniedge-android/blob/main/.gitlab-ci.yml


### OmniEdge iOS

1. Xcode를 다운로드하여 설치합니다.
2. 리포지토리를 검색하고 컴파일합니다.

```bash
git clone https://github.com/omniedgeio/omniedge-iOS.git
cd omniedge-iOS
open OmniEdgeNew/OmniEdgeNew.xcworkspace
```

Xcode가 자동으로 열립니다. 컴파일을 시작하려면 개발자 계정을 설정해야 합니다. 패키지, 특히 **Tunnel** 패키지를 기기에서 개별적으로 컴파일하는 것이 좋습니다.

<img width="902" alt="image" src="https://user-images.githubusercontent.com/93888/180374544-0ae0fbd8-3413-427f-8e9b-ec0c49249f0e.png">

### OmniEdge-macOS

1. Xcode를 다운로드하여 설치합니다.
2. 리포지토리를 검색하고 컴파일합니다.

```bash
git clone https://github.com/omniedgeio/omniedge-macOS.git
cd omniedge-macOS
open Omniedge.xcodeproj
```

Xcode가 자동으로 열립니다. 컴파일을 시작하려면 개발자 계정을 설정해야 합니다.

### OmniEdge-windows

1. QT를 다운로드하여 설치합니다.
2. 리포지토리를 검색하고 컴파일합니다.

```bash
git clone https://github.com/omniedgeio/omniedge-windows.git
cd omniedge-windows
```

**OmniEdge.pro**를 열고 컴파일을 시작합니다.

## 사용법

- [Virtual Network, Devices, Security Key, and Settings](https://omniedge.io/docs/article/admin)
- [Windows 7,10,11 for Intel or Arm](https://omniedge.io/docs/article/install/windows)
- [Android](https://omniedge.io/docs/article/install/android)
- [Linux Cli for raspberry Pi, Nvidia Jeston,and more](https://omniedge.io/docs/article/install/cli)
- [MacOS Cli](https://omniedge.io/docs/article/install/macoscli)
- [Synology](https://omniedge.io/docs/article/install/synology)
- [Docker](https://omniedge.io/docs/article/install/docker)
- [Github Action](https://omniedge.io/docs/article/install/github-action)
- [iOS](https://omniedge.io/docs/article/install/ios)
- [Setup custom supernode](https://omniedge.io/docs/article/install/customize-supernode)

## 유스 케이스

> 다른 사람들과 공유 할 수 있도록 유스 케이스를 알려주세요.

- [Remote connect windows without exposing public IP with Omniedge](https://omniedge.io/docs/article/Cases/RDP)
- [Display and control macOS, Linux and Windows ](https://omniedge.io/docs/article/Cases/VNC)
- [Keep connection with your AI based Project on Jetson](https://omniedge.io/docs/article/Cases/jetson)
- [Display and control your Android device with Omniedge from anywhere on MacOS, Windows and Linux](https://omniedge.io/docs/article/Cases/android-remote)
- [Talk to your family and share photos in a LAN on the internet](https://omniedge.io/docs/article/Cases/lan-messenger)
- [Air Drop Any Files between MacOS, Windows, Routers, Linux and Android with Omniedge from anywhere](https://omniedge.io/docs/article/Cases/landrop)

## 비교

- [VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/vpn-vs-omniedge)
- [Express VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/expressvpn-vs-omniedge)
- [frp/ngrok vs. OmniEdge](https://omniedge.io/docs/article/compare/frp-ngrok-vs-omniedge)
- [ZeroTier vs. OmniEdge](https://omniedge.io/docs/article/compare/zerotier-vs-omniedge)
- [n2n vs. OmniEdge](https://omniedge.io/docs/article/compare/n2n-vs-omniedge)


## 누가 우리에 대해 이야기하는지

- [Founded by a Single Tweet Startup OmniEdge’s effort to let connect without concern](https://threat.technology/founded-by-a-single-tweet-startup-omniedges-effort-to-let-connect-without-concern/)
- [voonze: OmniEdge, to access your Intranet from the Internet using P2P](https://voonze.com/omniedge-to-access-your-intranet-from-the-internet-using-p2p/)
- [wwwhatsnew: OMNIEDGE, PARA ACCEDER A TU INTRANET DESDE INTERNET USANDO P2P](https://wwwhatsnew.com/2022/03/03/omniedge-para-acceder-a-tu-intranet-desde-internet-usando-p2p/)
- [l'Entrepreneur: OmniEdge, pour accéder à votre Intranet depuis Internet en P2P](https://lentrepreneur.co/style/technologie/omniedge-pour-acceder-a-votre-intranet-depuis-internet-en-p2p-04032022)
- [RunaCapital: Awesome OSS alternatives](https://github.com/RunaCapital/awesome-oss-alternatives)
- [OmniEdge in ntopconf 2022](https://www.ntop.org/ntopconf2022/)

> 문제 또는 PR을 통해 Google과 관련된 게시물에 대해 언제든지 알려주십시오.

## Advisor

[lucaderi](https://github.com/lucaderi)



## Contributors

[harri8807](https://github.com/orgs/omniedgeio/people/harri8807) , [Tex-Tang](https://github.com/Tex-Tang), [ivyxjc](https://github.com/orgs/omniedgeio/people/ivyxjc), [kidylee](https://github.com/kidylee), [EbenDang](https://github.com/orgs/omniedgeio/people/EbenDang)
,[zteshadow](https://github.com/zteshadow), [ChenYouping](https://github.com/orgs/omniedgeio/people/ChenYouping),[ddrandy](https://github.com/orgs/omniedgeio/people/ddrandy), **Tsingv**, [mtx2d](https://github.com/mtx2d)，[Blackrose](https://github.com/Blackrose), [cheung-chifung](https://github.com/cheung-chifung),[我不是矿神](https://imnks.com/5768.html)


----

궁금한 점이 있으시면 [Discussions](https://github.com/omniedgeio/omniedge/discussions)로 문의해 주십시오.