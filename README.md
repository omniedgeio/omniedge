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
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/refs/heads/main/omniedge-install.sh | bash
```

## Desktop Applications

The new Desktop application is built with Tauri v2 and React. 

| Platform | Package | Architecture |
|----------|---------|--------------|
| **Windows** | MSI/EXE | x64, ARM64 |
| **macOS** | DMG/APP | Universal (Silicon & Intel) |
| **Linux** | AppImage, DEB, RPM | x64, ARM64 |

## Architecture

OmniEdge is now built in pure Rust for maximum efficiency and safety.

- **omni-core**: Unified connection management and state machine.
- **omni-tun**: Platform-specific WireGuard TUN interface management.
- **omni-api**: High-performance API client for the OmniEdge control plane.
- **ui/desktop**: Modern desktop shell using Tauri v2.

See [doc/ARCHITECTURE.md](doc/ARCHITECTURE.md) for details.

## License

Dual-licensed under **Apache License 2.0** and **MIT License**. See [LICENSE](LICENSE) and [LICENSING.md](LICENSING.md) for full details.

---

Built with ❤️ by [OmniEdge](https://connect.omniedge.io)
