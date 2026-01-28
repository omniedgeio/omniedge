# OmniEdge

> Secure P2P mesh networking for AI devices, IoT, and edge computing

[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge)](https://github.com/omniedgeio/omniedge/releases)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge)](LICENSE)

OmniEdge enables seamless connectivity between your devices across networks. Perfect for:
- 🤖 **AI/ML Devices**: NVIDIA Jetson, AI edge computers
- 🍓 **Raspberry Pi**: All models from Pi 3 to Pi 5
- 📡 **OpenWrt Routers**: Mesh your network infrastructure
- 🖥️ **Servers**: Linux, macOS, FreeBSD, Windows

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
- [x] **Emerging architectures** - RISC-V, LoongArch, FreeBSD 14

[🌐 Website](https://connect.omniedge.io) • [📚 Docs](https://connect.omniedge.io/docs) • [💬 Discord](https://discord.gg/d4faRPYj) • [🐦 Twitter](https://twitter.com/omniedgeio)

## Quick Install (CLI)

The easiest way to install OmniEdge CLI:

```bash
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/refs/heads/main/scripts/omniedge-install.sh | bash
```

## CLI Usage

After installation, OmniEdge runs as a background service on all platforms:

```bash
# Start OmniEdge (login and connect to first network)
omniedge start

# Start with a specific network
omniedge start -n <network_id>

# Run as a nucleus (signaling controller for mesh network)
omniedge start -N
omniedge start --nucleus

# Run as an exit node (allow others to route traffic through this node)
omniedge start -x
omniedge start --as-exit-node

# Combine options: nucleus + exit node on specific network
omniedge start -n <network_id> -N -x

# Use a specific exit node
omniedge start -e <exit_node_ip>
omniedge start --exit-node <exit_node_ip>

# Login with security key (non-interactive)
omniedge start -s <security_key>

# Stop OmniEdge
omniedge stop

# Scan local network and upload results
omniedge scan -c 192.168.1.0/24
```

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
