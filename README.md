# OmniEdge

> Secure P2P mesh networking for AI devices, IoT, and edge computing

[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge-cli)](https://github.com/omniedgeio/omniedge/releases)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge-cli)](LICENSE)

OmniEdge enables seamless connectivity between your devices across networks. Perfect for:
- 🤖 **AI/ML Devices**: NVIDIA Jetson, AI edge computers
- 🍓 **Raspberry Pi**: All models from Pi 3 to Pi 5
- 📡 **OpenWrt Routers**: Mesh your network infrastructure
- 🖥️ **Servers**: Linux, macOS, FreeBSD, Windows

[🌐 Website](https://connect.omniedge.io) • [📚 Docs](https://connect.omniedge.io/docs) • [💬 Discord](https://discord.gg/d4faRPYj) • [🐦 Twitter](https://twitter.com/omniedgeio)

## Quick Install

```bash
curl https://connect.omniedge.io/install/omniedge-install.sh | bash
```

## Desktop Applications

| Platform | Package | Download |
|----------|---------|----------|
| **Windows** | NSIS Installer | [omniedge-desktop-windows-amd64.exe](https://github.com/omniedgeio/omniedge/releases/latest) |
| **macOS** | DMG (Apple Silicon) | [omniedge-desktop-macos-arm64.dmg](https://github.com/omniedgeio/omniedge/releases/latest) |
| **Linux** | DEB (Ubuntu/Debian) | [omniedge-desktop-linux-amd64.deb](https://github.com/omniedgeio/omniedge/releases/latest) |
| **Linux** | RPM (Fedora/RHEL) | [omniedge-desktop-linux-amd64.rpm](https://github.com/omniedgeio/omniedge/releases/latest) |
| **Linux** | AppImage (Universal) | [omniedge-desktop-linux-amd64.AppImage](https://github.com/omniedgeio/omniedge/releases/latest) |
| **Linux** | Flatpak | [omniedge-desktop-linux-amd64.flatpak](https://github.com/omniedgeio/omniedge/releases/latest) |
| **Linux** | Arch (AUR) | [omniedge-desktop-linux-arch.tar.gz](https://github.com/omniedgeio/omniedge/releases/latest) |

## CLI Binaries

### Linux (Native)
| Architecture | Devices | Download |
|--------------|---------|----------|
| **amd64** | Servers, NUCs, Mini PCs | [omniedge-amd64.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **arm64** | NVIDIA Jetson, RPi 4/5, Apple Silicon | [omniedge-arm64.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **arm** | Raspberry Pi 3, IoT Gateways | [omniedge-arm.zip](https://github.com/omniedgeio/omniedge/releases/latest) |

### OpenWrt (Routers)
| Architecture | Devices | Download |
|--------------|---------|----------|
| **amd64** | x86 Software Routers | [omniedge-openwrt-amd64.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **arm64** | Modern ARM Routers | [omniedge-openwrt-arm64.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **arm** | Qualcomm IPQ40xx | [omniedge-openwrt-arm.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **mips** | Legacy MIPS Routers | [omniedge-openwrt-mips.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **mipsle** | MediaTek Routers | [omniedge-openwrt-mipsle.zip](https://github.com/omniedgeio/omniedge/releases/latest) |

### Other Platforms
| Platform | Download |
|----------|----------|
| **macOS CLI** (Apple Silicon) | [omniedge-macos-arm64.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **FreeBSD 14** | [omniedge-freebsd-14.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **RISC-V** | [omniedge-riscv64.zip](https://github.com/omniedgeio/omniedge/releases/latest) |
| **LoongArch** | [omniedge-loongarch64.zip](https://github.com/omniedgeio/omniedge/releases/latest) |

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

## Mobile Apps

Coming soon...

## Documentation

- [Architecture](https://connect.omniedge.io/docs/article/architecture)
- [Installation Guide](https://connect.omniedge.io/docs/article/install)
- [Use Cases](https://connect.omniedge.io/docs/article/cases)

## Contributing

See [CONTRIBUTING.md](.github/CONTRIBUTING.md) for guidelines.

## License

[GPL-3.0](LICENSE)

---

Built with ❤️ by [OmniEdge](https://connect.omniedge.io)
