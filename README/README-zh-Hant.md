
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


[【English】](../README.md) [【正體中文】](README-zh-Hant.md) [【簡體中文】](README-zh-Hans.md) [【日本語】](README-JP.md) [【Español】](README-ES.md) [【Italiano】](README-IT.md) [【한국어】](README-KR.md) 
[【العربي】](README-AR.md) [【Tiếng Việt】](README-VN.md) [【แบบไทย】](README-TH.md)

>OmniEdge 的端到端企業 VPN 解決方案，無需公網 IP，無需端口轉發，無需反嚮代理，零配置，不僅適用於初創業團隊、個人，也適用於需要彈性擴張，在世界各地擁有成仟上萬臺電腦的大公司。局域網的事情，就要放在局域網。

![OmniEdge-clients](../OmniEdge-clients.png)

## 關鍵功能:

||||
|----|----|----|
|控製管理平臺| :fire: Mesh VPNs|桌麵 GUI 客戶端支援: MacOS(menubar) 和 Windows(systray)|
| :fire: 多私有網路 | :fire: Site-to-Site VPNs|命令行客戶端支援: Linux,FreeBSD, Raspian 和 MacOS|
|多用戶|無限流量|命令行客戶端支援:armv7,arm64,RISC-V64,x86_64 和 amd64|
|多設備|加密的端到端連接|手機客戶端支援: iOS 和 Android|
| :fire: 自建超級節點 |加密relay|平闆客戶端支援: iPad, Android Tablet 和 Android TV|
| :fire: 分享私有網路|混合雲支援|NAS GUI 客戶端支援: 群暉|
|安全碼登入| :fire: 零配置|自動分配公共超級節點|
| :fire: [遠程控製](https://omniedge.io/docs/article/Cases/VNC)|[遠程 Drop 文件](https://omniedge.io/docs/article/Cases/landrop) |自動IP分配|


您也可以查閱 [Pricing](https://omniedge.io/pricing) 頁麵獲取更多的企業版功能。

## 5分鍾啓用OmniEdge

1. 註冊您的個人帳號: [註冊](https://omniedge.io/register)
2. [下載](https://omniedge.io/download) OmniEdge 客戶端
3. 如果您想使用命令行版本，可以使用以下命令安裝 Cli 版本：
```bash
curl https://omniedge.io/install/omniedge-install.sh | bash
```
4. 使用信箱和密碼登入客戶端，選擇需要加入的虛擬網路，點選加入，一切就準備好了。

如果您想使用**安全碼**登入或者想要管理設備和虛擬網路，請查閱[官方文檔](https://omniedge.io/docs)

## 服務器狀態

  >這個服務器狀態由 OmniEdge for Github Action 自動生成，每5個小時更新一次。
  
  [OmniEdge 服務狀態](https://github.com/omniedgeio/server-status#server-status)
  
## 編譯

### 編譯 OmniEdge Cli

1. 環境: Golang 1.16.6
2. 編譯: 
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

3. 交叉編譯

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

編譯好的文件可以在 **/out/** 找到。
您也可以使用自帶的 Github Workflow 自動化編譯。


### 編譯 OmniEdge Android

1. 下載並安裝 Android Studio: https://developer.android.com/studio
2. 下載源代碼開始編譯

```bash
git clone https://github.com/omniedgeio/omniedge-android.git`
./gradlew test --stacktrace
./gradlew assembleDebug --stacktrace
```

我們也針對 Github 和 Gitlab 準備了自動化編譯腳本，可以直接使用：

- Github: https://github.com/omniedgeio/omniedge-android/blob/main/.github/workflows/build.yml
- GitLab: https://github.com/omniedgeio/omniedge-android/blob/main/.gitlab-ci.yml


### 編譯 OmniEdge iOS

1. 下載並安裝 Xcode
2. 下載源代碼開始編譯

```bash
git clone https://github.com/omniedgeio/omniedge-iOS.git
cd omniedge-iOS
open OmniEdgeNew/OmniEdgeNew.xcworkspace
```

Xcode 會自動打開，開始編譯前請先設定開發者帳號。我們建議您單獨編譯以下每一個包，特別是**Tunnel**。

<img width="902" alt="image" src="https://user-images.githubusercontent.com/93888/180374544-0ae0fbd8-3413-427f-8e9b-ec0c49249f0e.png">

### 編譯 OmniEdge-macOS

1. 下載並安裝 Xcode
2. 下載源代碼開始編譯

```bash
git clone https://github.com/omniedgeio/omniedge-macOS.git
cd omniedge-macOS
open Omniedge.xcodeproj
```

Xcode 會自動打開，開始編譯前請先設定開發者帳號。

### OmniEdge-windows

1. 下載並安裝 QT
2. 下載源代碼開始編譯

```bash
git clone https://github.com/omniedgeio/omniedge-windows.git
cd omniedge-windows
```

打開 **OmniEdge.pro** 開始編譯.

## 使用安裝 OmniEdge

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

## 應用場景

> 如果您有以下冇有列出的應用，歡迎提PR，分享給更多的人

- [Remote connect windows without exposing public IP with Omniedge](https://omniedge.io/docs/article/Cases/RDP)
- [Display and control macOS, Linux and Windows ](https://omniedge.io/docs/article/Cases/VNC)
- [Keep connection with your AI based Project on Jetson](https://omniedge.io/docs/article/Cases/jetson)
- [Display and control your Android device with Omniedge from anywhere on MacOS, Windows and Linux](https://omniedge.io/docs/article/Cases/android-remote)
- [Talk to your family and share photos in a LAN on the internet](https://omniedge.io/docs/article/Cases/lan-messenger)
- [Air Drop Any Files between MacOS, Windows, Routers, Linux and Android with Omniedge from anywhere](https://omniedge.io/docs/article/Cases/landrop)

## 比較

- [VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/vpn-vs-omniedge)
- [Express VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/expressvpn-vs-omniedge)
- [frp/ngrok vs. OmniEdge](https://omniedge.io/docs/article/compare/frp-ngrok-vs-omniedge)
- [ZeroTier vs. OmniEdge](https://omniedge.io/docs/article/compare/zerotier-vs-omniedge)
- [n2n vs. OmniEdge](https://omniedge.io/docs/article/compare/n2n-vs-omniedge)


## 誰在談論 OmniEdge

- [Founded by a Single Tweet Startup OmniEdge’s effort to let connect without concern](https://threat.technology/founded-by-a-single-tweet-startup-omniedges-effort-to-let-connect-without-concern/)
- [voonze: OmniEdge, to access your Intranet from the Internet using P2P](https://voonze.com/omniedge-to-access-your-intranet-from-the-internet-using-p2p/)
- [wwwhatsnew: OMNIEDGE, PARA ACCEDER A TU INTRANET DESDE INTERNET USANDO P2P](https://wwwhatsnew.com/2022/03/03/omniedge-para-acceder-a-tu-intranet-desde-internet-usando-p2p/)
- [l'Entrepreneur: OmniEdge, pour accéder à votre Intranet depuis Internet en P2P](https://lentrepreneur.co/style/technologie/omniedge-pour-acceder-a-votre-intranet-depuis-internet-en-p2p-04032022)
- [RunaCapital: Awesome OSS alternatives](https://github.com/RunaCapital/awesome-oss-alternatives)
- [OmniEdge in ntopconf 2022](https://www.ntop.org/ntopconf2022/)

>如果您看到了任何有關於 OmniEdge 的文章，請給我們提PR或者發issue

## Advisor

[lucaderi](https://github.com/lucaderi)

## Contributors

[harri8807](https://github.com/orgs/omniedgeio/people/harri8807) , [Tex-Tang](https://github.com/Tex-Tang), [ivyxjc](https://github.com/orgs/omniedgeio/people/ivyxjc), [kidylee](https://github.com/kidylee), [EbenDang](https://github.com/orgs/omniedgeio/people/EbenDang)
,[zteshadow](https://github.com/zteshadow), [ChenYouping](https://github.com/orgs/omniedgeio/people/ChenYouping),[ddrandy](https://github.com/orgs/omniedgeio/people/ddrandy), **Tsingv**, [mtx2d](https://github.com/mtx2d)，[Blackrose](https://github.com/Blackrose), [cheung-chifung](https://github.com/cheung-chifung),[我不是礦神](https://imnks.com/5768.html)

----

如果您有更多問題，請去[Discussions](https://github.com/omniedgeio/omniedge/discussions) 提問。
