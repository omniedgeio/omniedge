# OmniEdge

> Secure P2P mesh networking for AI devices, IoT, and edge computing

[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/releases)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge)](LICENSE)

OmniEdge enables seamless connectivity between your devices across networks. Perfect for:
- 🤖 **AI/ML Devices**: NVIDIA Jetson, AI edge computers
- 🍓 **Raspberry Pi**: All models from Pi 3 to Pi 5
- 📡 **OpenWrt Routers**: Mesh your network infrastructure
- 🖥️ **Servers**: Linux, macOS, FreeBSD, Windows

## Highlights

- 🖥️ **Cross-platform Desktop App** - Windows, macOS, Linux with native GUI
- 📦 **Multiple Linux Packages** - DEB, RPM, AppImage, Flatpak, Arch
- 🪟 **Windows Support** - NSIS installer with bundled TAP driver
- ✨ **Native macOS `utun` support** - No third-party kernel extensions needed
- 💓 **Real-time heartbeat** - Device online status visible in dashboard
- 🏗️ **Emerging architectures** - RISC-V, LoongArch, FreeBSD 14

[🌐 Website](https://connect.omniedge.io) • [📚 Docs](https://connect.omniedge.io/docs) • [💬 Discord](https://discord.gg/d4faRPYj) • [🐦 Twitter](https://twitter.com/omniedgeio)

## Quick Install (CLI)

The easiest way to install OmniEdge CLI:

```bash
curl -fsSL https://connect.omniedge.io/install/omniedge-install.sh | bash
```

To install a specific version:

```bash
curl -fsSL https://connect.omniedge.io/install/omniedge-install.sh | OMNIEDGE_VERSION=v1.0.0 bash
```

## Desktop Applications

Download the latest desktop app from the [Releases page](https://github.com/omniedgeio/omniedge/releases/latest).

| Platform | Package | Filename |
|----------|---------|----------|
| **Windows** | NSIS Installer | `omniedge-desktop-{version}-windows-amd64.exe` |
| **macOS** | DMG (Apple Silicon) | `omniedge-desktop-{version}-macos-arm64.dmg` |
| **Linux** | DEB (Ubuntu/Debian) | `omniedge-desktop-{version}-linux-amd64.deb` |
| **Linux** | RPM (Fedora/RHEL) | `omniedge-desktop-{version}-linux-amd64.rpm` |
| **Linux** | AppImage (Universal) | `omniedge-desktop-{version}-linux-amd64.AppImage` |
| **Linux** | Flatpak | `omniedge-desktop-{version}-linux-amd64.flatpak` |
| **Linux** | Arch (AUR) | `omniedge-desktop-{version}-linux-amd64-arch.tar.gz` |

## CLI Binaries

Download CLI binaries from the [Releases page](https://github.com/omniedgeio/omniedge/releases/latest). Filenames follow the format `omniedge-{version}-{platform}.zip`.

### Linux (Native)
| Architecture | Devices | Filename |
|--------------|---------|----------|
| **amd64** | Servers, NUCs, Mini PCs | `omniedge-{version}-amd64.zip` |
| **arm64** | NVIDIA Jetson, RPi 4/5 | `omniedge-{version}-arm64.zip` |
| **arm** | Raspberry Pi 3, IoT Gateways | `omniedge-{version}-arm.zip` |

### OpenWrt (Routers)
| Architecture | Devices | Filename |
|--------------|---------|----------|
| **amd64** | x86 Software Routers | `omniedge-{version}-openwrt-amd64.zip` |
| **arm64** | Modern ARM Routers | `omniedge-{version}-openwrt-arm64.zip` |
| **arm** | Qualcomm IPQ40xx | `omniedge-{version}-openwrt-arm.zip` |
| **mips** | Legacy MIPS Routers | `omniedge-{version}-openwrt-mips.zip` |
| **mipsle** | MediaTek Routers | `omniedge-{version}-openwrt-mipsle.zip` |

### Other Platforms
| Platform | Filename |
|----------|----------|
| **macOS CLI** (Apple Silicon) | `omniedge-{version}-macos-arm64.zip` |
| **FreeBSD 14** | `omniedge-{version}-freebsd-14.zip` |
| **RISC-V** | `omniedge-{version}-riscv64.zip` |
| **LoongArch** | `omniedge-{version}-loongarch64.zip` |

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
