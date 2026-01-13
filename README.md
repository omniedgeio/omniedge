# OmniEdge

> Secure P2P Mesh Networking for Humanoid Robots, Edge AI, and Industrial Automation.

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
[![Discord](https://img.shields.io/discord/1079361536739770368?label=Discord&logo=discord&logoColor=white)](https://discord.gg/afGrMMtN)
[![GitHub issues](https://img.shields.io/github/issues/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/issues)
[![GitHub last commit](https://img.shields.io/github/last-commit/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/commits)

OmniEdge enables seamless, low-latency connectivity between devices across any network. It is specifically optimized for:
- 🤖 **Humanoid Robotics**: Ultra-low latency P2P for real-time control and sensor fusion.
- � **Industrial Automation**: Native Layer 2 VPN support for **EtherCAT**, **PROFINET**, and **EtherNet/IP**.
- � **Robot Operating System (ROS2)**: Full support for multicast/broadcast discovery across remote sites.
- 🧠 **Edge AI**: NVIDIA Jetson, Orin, Thor,and dedicated AI edge computers.
- 📡 **Infrastructure**: OpenWrt routers, 5G/4G gateways, and Raspberry Pi (3/4/5).

- 🌐 **True P2P Connectivity** - Direct device-to-device communication using high-performance NAT traversal.
- 🔗 **Native Layer 2 Support** - Bridging Ethernet frames for industrial protocols and legacy applications.
- 🖥️ **Desktop Tray App** - Seamless management on Windows, macOS, and Linux (DEB, RPM, AppImage, Flatpak).
- 🍎 **Universal macOS Support** - Native binaries for both Apple Silicon (arm64) and Intel (x86_64).
- 🛡️ **Zero-Config Security** - AES-256 encrypted tunnels with certificate-based authentication.
- 🏗️ **Multi-Arch Support** - Optimized for RISC-V, LoongArch, ARMv7, and FreeBSD 14.

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

Distributed under the [GPL-3.0 License](LICENSE).

---

Built with ❤️ by [OmniEdge](https://connect.omniedge.io)
