# OmniEdge 

**因特网上的私有网络时代**

[【简体中文】](README-zh-Hans.md)  [【繁体中文】](README-zh-Hant.md) [【English】](../README-ZH.md)

>OmniEdge 的端到端企业 VPN 解决方案，无需公网 IP，无需端口转发，无需反向代理，零配置，不仅适用于初创业团队、个人，也适用于需要弹性扩张，在世界各地拥有成千上万台电脑的大公司。局域网的事情，就要放在局域网。

[【OmniEdge 如何工作】](https://omniedge.io/docs/article/architecture) [【下载】](#安装-omniedge) [【公共超级节点】](#免费的公共超级节点) [【编译】](#编译) [【优势】](#omniedge的优势) [【媒体】](#谁在谈论-omniedge)

我们需要您用您的语言翻译本 README, [OmniEdge Windows UI](https://github.com/omniedgeio/omniedge-windows/tree/dev/languages),[OmniEdge Android UI](https://github.com/omniedgeio/omniedge-android/tree/main/app/src/main/res/values) 和 [Docs](https://github.com/omniedgeio/docs) 。

Chat with us: [🤝 网站](https://omniedge.io) [💬 Twitter](https://twitter.com/omniedgeio) [😇 Discord](https://discord.gg/d4faRPYj)

![OmniEdge-clients](../OmniEdge-clients.png)

## 安装 OmniEdge

在官方网站 https://omniedge.io 注册账号，同时下载对应设备的客户端： 

-   [Windows](https://omniedge.io/install/download/0.2.3/omniedge-setup-0.2.3.exe)
-   [iOS & M1 Mac on App Store](https://apps.apple.com/us/app/omniedgenew/id1603005893)
-   [Android: OmniEdge.apk](https://omniedge.io/install/download/0.2.2/omniedge-release-v0.2.2.apk)
-   [CLi for macOS, Linux, Raspberry Pi, ARM and Nvidia Jetson](https://omniedge.io/install/download/0.2.3/omniedgecli-macos-latest.zip)
    ```bash
    curl https://omniedge.io/install/omniedge-install.sh | bash
    ```
-   [群晖](https://omniedge.io/download/synology)

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
2. 依赖: 

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
3. 编译

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

编译好的文件可以在 **/out/** 找到。
您也可以使用自带的 Github Workflow 自动化编译。

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
