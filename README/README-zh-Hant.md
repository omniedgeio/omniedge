# OmniEdge 

**因特網上的私有網路時代**

[【簡體中文】](README-zh-Hans.md)  [【正體中文】](README-zh-Hant.md) [【English】](../README-ZH.md)

>OmniEdge 的端到端企業 VPN 解決方案，無需公網 IP，無需端口轉發，無需反嚮代理，零配置，不僅適用於初創業團隊、個人，也適用於需要彈性擴張，在世界各地擁有成仟上萬臺電腦的大公司。局域網的事情，就要放在局域網。

[【OmniEdge 如何工作】](https://omniedge.io/docs/article/architecture) [【下載】](#安裝-omniedge) [【公共超級節點】](#免費的公共超級節點) [【自建超級節點】](#自建超級節點)[【編譯】](#編譯) [【使用】](#使用安裝-omniedge) [【應用場景】](#應用場景)[【比較】](#比較) [【優勢】](#omniedge的優勢) [【媒體】](#誰在談論-omniedge)

我們需要您用您的語言翻譯本 README, [OmniEdge Windows UI](https://github.com/omniedgeio/omniedge-windows/tree/dev/languages) ,[OmniEdge Android UI](https://github.com/omniedgeio/omniedge-android/tree/main/app/src/main/res/values) 和 [Docs](https://github.com/omniedgeio/docs) 。

Chat with us: [🤝 網站](https://omniedge.io) [💬 Twitter](https://twitter.com/omniedgeio) [😇 Discord](https://discord.gg/d4faRPYj)

![OmniEdge-clients](../OmniEdge-clients.png)

## 安裝 OmniEdge

- 在官方網站 https://omniedge.io 註冊賬號，同時下載對應設備的客戶端。
- [下載](https://github.com/omniedgeio/omniedge/releases)

## 免費的公共超級節點

我們提供免費的公共超級節點為免費用戶使用，節點會隨點用戶註冊時的IP地理位置自動分配。如果您註冊的時候使用的IP位址與您的設備不同，p2p的連接可能會慢，您也可以使用[專業和團隊]((https://omniedge.io/pricing))版本的[自定義超級節點]((https://omniedge.io/docs/article/install/customize-supernode) )，使用自建的超級節點，更快更安全 . 

|位置|雲服務商|配置|超級節點版本|
|--|--|--|--|
|Hong Kong,CN|AWS| 2vCPUs / 1GB RAM|2.6-stable-omni|
|Singapore,SG|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Tokyo,JP|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Oregon,US|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Ohio,US|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Mumbai,IN|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Sao Paulo,BR|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Frankfurt,DE|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Milan,IT|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|
|Sydney,AU|AWS|1vCPU / 0.5GB RAM|2.6-stable-omni|

## 自建超級節點

OmniEdge 可以讓用戶自建超級節點，使用自建的超級節點，可以最大限度的提高速度，降低延時。這裏我們提供了一個腳本，可以非常方便的使用 Docker 設定一個超級節點。

### 安裝

```bash
curl https://raw.githubusercontent.com/omniedgeio/docker-customize-supernode/main/install.sh | bash

1) 2.6-stable-omni
2) 3.0-stable
3) Quit
Please enter your choice: 1

#請輸入 1 選擇 2.6-stable-omni，這是目前客戶端支援的版本，3.0稍後推出
#預設端口是 443，也可以選擇其他端口，請確認服務器和端口的可用性。

```

### 在 OmniEdge 的管理界麵設定自建超級節點

OmniEdge允許為不同的虛擬網路設定不同的超級節點。登入你的帳號，到管理界麵，選擇對應的虛擬網路，輸入自建超級節點的 **IP 地址** 和 **端口**。

![](../Customizesupernode.png)

**註意：更改超級節點後，各個客戶端需要重新登入以更新超級節點信息。**


## OmniEdge的優勢

![OmniEdgeComparison](../OmniEdgeComparison.gif)

## 源代碼

- 自定義認證節點：https://github.com/omniedgeio/docker-customize-supernode
- 客戶端原代碼: 
    - [Windows](https://github.com/omniedgeio/omniedge-windows)
    - [macOS (Intel, M1/M2 MacBook)](https://github.com/omniedgeio/omniedge-macOS)
    - [iOS](https://github.com/omniedgeio/omniedge-iOS) 
    - [Android 安卓](https://github.com/omniedgeio/omniedge-android)
    - [群暉版本](https://github.com/omniedgeio/omniedge-synology)  
    - [Linux Cli](https://github.com/omniedgeio/omniedge-cli)
- 協議： https://github.com/omniedgeio/n2n

## 編譯

### 編譯 OmniEdge Cli

1. 環境: Golang 1.16.6
2. 依賴: 

```bash
#ubuntu/linux
sudo -E apt-get -y update
sudo -E apt-get install -y openssl
sudo -E apt-get install -y build-essential
sudo -E apt-get install -y libssl-dev
sudo -E apt-get install -y zip
```

```bash
#macOS
brew install autoconf automake libtool
```
3. 編譯

```bash
#ubuntu/linux
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build
```

```bash
# MacOS
cd omniedge-cli
go mod download
go generate
BUILD_ENV=prod make build-darwin
```

編譯好的文件可以在 **/out/** 找到。
您也可以使用自帶的 Github Workflow 自動化編譯。

## 使用安裝 OmniEdge

- [Virtual Network, Devices, Security Key, and Settings](https://omniedge.io/docs/article/admin)
- [Windows 7,10,11 for Intel or Arm](https://omniedge.io/docs/article/Install/windows)
- [Android](https://omniedge.io/docs/article/Install/android)
- [Linux Cli for raspberry Pi, Nvidia Jeston,and more](https://omniedge.io/docs/article/Install/cli)
- [MacOS Cli](https://omniedge.io/docs/article/Install/macoscli)
- [Synology](https://omniedge.io/docs/article/Install/synology)
- [iOS](https://omniedge.io/docs/article/Install/ios)
- [Setup custom supernode](https://omniedge.io/docs/article/Install/customize-supernode)

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
- [OmniEdge 虛擬組網工具使用及原理簡介](https://einverne.github.io/post/2021/11/omniedge-usage.html)
- [群暉新套件：OmniEdge 輕鬆連接任何平臺上的所有設備](https://imnks.com/5768.html)
- [發了一條消息，我創建了一個服務全球26個國家用戶的開源項目](https://zhuanlan.zhihu.com/p/535614999)

>如果您看到了任何有關於 OmniEdge 的文章，請給我們提PR或者發issue

----

如果您有更多問題，請去[Discord](https://discord.gg/d4faRPYj) 提問。
