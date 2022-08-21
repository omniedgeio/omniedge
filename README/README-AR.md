
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

OmniEdge عبارة عن بنية أساسية لشبكة VPN من الطبقة الثانية من p2p مفتوحة المصدر تعتمد على بروتوكول [n2n](https://github.com/ntop/n2n) ، وهو بديل تقليدي للشبكات الظاهرية الخاصة. لا يوجد خادم مركزي ، سهل القياس مع صيانة أقل. ما يحدث في الإنترانت ، يبقى في الإنترانت.
   
![OmniEdge-clients](../OmniEdge-clients.png)

## دلائل الميزات:

||||
| ---- | ---- | ---- |
| إدارة إدارة لوحة المعلومات | الشبكات الظاهرية الخاصة الشبكية | تطبيقات واجهة المستخدم الرسومية لسطح المكتب لنظام التشغيل MacOS (شريط القوائم) و Windows (نظام التشغيل systray) |
| شبكات افتراضية متعددة | شبكات VPN من موقع إلى موقع | تطبيقات سطر الأوامر cli لنظام التشغيل Linux و FreeBSD و Raspbian و MacOS |
| تعدد المستخدمين | نقل غير محدود للبيانات | تطبيقات سطر الأوامر cli لـ armv7 و arm64 و RISC-V64 و x86_64 و amd64 |
| أجهزة متعددة | اتصال نظير إلى نظير مشفر | تطبيقات الجوال لنظامي التشغيل iOS و Android |
| Supernode ذاتية الاستضافة | ترحيل اتصال مشفر | تطبيقات الأجهزة اللوحية لأجهزة iPad و Android Tablet و Android TV |
| مشاركة الشبكة الافتراضية | دعم السحابة المختلطة | تطبيق NAS لـ Synology |
| مفاتيح الأمان | التكوين الصفري | التخصيص التلقائي للعموم الفائق |
| [التحكم في الجهاز عن بُعد](https://omniedge.io/docs/article/Cases/VNC) | [إفلات الملفات عن بُعد](https://omniedge.io/docs/article/Cases/landrop) | التخصيص التلقائي لعنوان IP |


يمكنك العثور على مزيد من الميزات في صفحة [التسعير] (https://omniedge.io/pricing) للمؤسسات.

## ابدأ في 5 دقائق

1. اشترك في حسابك: [اشترك] (https://omniedge.io/register)
2. [تنزيل] (https://omniedge.io/download) تطبيقات OmniEdge لمنصتك
3. أو قم بتشغيل الأمر التالي إذا كنت تريد استخدام إصدار cli:
"" باش
حليقة https://omniedge.io/install/omniedge-install.sh | سحق
""
4. تسجيل الدخول باستخدام البريد الإلكتروني وكلمة المرور الخاصة بك ، حدد الشبكة الخاصة بك ، والاتصال!

أنت كل مجموعة!

وإذا كنت تريد تسجيل الدخول باستخدام ** مفتاح الأمان ** ، أو إدارة أجهزتك ، فانتقل وتحقق من [التوثيق] (https://omniedge.io/docs) لمزيد من المعلومات.

## ترجمة

### OmniEdge Cli

1. البيئة: جولانج 1.16.6
2. تجميع:

- 2.1. Ubuntu /linux

```bash
sudo apt-get -y update
sudo apt-get install -y openssl build-essential libssl-dev zip autoconf
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

3. عبر ترجمة

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

سيتم العثور على omniedge-cli المترجم في ** / خارج / **

### OmniEdge Android

1. قم بتنزيل Android Studio: https://developer.android.com/studio
2. الحصول على الريبو والترجمة

```bash
git clone https://github.com/omniedgeio/omniedge-android.git`
./gradlew test --stacktrace
./gradlew assembleDebug --stacktrace
```

لقد أعددنا أيضًا CI لـ Github و Gitlab للبناء تلقائيًا.

1. Github: https://github.com/omniedgeio/omniedge-android/blob/main/.github/workflows/build.yml
2. GitLab: https://github.com/omniedgeio/omniedge-android/blob/main/.gitlab-ci.yml


### OmniEdge iOS

1. قم بتنزيل وتثبيت Xcode
2. الحصول على الريبو والترجمة

```bash
git clone https://github.com/omniedgeio/omniedge-iOS.git
cd omniedge-iOS
open OmniEdgeNew/OmniEdgeNew.xcworkspace
```

سيتم فتح Xcode تلقائيًا ، يجب عليك تعيين حساب المطور الخاص بك لبدء الترجمة. نوصي بتجميع الحزمة على أجهزتك بشكل منفصل ، خاصة حزمة **Tunnel**.

<img width="902" alt="image" src="https://user-images.githubusercontent.com/93888/180374544-0ae0fbd8-3413-427f-8e9b-ec0c49249f0e.png">

### OmniEdge-macOS

1. قم بتنزيل وتثبيت Xcode
2. الحصول على الريبو والترجمة

```bash
git clone https://github.com/omniedgeio/omniedge-macOS.git
cd omniedge-macOS
open Omniedge.xcodeproj
```

سيتم فتح Xcode تلقائيًا ، يجب عليك تعيين حساب المطور الخاص بك لبدء الترجمة.

### OmniEdge-windows

1. قم بتنزيل وتثبيت QT
2. الحصول على الريبو والترجمة

```bash
git clone https://github.com/omniedgeio/omniedge-windows.git
cd omniedge-windows
```

افتح **OmniEdge.pro** وابدأ في التجميع.

## الاستخدام

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

## استخدم حالات

> أخبرنا بحالة الاستخدام الخاصة بك ، حتى نتمكن من مشاركتها مع الآخرين

- [Remote connect windows without exposing public IP with Omniedge](https://omniedge.io/docs/article/Cases/RDP)
- [Display and control macOS, Linux and Windows ](https://omniedge.io/docs/article/Cases/VNC)
- [Keep connection with your AI based Project on Jetson](https://omniedge.io/docs/article/Cases/jetson)
- [Display and control your Android device with Omniedge from anywhere on MacOS, Windows and Linux](https://omniedge.io/docs/article/Cases/android-remote)
- [Talk to your family and share photos in a LAN on the internet](https://omniedge.io/docs/article/Cases/lan-messenger)
- [Air Drop Any Files between MacOS, Windows, Routers, Linux and Android with Omniedge from anywhere](https://omniedge.io/docs/article/Cases/landrop)

## قارن

- [VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/vpn-vs-omniedge)
- [Express VPN vs. OmniEdge](https://omniedge.io/docs/article/compare/expressvpn-vs-omniedge)
- [frp/ngrok vs. OmniEdge](https://omniedge.io/docs/article/compare/frp-ngrok-vs-omniedge)
- [ZeroTier vs. OmniEdge](https://omniedge.io/docs/article/compare/zerotier-vs-omniedge)
- [n2n vs. OmniEdge](https://omniedge.io/docs/article/compare/n2n-vs-omniedge)

## من يتحدث عنا

- [Founded by a Single Tweet Startup OmniEdge’s effort to let connect without concern](https://threat.technology/founded-by-a-single-tweet-startup-omniedges-effort-to-let-connect-without-concern/)
- [voonze: OmniEdge, to access your Intranet from the Internet using P2P](https://voonze.com/omniedge-to-access-your-intranet-from-the-internet-using-p2p/)
- [wwwhatsnew: OMNIEDGE, PARA ACCEDER A TU INTRANET DESDE INTERNET USANDO P2P](https://wwwhatsnew.com/2022/03/03/omniedge-para-acceder-a-tu-intranet-desde-internet-usando-p2p/)
- [l'Entrepreneur: OmniEdge, pour accéder à votre Intranet depuis Internet en P2P](https://lentrepreneur.co/style/technologie/omniedge-pour-acceder-a-votre-intranet-depuis-internet-en-p2p-04032022)
- [RunaCapital: Awesome OSS alternatives](https://github.com/RunaCapital/awesome-oss-alternatives)
- [OmniEdge in ntopconf 2022](https://www.ntop.org/ntopconf2022/)

## Advisor

[lucaderi](https://github.com/lucaderi)

## Contributors

[harri8807](https://github.com/orgs/omniedgeio/people/harri8807) , [Tex-Tang](https://github.com/Tex-Tang), [ivyxjc](https://github.com/orgs/omniedgeio/people/ivyxjc), [kidylee](https://github.com/kidylee), [EbenDang](https://github.com/orgs/omniedgeio/people/EbenDang)
,[zteshadow](https://github.com/zteshadow), [ChenYouping](https://github.com/orgs/omniedgeio/people/ChenYouping),[ddrandy](https://github.com/orgs/omniedgeio/people/ddrandy), **Tsingv**, [mtx2d](https://github.com/mtx2d)，[Blackrose](https://github.com/Blackrose), [cheung-chifung](https://github.com/cheung-chifung),[我不是矿神](https://imnks.com/5768.html)


> لا تتردد في إخبارنا بأي منشورات تتعلق بنا عبر المشكلة أو العلاقات العامة.

----

إذا كان لديك المزيد من الأسئلة ، فلا تتردد في التحدث إلينا على [Discussions](https://github.com/omniedgeio/omniedge/discussions).