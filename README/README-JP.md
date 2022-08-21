
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

OmniEdgeは、従来のVPNの代替手段である[n2n]（https://github.com/ntop/n2n）プロトコルに基づくオープンソースのp2pレイヤー2VPNインフラストラクチャです。 中央サーバーがなく、メンテナンスが少なくて簡単に拡張できます。 イントラネットで何が起こるかは、イントラネットにとどまります。

![OmniEdge-clients](../OmniEdge-clients.png)

## 主な機能：

||||
| ---- | ---- | ---- |
|ダッシュボード管理|メッシュVPN|MacOS（メニューバー）およびWindows（システムトレイ）用のデスクトップGUIアプリ|
|マルチ仮想ネットワーク|サイト間VPN|Linux、FreeBSD、Raspbian、MacOS用のコマンドラインCLIアプリ|
|マルチユーザー|無制限のデータ転送|armv7、arm64、RISC-V64、x86_64およびamd64用のコマンドラインCLIアプリ|
|マルチデバイス|暗号化されたピアツーピア接続|iOSおよびAndroid用のモバイルアプリ|
|セルフホストスーパーノード|暗号化接続リレー|iPad、Androidタブレット、AndroidTV用のタブレットアプリ|
|仮想ネットワークの共有|ハイブリッドクラウドのサポート|Synology用のNASアプリ|
|セキュリティキー| Zero-Config|自動パブリックスーパーノード割り当て|
| [リモートデバイス制御](https://omniedge.io/docs/article/Cases/VNC)| [ファイルをリモートでドロップ](https://omniedge.io/docs/article/Cases/landrop)|自動IP割り当て |


その他の機能については、エンタープライズ向けの[価格設定]（https://omniedge.io/pricing）ページをご覧ください。

## 5分で始めましょう

1. アカウントにサインアップします：[サインアップ]（https://omniedge.io/register）
2. [ダウンロード]（https://omniedge.io/download）プラットフォーム用のOmniEdgeアプリ
3. または、cliバージョンを使用する場合は、次のコマンドを実行します。
`` `bash
curl https://omniedge.io/install/omniedge-install.sh | bash
`` `
4. メールアドレスとパスワードでログインし、仮想ネットワークを選択して接続します。

準備万端です！

また、**セキュリティキー**でログインする場合、またはデバイスを管理する場合は、[ドキュメント]（https://omniedge.io/docs）にアクセスして詳細を確認してください。


## コンパイル

### OmniEdge Cli

1. 環境：Golang 1.16.6
2. コンパイル：

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
  
3. クロスコンパイル

- 3.1 RISC-V 

ホストOS: Ubuntu 20.04

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

コンパイルされたomniedge-cliは**/out/**にあります


### OmniEdge Android

1. Android Studioをダウンロードします：https：//developer.android.com/studio
2. リポジトリを取得してコンパイルします

```bash
git clone https://github.com/omniedgeio/omniedge-android.git`
./gradlew test --stacktrace
./gradlew assembleDebug --stacktrace
```

また、GithubとGitlabのCIを自動的にビルドするために準備しました。

- Github: https://github.com/omniedgeio/omniedge-android/blob/main/.github/workflows/build.yml
- GitLab: https://github.com/omniedgeio/omniedge-android/blob/main/.gitlab-ci.yml


### OmniEdge iOS

1. Xcodeをダウンロードしてインストールします
2. リポジトリを取得してコンパイルします

```bash
git clone https://github.com/omniedgeio/omniedge-iOS.git
cd omniedge-iOS
open OmniEdgeNew/OmniEdgeNew.xcworkspace
```

Xcodeが自動的に開きます。コンパイルを開始するには、開発者アカウントを設定する必要があります。 パッケージ、特に**Tunnel**パッケージをデバイスで個別にコンパイルすることをお勧めします。

<img width="902" alt="image" src="https://user-images.githubusercontent.com/93888/180374544-0ae0fbd8-3413-427f-8e9b-ec0c49249f0e.png">

### OmniEdge-macOS

1. Xcodeをダウンロードしてインストールします
2. リポジトリを取得してコンパイルします

```bash
git clone https://github.com/omniedgeio/omniedge-macOS.git
cd omniedge-macOS
open Omniedge.xcodeproj
```

Xcodeが自動的に開きます。コンパイルを開始するには、開発者アカウントを設定する必要があります。

### OmniEdge-windows

1. QTをダウンロードしてインストールします
2. リポジトリを取得してコンパイルします

```bash
git clone https://github.com/omniedgeio/omniedge-windows.git
cd omniedge-windows
```

**OmniEdge.pro**を開き、コンパイルを開始します。

## 使用法

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

## ユースケース

>他の人と共有できるように、ユースケースを教えてください

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


## Media

- [Founded by a Single Tweet Startup OmniEdge’s effort to let connect without concern](https://threat.technology/founded-by-a-single-tweet-startup-omniedges-effort-to-let-connect-without-concern/)
- [voonze: OmniEdge, to access your Intranet from the Internet using P2P](https://voonze.com/omniedge-to-access-your-intranet-from-the-internet-using-p2p/)
- [wwwhatsnew: OMNIEDGE, PARA ACCEDER A TU INTRANET DESDE INTERNET USANDO P2P](https://wwwhatsnew.com/2022/03/03/omniedge-para-acceder-a-tu-intranet-desde-internet-usando-p2p/)
- [l'Entrepreneur: OmniEdge, pour accéder à votre Intranet depuis Internet en P2P](https://lentrepreneur.co/style/technologie/omniedge-pour-acceder-a-votre-intranet-depuis-internet-en-p2p-04032022)
- [RunaCapital: Awesome OSS alternatives](https://github.com/RunaCapital/awesome-oss-alternatives)
- [OmniEdge in ntopconf 2022](https://www.ntop.org/ntopconf2022/)

>問題またはPRを介して私たちに関連する投稿についてお気軽にお知らせください。

## Advisor

[lucaderi](https://github.com/lucaderi)

## Contributors

[harri8807](https://github.com/orgs/omniedgeio/people/harri8807) , [Tex-Tang](https://github.com/Tex-Tang), [ivyxjc](https://github.com/orgs/omniedgeio/people/ivyxjc), [kidylee](https://github.com/kidylee), [EbenDang](https://github.com/orgs/omniedgeio/people/EbenDang)
,[zteshadow](https://github.com/zteshadow), [ChenYouping](https://github.com/orgs/omniedgeio/people/ChenYouping),[ddrandy](https://github.com/orgs/omniedgeio/people/ddrandy), **Tsingv**, [mtx2d](https://github.com/mtx2d)，[Blackrose](https://github.com/Blackrose), [cheung-chifung](https://github.com/cheung-chifung),[我不是矿神](https://imnks.com/5768.html)

----

ご不明な点がございましたら、[Discord](https://discord.gg/d4faRPYj）までお気軽にお問い合わせください。
