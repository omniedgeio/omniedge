# OmniEdge 

**因特网上的私有网络时代**

[【简体中文】](README-zh-Hans.md)  [【繁体中文】](README-zh-Hant.md) [【English】](../README-ZH.md)

>OmniEdge 的端到端企业 VPN 解决方案，无需公网 IP，无需端口转发，无需反向代理，零配置，不仅适用于初创业团队、个人，也适用于需要弹性扩张，在世界各地拥有成千上万台电脑的大公司。局域网的事情，就要放在局域网。

[【OmniEdge 如何工作】](https://omniedge.io/docs/article/architecture) [【下载】](#安装-omniedge) [【公共超级节点】](#免费的公共超级节点) [【自建超级节点】](#自建超级节点)[【编译】](#编译) [【使用】](#使用安装-omniedge) [【应用场景】](#应用场景)[【比较】](#比较) [【优势】](#omniedge的优势) [【媒体】](#谁在谈论-omniedge)

我们需要您用您的语言翻译本 README, [OmniEdge Windows UI](https://github.com/omniedgeio/omniedge-windows/tree/dev/languages),[OmniEdge Android UI](https://github.com/omniedgeio/omniedge-android/tree/main/app/src/main/res/values) 和 [Docs](https://github.com/omniedgeio/docs) 。

Chat with us: [🤝 网站](https://omniedge.io) [💬 Twitter](https://twitter.com/omniedgeio) [😇 Discord](https://discord.gg/d4faRPYj)

![OmniEdge-clients](../OmniEdge-clients.png)

## 安装 OmniEdge

- 在官方网站 https://omniedge.io 注册账号，同时下载对应设备的客户端。
- [下载](https://github.com/omniedgeio/omniedge/releases)

## 免费的公共超级节点

我们提供免费的公共超级节点为免费用户使用，节点会随点用户注册时的IP地理位置自动分配。如果您注册的时候使用的IP地址与您的设备不同，p2p的连接可能会慢，您也可以使用[专业和团队]((https://omniedge.io/pricing))版本的[自定义超级节点]((https://omniedge.io/docs/article/install/customize-supernode) )，使用自建的超级节点，更快更安全 . 

|位置|云服务商|配置|超级节点版本|
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

## 自建超级节点

OmniEdge 可以让用户自建超级节点，使用自建的超级节点，可以最大限度的提高速度，降低延时。这里我们提供了一个脚本，可以非常方便的使用 Docker 设置一个超级节点。

### 安装

```bash
curl https://raw.githubusercontent.com/omniedgeio/docker-customize-supernode/main/install.sh | bash
```

>1) 2.6-stable-omni
>2) 3.0-stable
>3) Quit

>Please enter your choice: 1

#请输入 1 选择 2.6-stable-omni，这是目前客户端支持的版本，3.0稍后推出
#默认端口是 443，也可以选择其他端口，请确认服务器和端口的可用性。


### 在 OmniEdge 的管理界面设置自建超级节点

OmniEdge允许为不同的虚拟网络设置不同的超级节点。登录你的帐号，到管理界面，选择对应的虚拟网络，输入自建超级节点的 **IP 地址** 和 **端口**。

![](../Customizesupernode.png)

**注意：更改超级节点后，各个客户端需要重新登录以更新超级节点信息。**

## OmniEdge的优势

![OmniEdgeComparison](../OmniEdgeComparison.gif)

## 源代码

- 自定义认证节点：https://github.com/omniedgeio/docker-customize-supernode
- 客户端原代码: 
    - [Windows](https://github.com/omniedgeio/omniedge-windows)
    - [macOS (Intel, M1/M2 MacBook)](https://github.com/omniedgeio/omniedge-macOS)
    - [iOS](https://github.com/omniedgeio/omniedge-iOS) 
    - [Android 安卓](https://github.com/omniedgeio/omniedge-android)
    - [群晖版本](https://github.com/omniedgeio/omniedge-synology)  
    - [Linux Cli](https://github.com/omniedgeio/omniedge-cli)
- 协议： https://github.com/omniedgeio/n2n

## 编译

### 编译 OmniEdge Cli

1. 环境: Golang 1.16.6
2. 编译: 
- 2.1. Ubuntu /linux

```bash
sudo -E apt-get -y update
sudo -E apt-get install -y openssl
sudo -E apt-get install -y build-essential
sudo -E apt-get install -y libssl-dev
sudo -E apt-get install -y zip
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

编译好的文件可以在 **/out/** 找到。
您也可以使用自带的 Github Workflow 自动化编译。

## 使用安装 OmniEdge

- [Virtual Network, Devices, Security Key, and Settings](https://omniedge.io/docs/article/admin)
- [Windows 7,10,11 for Intel or Arm](https://omniedge.io/docs/article/Install/windows)
- [Android](https://omniedge.io/docs/article/Install/android)
- [Linux Cli for raspberry Pi, Nvidia Jeston,and more](https://omniedge.io/docs/article/Install/cli)
- [MacOS Cli](https://omniedge.io/docs/article/Install/macoscli)
- [Synology](https://omniedge.io/docs/article/Install/synology)
- [iOS](https://omniedge.io/docs/article/Install/ios)
- [Setup custom supernode](https://omniedge.io/docs/article/Install/customize-supernode)

## 应用场景

> 如果您有以下没有列出的应用，欢迎提PR，分享给更多的人

- [Remote connect windows without exposing public IP with Omniedge](https://omniedge.io/docs/article/Cases/RDP)
- [Display and control macOS, Linux and Windows ](https://omniedge.io/docs/article/Cases/VNC)
- [Keep connection with your AI based Project on Jetson](https://omniedge.io/docs/article/Cases/jetson)
- [Display and control your Android device with Omniedge from anywhere on MacOS, Windows and Linux](https://omniedge.io/docs/article/Cases/android-remote)
- [Talk to your family and share photos in a LAN on the internet](https://omniedge.io/docs/article/Cases/lan-messenger)
- [Air Drop Any Files between MacOS, Windows, Routers, Linux and Android with Omniedge from anywhere](https://omniedge.io/docs/article/Cases/landrop)

## 比较

- [VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/vpn-vs-omniedge)
- [Express VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/expressvpn-vs-omniedge)
- [frp/ngrok vs. OmniEdge](https://omniedge.io/docs/article/compare/frp-ngrok-vs-omniedge)
- [ZeroTier vs. OmniEdge](https://omniedge.io/docs/article/compare/zerotier-vs-omniedge)
- [n2n vs. OmniEdge](https://omniedge.io/docs/article/compare/n2n-vs-omniedge)


## 谁在谈论 OmniEdge

- [Founded by a Single Tweet Startup OmniEdge’s effort to let connect without concern](https://threat.technology/founded-by-a-single-tweet-startup-omniedges-effort-to-let-connect-without-concern/)
- [voonze: OmniEdge, to access your Intranet from the Internet using P2P](https://voonze.com/omniedge-to-access-your-intranet-from-the-internet-using-p2p/)
- [wwwhatsnew: OMNIEDGE, PARA ACCEDER A TU INTRANET DESDE INTERNET USANDO P2P](https://wwwhatsnew.com/2022/03/03/omniedge-para-acceder-a-tu-intranet-desde-internet-usando-p2p/)
- [l'Entrepreneur: OmniEdge, pour accéder à votre Intranet depuis Internet en P2P](https://lentrepreneur.co/style/technologie/omniedge-pour-acceder-a-votre-intranet-depuis-internet-en-p2p-04032022)
- [RunaCapital: Awesome OSS alternatives](https://github.com/RunaCapital/awesome-oss-alternatives)
- [OmniEdge 虚拟组网工具使用及原理简介](https://einverne.github.io/post/2021/11/omniedge-usage.html)
- [群晖新套件：OmniEdge 轻松连接任何平台上的所有设备](https://imnks.com/5768.html)
- [发了一条消息，我创建了一个服务全球26个国家用户的开源项目](https://zhuanlan.zhihu.com/p/535614999)

>如果您看到了任何有关于 OmniEdge 的文章，请给我们提PR或者发issue


----

如果您有更多问题，请去[Discord](https://discord.gg/d4faRPYj) 提问。
