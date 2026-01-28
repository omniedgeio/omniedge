# OmniEdge

> Secure P2P mesh networking for AI devices, IoT, and edge computing

[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge?style=flat-square)](https://github.com/omniedgeio/omniedge/releases)
[![Release CLI](https://img.shields.io/github/actions/workflow/status/omniedgeio/omniedge/release.yml?label=CLI%20Build&style=flat-square)](https://github.com/omniedgeio/omniedge/actions/workflows/release.yml)
[![Release Desktop](https://img.shields.io/github/actions/workflow/status/omniedgeio/omniedge/desktop-release.yml?label=Desktop%20Build&style=flat-square)](https://github.com/omniedgeio/omniedge/actions/workflows/desktop-release.yml)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-blue?style=flat-square)](#supported-platforms)
[![Discord](https://img.shields.io/discord/1234567890?color=5865F2&label=discord&logo=discord&logoColor=white&style=flat-square)](https://discord.gg/d4faRPYj)

OmniEdge enables seamless connectivity between your devices across networks. Perfect for:
- 🤖 **AI/ML Devices**: NVIDIA Jetson, AI edge computers
- 🍓 **Raspberry Pi**: All models from Pi 3 to Pi 5
- 📡 **OpenWrt Routers**: Mesh your network infrastructure
- 🖥️ **Servers**: Linux, macOS, Windows

## V2 Migration & License Notice

OmniEdge has transitioned from its legacy Go-based implementation (using n2n and licensed under GPL-3) to a modern, high-performance Rust-based architecture (using OmniNervous and dual-licensed under MIT/Apache-2.0).

-   **New Repository (Main)**: This repository now contains the V2 Rust implementation.
-   **Legacy Repository**: The previous Go/n2n implementation (GPL-3) is preserved at [omniedgeio/omniedge-legacy](https://github.com/omniedgeio/omniedge-legacy).

## Highlights

- 🖥️ **Cross-platform Desktop App** - Windows, macOS, Linux with system tray
- 📦 **Multiple Linux Packages** - DEB, RPM, AppImage, Flatpak, Arch
- 🪟 **Windows Support** - NSIS installer with bundled TAP driver
- 🍎 **Universal macOS Support** - Native DMG/CLI for both Apple Silicon and Intel Macs
- [x] **Verified Stability** - Every release is automatically tested via real-world connectivity probes
- [x] **Rust Core** - Built for performance and safety with a modular architecture
- [x] **Real-time heartbeat** - Device online status visible in dashboard
- [x] **Emerging architectures** - RISC-V 64, ARMv7

[🌐 Website](https://connect.omniedge.io) • [📚 Docs](https://connect.omniedge.io/docs) • [💬 Discord](https://discord.gg/d4faRPYj) • [🐦 Twitter](https://twitter.com/omniedgeio)

## Supported Platforms

### CLI (`omniedge-cli`)

| Platform | Architecture | Package Formats |
|----------|--------------|-----------------|
| **Linux** | x86_64 | `.tar.gz`, `.deb`, `.rpm`, `.AppImage` |
| **Linux** | ARM64 (aarch64) | `.tar.gz`, `.deb`, `.rpm` |
| **Linux** | ARMv7 | `.tar.gz`, `.deb` |
| **Linux** | RISC-V 64 | `.tar.gz`, `.deb` |
| **macOS** | x86_64 (Intel) | `.tar.gz` |
| **macOS** | ARM64 (Apple Silicon) | `.tar.gz` |
| **Windows** | x86_64 | `.zip` |

### Desktop (`omniedge-desktop`)

| Platform | Architecture | Package Formats |
|----------|--------------|-----------------|
| **Windows** | x86_64 | `.msi`, `.exe` |
| **macOS** | x86_64 (Intel) | `.dmg` |
| **macOS** | ARM64 (Apple Silicon) | `.dmg` |
| **Linux** | x86_64 | `.deb`, `.AppImage` |

## Quick Install (CLI)

The easiest way to install OmniEdge CLI:

```bash
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/refs/heads/main/scripts/omniedge-install.sh | bash
```

### Package Manager Installation

**Debian/Ubuntu (.deb):**
```bash
# Download the latest release
wget https://github.com/omniedgeio/omniedge/releases/latest/download/omniedge-cli_VERSION_amd64.deb
sudo dpkg -i omniedge-cli_*_amd64.deb
```

**Fedora/RHEL/openSUSE (.rpm):**
```bash
wget https://github.com/omniedgeio/omniedge/releases/latest/download/omniedge-cli-VERSION-1.x86_64.rpm
sudo rpm -i omniedge-cli-*.x86_64.rpm
```

**AppImage (Universal Linux):**
```bash
wget https://github.com/omniedgeio/omniedge/releases/latest/download/omniedge-cli-VERSION-x86_64.AppImage
chmod +x omniedge-cli-*.AppImage
./omniedge-cli-*.AppImage
```

**macOS (Homebrew coming soon):**
```bash
# Download and extract
curl -LO https://github.com/omniedgeio/omniedge/releases/latest/download/omniedge-cli-macos-arm64.tar.gz
tar -xzf omniedge-cli-macos-arm64.tar.gz
sudo mv omniedge-cli-macos-arm64 /usr/local/bin/omniedge
```

## CLI Usage

After installation, OmniEdge runs as a background service on all platforms:

```bash
# Start OmniEdge (login and connect to first network)
omniedge start

# Start with a specific network
omniedge start -n <network_id>

# Login with security key (non-interactive, ideal for CI/CD and automation)
omniedge start -s <security_key>
omniedge start -n <network_id> -s <security_key>

# Run as an exit node (allow others to route traffic through this node)
omniedge start -x
omniedge start --as-exit-node

# Use a specific exit node
omniedge start -e <exit_node_ip>
omniedge start --exit-node <exit_node_ip>

# Stop OmniEdge
omniedge stop

# Scan local network and upload results
omniedge scan -c 192.168.1.0/24
```

### Operating Modes

OmniEdge supports three operating modes via `--mode`:

| Mode | Description | Auth Required | VPN Tunnel | Signaling Server |
|------|-------------|---------------|------------|------------------|
| **edge** (default) | Regular VPN client | Yes | Yes | No |
| **nucleus** | Signaling server only | No | No | Yes |
| **dual** | VPN client + signaling | Yes | Yes | Yes |

```bash
# EDGE MODE (default) - Regular VPN client
omniedge start -n <network_id>

# NUCLEUS MODE - Standalone signaling server (no VPN, no login required)
# Requires --secret for cluster authentication (min 16 chars)
omniedge start --mode nucleus --secret "MySecretMin16Chars"
omniedge start --mode nucleus --port 51821 --secret "MySecretMin16Chars"

# DUAL MODE - VPN client + nucleus signaling server
# Acts as both an edge client AND a signaling server for other peers
omniedge start -n <network_id> --mode dual --secret "MySecretMin16Chars"
omniedge start -n <network_id> --mode dual --port 51821 --secret "MySecretMin16Chars"

# Full mesh relay: dual mode + exit node
# This node becomes a central hub for signaling AND traffic routing
omniedge start -n <network_id> --mode dual --secret "MySecret123456789" -x
```

### Nucleus Mode

In **nucleus mode**, OmniEdge runs as a standalone UDP signaling server:

- **No VPN tunnel** - Does not create a network interface
- **No authentication** - Does not require OmniEdge account login
- **Lightweight** - Just handles peer discovery and NAT traversal signaling
- **Cluster secret** - All clients must use the same secret to connect

**Use cases:**
- Self-hosted signaling without cloud dependency
- Private/air-gapped networks
- Low-latency local cluster signaling

### Dual Mode

In **dual mode**, OmniEdge combines both capabilities:

1. **Edge Client**: Connects to the network with a virtual IP, communicates with peers
2. **Signaling Server**: Listens on UDP port (default 51820) for peer discovery requests

This allows a stable node (e.g., a server with a public IP) to act as a nucleus for other nodes in the network while also participating as a peer.

**Use cases:**
- Self-hosted mesh with full participation
- Central hub that can also be reached directly
- Exit node + signaling server combo

### Background Service

When you run `omniedge start`, it automatically runs in the background:

| Platform    | Service Type          | Management                        |
| ----------- | --------------------- | --------------------------------- |
| **Windows** | Windows Service       | `sc stop OmniEdge`                |
| **Linux**   | systemd               | `systemctl status omniedge`       |
| **macOS**   | launchd (LaunchAgent) | `launchctl list io.omniedge.cli`  |

If the OmniEdge Desktop helper service is already running, the CLI will use it instead of creating a separate service.

## Desktop Applications

The new Desktop application is built with Tauri v2 and React. 

| Platform    | Package            | Architecture                |
| ----------- | ------------------ | --------------------------- |
| **Windows** | MSI/EXE            | x64, ARM64                  |
| **macOS**   | DMG/APP            | Universal (Silicon & Intel) |
| **Linux**   | AppImage, DEB, RPM | x64, ARM64                  |

## Architecture

OmniEdge is built in pure Rust for maximum efficiency and safety, leveraging the **OmniNervous** daemon for peer-to-peer connectivity.

- **omni-core**: Unified connection management and state machine.
- **omninervous**: High-performance P2P orchestration daemon.
- **omni-tun**: Platform-specific WireGuard TUN interface management.
- **omni-api**: Performance API client for the OmniEdge control plane.
- **ui/desktop**: Modern desktop shell using Tauri v2.

See [doc/ARCHITECTURE.md](doc/ARCHITECTURE.md) for details.

## License

Dual-licensed under **Apache License 2.0** and **MIT License**. See [LICENSE](LICENSE) and [LICENSING.md](LICENSING.md) for full details.

---

Built with ❤️ by [OmniEdge](https://connect.omniedge.io)
