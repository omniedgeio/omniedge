# OmniEdge

> Secure P2P mesh networking for AI devices, IoT, and edge computing

<!-- Releases & Build Status -->
[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/releases)
[![CLI Release](https://github.com/omniedgeio/omniedge/actions/workflows/release.yml/badge.svg)](https://github.com/omniedgeio/omniedge/actions/workflows/release.yml)
[![Desktop Release](https://github.com/omniedgeio/omniedge/actions/workflows/desktop-release.yml/badge.svg)](https://github.com/omniedgeio/omniedge/actions/workflows/desktop-release.yml)
[![GitHub all releases](https://img.shields.io/github/downloads/omniedgeio/omniedge/total?label=Downloads&color=orange)](https://github.com/omniedgeio/omniedge/releases)
<br />

<!-- Tech Stack & Quality -->
[![Go Version](https://img.shields.io/github/go-mod/go-version/omniedgeio/omniedge)](go.mod)
[![Wails Version](https://img.shields.io/badge/Wails-v3.0.0--alpha-red)](shell/desktop/go.mod)
[![Go Report Card](https://goreportcard.com/badge/github.com/omniedgeio/omniedge)](https://goreportcard.com/report/github.com/omniedgeio/omniedge)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge)](LICENSE)
<br />

<!-- Platforms & Architecture -->
[![Platforms](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS%20%7C%20Windows%20%7C%20FreeBSD%20%7C%20Android-blue)](#)
[![Arch](https://img.shields.io/badge/Arch-amd64%20%7C%20arm64%20%7C%20armv7%20%7C%20riscv64%20%7C%20mips-lightgrey)](#)
<br />

<!-- Community & Activity -->
[![GitHub Stars](https://img.shields.io/github/stars/omniedgeio/omniedge?style=social)](https://github.com/omniedgeio/omniedge/stargazers)
[![Twitter Follow](https://img.shields.io/twitter/follow/omniedgeio?style=social)](https://twitter.com/omniedgeio)
[![Discord](https://img.shields.io/discord/1079361536739770368?label=Discord&logo=discord&logoColor=white)](https://connect.omniedge.io/discord)
[![GitHub issues](https://img.shields.io/github/issues/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/issues)
[![GitHub last commit](https://img.shields.io/github/last-commit/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/commits)

OmniEdge enables seamless connectivity between your devices across networks. Perfect for:
- 🤖 **AI/ML Devices**: NVIDIA Jetson, AI edge computers
- 🍓 **Raspberry Pi**: All models from Pi 3 to Pi 5
- 📡 **OpenWrt Routers**: Mesh your network infrastructure
- 🖥️ **Servers**: Linux, macOS, FreeBSD, Windows
- 🏭 **Industrial & Robotics**: Layer 2 VPN for real-time protocols (EtherCAT, PROFINET, etc.) in Humanoid Robots

## Highlights

- 🖥️ **Cross-platform Desktop App** - Windows, macOS, Linux with system tray
- 📦 **Multiple Linux Packages** - DEB, RPM, AppImage, Flatpak, Arch
- 🪟 **Windows Support** - NSIS installer with bundled TAP driver
- 🍎 **Universal macOS Support** - Native DMG/CLI for both Apple Silicon and Intel Macs
- 🛡️ **Verified Stability** - Every release is automatically tested via real-world connectivity probes
- 💓 **Real-time heartbeat** - Device online status visible in dashboard
- 🏗️ **Emerging architectures** - RISC-V, LoongArch, FreeBSD 14

[🌐 Website](https://connect.omniedge.io) • [📚 Docs](https://connect.omniedge.io/docs) • [💬 Discord](https://discord.gg/d4faRPYj) • [🐦 Twitter](https://twitter.com/omniedgeio)

## Quick Install (CLI)

The easiest way to install OmniEdge CLI:

```bash
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/refs/heads/main/omniedge-install.sh | bash
```

To install a specific version(beta/rc):

```bash
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/refs/heads/main/omniedge-install.sh | OMNIEDGE_VERSION=v1.0.1 bash
```

## Desktop Applications

Download the latest desktop app from the [Releases page](https://github.com/omniedgeio/omniedge/releases/latest).

| Platform | Package | Filename |
|----------|---------|----------|
| **Windows** | NSIS Installer | `omniedge-desktop-{version}-windows-amd64.exe` |
| **macOS** | DMG (Apple Silicon) | `omniedge-desktop-{version}-macos-arm64.dmg` |
| **macOS** | DMG (Intel) | `omniedge-desktop-{version}-macos-amd64.dmg` |
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
| **macOS CLI** (Intel) | `omniedge-{version}-macos-amd64.zip` |
| **FreeBSD 14** | `omniedge-{version}-freebsd-14.zip` |
| **RISC-V** | `omniedge-{version}-riscv64.zip` |
| **LoongArch** | `omniedge-{version}-loongarch64.zip` |

## Usage

The CLI is now fully automated. Running `start` will handle login, network selection, and background daemonization.

### Start & Connection

```bash
# Basic start - trigger login and interactive network selection
omniedge start

# Connect to a specific network directly
omniedge start -n "your-network-id"

# Act as an Exit Node
omniedge start --as-exit-node

# Route traffic via an Exit Node IP
omniedge start -e "100.64.0.1"
```

### Management

```bash
# Show current connection status, IP, and PID
omniedge status

# Disconnect and stop background service
omniedge stop
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
