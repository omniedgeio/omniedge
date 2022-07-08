# OmniEdge 

**因特網上的私有網路時代**

>OmniEdge 的端到端企業 VPN 解決方案，無需公網 IP，無需端口轉發，無需反嚮代理，零配置，不僅適用於初創業團隊、個人，也適用於需要彈性擴張，在世界各地擁有成仟上萬臺電腦的大公司。局域網的事情，就要放在局域網。

[OmniEdge 如何工作](https://omniedge.io/docs/article/architecture)

[【簡體中文】](README-zh-Hans.md)  [【正體中文】](README-zh-Hant.md) [【English】](../README-ZH.md)

我們需要您用您的語言翻譯本 README, [OmniEdge Windows UI](https://github.com/omniedgeio/omniedge-windows/tree/dev/languages) 和 [Docs](https://github.com/omniedgeio/docs) 。

Chat with us: [🤝 網站](https://omniedge.io) [💬 Twitter](https://twitter.com/omniedgeio) [😇 Discord](https://discord.gg/d4faRPYj)

![OmniEdge-clients](../OmniEdge-clients.png)

## 安裝 OmniEdge

在官方網站 https://omniedge.io 註冊賬號，同時下載對應設備的客戶端： 

-   [Windows](https://omniedge.io/install/download/0.2.3/omniedge-setup-0.2.3.exe)
-   [iOS & M1 Mac on App Store](https://apps.apple.com/us/app/omniedgenew/id1603005893)
-   [Android: OmniEdge.apk](https://omniedge.io/install/download/0.2.2/omniedge-release-v0.2.2.apk)
-   [CLi for macOS, Linux, Raspberry Pi, ARM and Nvidia Jetson](https://omniedge.io/install/download/0.2.3/omniedgecli-macos-latest.zip)
    ```bash
    curl https://omniedge.io/install/omniedge-install.sh | bash
    ```
-   [群暉](https://omniedge.io/download/synology)

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
