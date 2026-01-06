# OmniEdge CLI

> Secure P2P mesh networking for AI devices, IoT, and edge computing

[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge-cli)](https://github.com/omniedgeio/omniedge-cli/releases)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge-cli)](LICENSE)

OmniEdge CLI enables seamless connectivity between your devices across networks. Perfect for:
- 🤖 **AI/ML Devices**: NVIDIA Jetson, AI edge computers
- 🍓 **Raspberry Pi**: All models from Pi 3 to Pi 5
- 📡 **OpenWrt Routers**: Mesh your network infrastructure
- 🖥️ **Servers**: Linux, macOS, FreeBSD

[🌐 Website](https://omniedge.io) • [� Docs](https://omniedge.io/docs) • [� Discord](https://discord.gg/d4faRPYj) • [🐦 Twitter](https://twitter.com/omniedgeio)

## Quick Install

```bash
curl https://omniedge.io/install/omniedge-install.sh | bash
```

## Supported Platforms

### Linux (Native)
| Architecture | Devices | Download |
|--------------|---------|----------|
| **amd64** | Servers, NUCs, Mini PCs | [omniedge-amd64.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **arm64** | NVIDIA Jetson, RPi 4/5, Apple Silicon | [omniedge-arm64.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **arm** | Raspberry Pi 3, IoT Gateways | [omniedge-arm.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |

### OpenWrt (Routers)
| Architecture | Devices | Download |
|--------------|---------|----------|
| **amd64** | x86 Software Routers | [omniedge-openwrt-amd64.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **arm64** | Modern ARM Routers | [omniedge-openwrt-arm64.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **arm** | Qualcomm IPQ40xx | [omniedge-openwrt-arm.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **mips** | Legacy MIPS Routers | [omniedge-openwrt-mips.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **mipsle** | MediaTek Routers | [omniedge-openwrt-mipsle.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |

### Emerging Architectures
| Architecture | Devices | Download |
|--------------|---------|----------|
| **riscv64** | Sipeed, StarFive, VisionFive | [omniedge-riscv64.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **loongarch64** | Loongson (China) | [omniedge-loongarch64.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |

### Desktop/Workstation
| Platform | Download |
|----------|----------|
| **macOS** (Apple Silicon) | [omniedge-macos-latest.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **Ubuntu 22.04 LTS** | [omniedge-ubuntu-22.04.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **Ubuntu 24.04 LTS** | [omniedge-ubuntu-24.04.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |
| **FreeBSD 14** | [omniedge-freebsd-14.zip](https://github.com/omniedgeio/omniedge-cli/releases/latest) |

## Usage

### Login

```bash
# Login with email
omniedge login -u your@email.com

# Login with API key (recommended for automation)
omniedge login -s YOUR_SECRET_KEY
```

### Join Network

```bash
# Interactive mode - choose network from list
sudo omniedge join

# Direct mode - specify network ID
sudo omniedge join -n "your-network-id"
```

## Other Clients

- [📱 iOS & M1 Mac](https://apps.apple.com/us/app/omniedgenew/id1603005893) - App Store
- [🤖 Android](https://omniedge.io/download/android) - APK Download
- [🪟 Windows](https://omniedge.io/download/windows) - Installer
- [🔌 Synology NAS](https://omniedge.io/download/synology) - Package

## v0.3.0 Highlights

- ✨ **Native macOS `utun` support** - No third-party kernel extensions needed
- 💓 **Real-time heartbeat** - Device online status visible in dashboard
- 🏗️ **Modern toolchain** - Go 1.21, OpenWrt SDK 23.05
- 🌏 **Emerging architectures** - RISC-V and LoongArch support

## Documentation

- [Architecture](https://omniedge.io/docs/article/architecture)
- [Installation Guide](https://omniedge.io/docs/article/install)
- [Use Cases](https://omniedge.io/docs/article/cases)
- [Performance](https://omniedge.io/docs/article/performance)

## Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md) for guidelines.

## License

[GPL-3.0](LICENSE)

---

Built with ❤️ by [OmniEdge](https://omniedge.io)
