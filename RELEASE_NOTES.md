# OmniEdge Release Notes

## v2.0.0 (2026-01-29)

### Complete Rust Rewrite

OmniEdge 2.0 is a **complete ground-up rewrite** from Go/n2n to pure Rust, delivering a modern, high-performance mesh VPN with significantly improved architecture.

### Performance: Industrial-Grade Stability

Validated through [50-run longitudinal testing](https://github.com/omniedgeio/OmniNervous/blob/main/Capability_test/cloud_test_50_run_paper.md) using Process Capability Analysis (Cpk):

| Metric                      | OmniEdge Tunnel    | Raw Internet | Improvement          |
| --------------------------- | ------------------ | ------------ | -------------------- |
| **Latency**                 | 54.69ms            | 54.36ms      | +0.3ms overhead      |
| **Latency Stability (Cpk)** | **2.92 (6-Sigma)** | 6.47         | Near-deterministic   |
| **Throughput**              | **484.7 Mbps**     | 344.1 Mbps   | **+140.8%**          |
| **Jitter (StdDev)**         | 0.057ms            | 0.026ms      | Bounded, predictable |

> **What this means**: Cpk > 2.0 indicates industrial-grade process capability. OmniEdge provides deterministic, jitter-controlled networking suitable for real-time robot control and latency-sensitive AI inference.

### Architecture Changes

| Component  | v1.x (Legacy)       | v2.0 (New)                    |
| ---------- | ------------------- | ----------------------------- |
| Language   | Go + C (n2n)        | Pure Rust                     |
| Protocol   | n2n supernode/edge  | OmniNervous (WireGuard-based) |
| TUN Driver | tap-windows, tuntap | omni-tun (native)             |
| Desktop    | Wails v3            | Tauri v2 + React              |
| Signaling  | n2n supernode       | Nucleus server                |

### Three-Mode System

The CLI now supports three operational modes via `--mode`:

| Mode               | Description            | Auth Required | VPN Tunnel | Signaling Server |
| ------------------ | ---------------------- | ------------- | ---------- | ---------------- |
| **edge** (default) | Regular VPN client     | Yes           | Yes        | No               |
| **nucleus**        | Signaling server only  | No            | No         | Yes              |
| **dual**           | VPN client + signaling | Yes           | Yes        | Yes              |

```bash
# Edge mode (default) - VPN client
omniedge start -n <network_id>

# Nucleus mode - Standalone signaling server (no VPN, no login required)
# Secret is optional but recommended for production
omniedge start --mode nucleus --port 51821
omniedge start --mode nucleus --port 51821 --secret "MySecretMin16Chars"

# Dual mode - VPN client + nucleus signaling server
# Secret comes from backend API automatically
omniedge start -n <network_id> --mode dual
```

### New Features

- **Exit Node Support**: Route all traffic through a peer
  - `-x` / `--as-exit-node`: Advertise this device as an exit node
  - `--no-exit-node`: Disable exit node mode (if previously enabled)
  - `-e` / `--exit-node <ip>`: Use a specific peer as exit node
  - Exit node settings are persisted across restarts
- **Security Key Authentication**: Non-interactive login for automation
  - `omniedge start -s <security_key> -n <network_id>`
- **Background Service**: Native service integration
  - Windows: Windows Service
  - Linux: systemd unit file
  - macOS: launchd plist
- **Custom User Servers**: Users can configure their own nucleus/relay servers via dashboard
- **Enhanced Status Command**: Check connection status with `omniedge status`
  - Shows virtual IP, network, interface name, and exit node role
  - Displays running mode (edge/nucleus/dual)
  - Shows nucleus port and secret status when applicable
  - Displays live data from network interface
- **Flexible Nucleus Mode**: 
  - `--secret` is now optional for nucleus-only mode (warning shown if not provided)
  - Dual mode automatically uses backend secret from API
- **Docker-based E2E Testing**: Automated testing for nucleus server functionality

### Expanded Platform Support

| Platform | Architectures                   |
| -------- | ------------------------------- |
| Linux    | x86_64, aarch64, riscv64        |
| macOS    | x86_64, aarch64 (Apple Silicon) |
| Windows  | x86_64                          |
| FreeBSD  | x86_64, aarch64                 |

### New Packaging Formats

| Format     | Platforms             |
| ---------- | --------------------- |
| DEB        | amd64, arm64, riscv64 |
| RPM        | x86_64, aarch64       |
| AppImage   | x86_64, aarch64       |
| DMG        | macOS (x64, arm64)    |
| MSI        | Windows x64           |
| tar.gz/zip | All platforms         |

### Desktop Application

- **Tauri v2**: Modern, lightweight desktop framework
- **React + TypeScript**: Responsive UI
- **System Tray**: Quick access menu with network switching
- **Dynamic Positioning**: Window follows tray icon
- **Cross-Platform**: Windows, macOS, Linux

### Breaking Changes

- **Complete Protocol Change**: v2.0 is **not compatible** with v1.x networks
  - All devices in a network must upgrade to v2.0
- **Configuration Format**: New JSON-based config replaces legacy format
- **Architecture**: Complete rewrite from Go/n2n to Rust/OmniNervous
- **TUN Interface**: Interface names changed
  - Linux: `omniedge0` (was `edge0`)
  - macOS: `utun*` (unchanged)
  - Windows: `OmniEdge` (was `tap-omniedge`)

### Migration Guide

1. **Backup Configuration**: Export your network settings from dashboard
2. **Uninstall v1.x**: Remove old OmniEdge installation
3. **Install v2.0**: Download and install new version
4. **Re-authenticate**: Login with `omniedge login` or use security key
5. **Join Network**: Use `omniedge start -n <network_id>`

### Download Packages

#### CLI - Linux
| Package                                    | Architecture |
| ------------------------------------------ | ------------ |
| `omniedge-cli-v2.0.0-linux-x86_64.tar.gz`  | x86_64       |
| `omniedge-cli-v2.0.0-linux-aarch64.tar.gz` | ARM64        |
| `omniedge-cli-v2.0.0-linux-riscv64.tar.gz` | RISC-V 64    |

#### CLI - DEB Packages
| Package                          | Architecture         |
| -------------------------------- | -------------------- |
| `omniedge-cli_2.0.0_amd64.deb`   | Debian/Ubuntu x64    |
| `omniedge-cli_2.0.0_arm64.deb`   | Debian/Ubuntu ARM64  |
| `omniedge-cli_2.0.0_riscv64.deb` | Debian/Ubuntu RISC-V |

#### CLI - RPM Packages
| Package                            | Architecture      |
| ---------------------------------- | ----------------- |
| `omniedge-cli-2.0.0-1.x86_64.rpm`  | Fedora/RHEL x64   |
| `omniedge-cli-2.0.0-1.aarch64.rpm` | Fedora/RHEL ARM64 |

#### CLI - AppImage
| Package                               | Architecture |
| ------------------------------------- | ------------ |
| `omniedge-cli-2.0.0-x86_64.AppImage`  | Linux x64    |
| `omniedge-cli-2.0.0-aarch64.AppImage` | Linux ARM64  |

#### CLI - Other Platforms
| Package                                    | Platform            |
| ------------------------------------------ | ------------------- |
| `omniedge-cli-v2.0.0-macos-x86_64.tar.gz`  | macOS Intel         |
| `omniedge-cli-v2.0.0-macos-aarch64.tar.gz` | macOS Apple Silicon |
| `omniedge-cli-v2.0.0-windows-x86_64.zip`   | Windows x64         |

#### Desktop Applications
| Package                                       | Platform            |
| --------------------------------------------- | ------------------- |
| `omniedge-desktop-2.0.0-windows-x64.msi`      | Windows x64         |
| `omniedge-desktop-2.0.0-macos-x64.dmg`        | macOS Intel         |
| `omniedge-desktop-2.0.0-macos-arm64.dmg`      | macOS Apple Silicon |
| `omniedge-desktop-2.0.0-linux-amd64.deb`      | Linux x64 DEB       |
| `omniedge-desktop-2.0.0-linux-amd64.rpm`      | Linux x64 RPM       |
| `omniedge-desktop-2.0.0-linux-amd64.AppImage` | Linux x64 AppImage  |

### Installation

#### Quick Install (Linux/macOS)
```bash
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/refs/heads/main/scripts/omniedge-install.sh | bash
```

#### Package Managers
```bash
# Debian/Ubuntu
sudo dpkg -i omniedge-cli_2.0.0_amd64.deb

# Fedora/RHEL
sudo rpm -i omniedge-cli-2.0.0-1.x86_64.rpm

# AppImage
chmod +x omniedge-cli-2.0.0-x86_64.AppImage
./omniedge-cli-2.0.0-x86_64.AppImage
```

#### macOS
```bash
# Download and extract
curl -LO https://github.com/omniedgeio/omniedge/releases/latest/download/omniedge-cli-macos-arm64.tar.gz
tar -xzf omniedge-cli-macos-arm64.tar.gz
sudo mv omniedge-cli-macos-arm64 /usr/local/bin/omniedge

# Or use Homebrew (coming soon)
brew install omniedge
```

### Contributors

Thank you to all contributors who made this major release possible!

---

## v1.0.2-beta.0 (2026-01-12)
### 🚀 New Features & Improvements
- **Advanced Exit Node Support**: 
    - **Smart CLI Flags**: The `--as-exit-node` flag now automatically implies `-r` (routing), reducing command complexity.
    - **Cloud Synchronization**: Exit node selection (`-e`) now persists across sessions and synchronizes directly with the OmniEdge dashboard.
    - **Desktop Controls**: Introduced "Run as Exit Node" toggle and device selection menu to the system tray.
    - **Persistent State**: "Run as Exit Node" preference is now saved locally and restored on launch.
- **Desktop UI/UX Refinement**:
    - **Accessory Mode (macOS)**: Optimized the app to live exclusively in the menu bar, hiding the Dock icon for a less intrusive experience.
    - **Decoupled Loading**: Separated login and connection states to prevent UI flickering during network transitions.
    - **Stable Layout**: Enforced a 480px minimum height for the desktop window to ensure consistent content display.
    - **Universal Support**: Native performance on both **Intel (x64)** and **Apple Silicon (arm64)** macOS.
- **Reliability & Verification**:
    - **Automated Pulse Test**: Every release is now guarded by a mandatory connection test (login -> join -> ping) verifying real-world connectivity.
    - **Production Guard**: Standardized all release channels to use the production environment by default for representative testing.


---


## v1.0.1 (2026-01-09)
### 🚀 New Features
- **CLI Interactive Login**: The CLI now supports OAuth 2.0 Device Flow (`omniedge login`), allowing users to log in via browser without handling passwords directly.
- **Desktop Session Login**: Desktop application now uses seamless browser-based session login for improved security and experience.

### 🎨 Desktop UI/UX Polish
- **Refined Header**: Implemented the official OmniEdge logo and clean typography.
- **Identity Status**: Added a visual status indicator (green dot) to the user profile chip for clear connection feedback.
- **Active Network**: The currently connected virtual network is now clearly highlighted with a distinct background and indicator in the dashboard list.

### 🐛 Bug Fixes
- **Backend Stability**: Fixed a critical race condition (Error 1006) in the WebSocket service that caused login failures or "unexpected EOF" errors.
- **Login Flow**: Improved robustness of the browser-based login mechanism.

### 🔧 CI/CD Improvements
- **macOS Build**: Release workflow now correctly targets Apple Silicon (arm64) for optimized performance on modern Macs.

---

## v1.0.0 (2026-01-08)
Release Notes

**Release Date:** January 8, 2026

## 🎉 What's New

### Cross-Platform Desktop App
- **Windows**: NSIS installer with bundled TAP driver
- **macOS**: DMG package for Apple Silicon (arm64)
- **Linux**: DEB, RPM, AppImage, Flatpak, Arch packages

### Native macOS `utun` Support
- **Driverless experience**: No need for Tunnelblick or third-party kernel extensions
- **Apple Silicon optimized**: Native ARM64 support for M1/M2/M3/M4 Macs
- **L2/L3 bridge**: Seamless integration with n2n mesh protocol

### Real-Time Device Status
- **Heartbeat mechanism**: Devices now report status every minute
- **Dashboard visibility**: See which devices are online/offline
- **Automatic recovery**: Heartbeat resumes after network interruptions

### Expanded Platform Support
- **RISC-V (riscv64)**: Support for Sipeed, StarFive boards
- **LoongArch (loongarch64)**: Support for Loongson CPUs (China)
- **FreeBSD 14**: Updated from FreeBSD 13.1
- **Ubuntu 24.04 LTS**: Latest Ubuntu support

### Build Infrastructure
- **Go 1.21/1.23**: Modern Go toolchain
- **Wails v3**: Native desktop GUI framework
- **OpenWrt SDK 23.05**: Stable router SDK
- **GitHub Actions v4/v5**: Modern CI/CD

## 📦 Download Packages

### Desktop Apps
| Package                                          | Platform                 |
| ------------------------------------------------ | ------------------------ |
| `omniedge-desktop-1.0.0-windows-amd64.exe`       | Windows (NSIS Installer) |
| `omniedge-desktop-1.0.0-macos-arm64.dmg`         | macOS (Apple Silicon)    |
| `omniedge-desktop-1.0.0-linux-amd64.deb`         | Ubuntu/Debian            |
| `omniedge-desktop-1.0.0-linux-amd64.rpm`         | Fedora/RHEL              |
| `omniedge-desktop-1.0.0-linux-amd64.AppImage`    | Universal Linux          |
| `omniedge-desktop-1.0.0-linux-amd64.flatpak`     | Flatpak                  |
| `omniedge-desktop-1.0.0-linux-amd64-arch.tar.gz` | Arch Linux               |

### CLI - Linux
| Package                     | Architecture            |
| --------------------------- | ----------------------- |
| `omniedge-v1.0.0-amd64.zip` | x86_64 (Servers, NUCs)  |
| `omniedge-v1.0.0-arm64.zip` | ARM64 (Jetson, RPi 4/5) |
| `omniedge-v1.0.0-arm.zip`   | ARMv7 (RPi 3)           |

### CLI - OpenWrt
| Package                              | Architecture   |
| ------------------------------------ | -------------- |
| `omniedge-v1.0.0-openwrt-amd64.zip`  | x86_64 Routers |
| `omniedge-v1.0.0-openwrt-arm64.zip`  | ARM64 Routers  |
| `omniedge-v1.0.0-openwrt-arm.zip`    | ARMv7 Routers  |
| `omniedge-v1.0.0-openwrt-mips.zip`   | MIPS Routers   |
| `omniedge-v1.0.0-openwrt-mipsle.zip` | MIPSle Routers |

### CLI - Other Platforms
| Package                           | Platform                  |
| --------------------------------- | ------------------------- |
| `omniedge-v1.0.0-macos-arm64.zip` | macOS CLI (Apple Silicon) |
| `omniedge-v1.0.0-freebsd-14.zip`  | FreeBSD 14                |
| `omniedge-v1.0.0-riscv64.zip`     | RISC-V 64-bit             |
| `omniedge-v1.0.0-loongarch64.zip` | LoongArch 64-bit          |

## 🔧 Breaking Changes

- **macOS**: Now uses `utun` interface (appears as `utunX` instead of `tapX`)
- **macOS amd64**: Removed (n2n library is arch-specific, use arm64 for Apple Silicon)
- **Ubuntu 20.04**: Dropped due to runner availability (use Ubuntu 22.04+)
- **Ubuntu 18.04**: Removed (EOL)
- **Legacy platforms**: i386, ppc64le, s390x removed

## 🐛 Bug Fixes

- Fixed device status not updating on Linux/Docker/Router platforms
- Fixed hardware UUID mismatch in heartbeat API
- Fixed release workflow missing parameters

## 📋 Install / Upgrade

### Recommended: Install Script

```bash
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/refs/heads/main/omniedge-install.sh | bash
```

The script auto-detects your platform and installs the latest version.

### Manual Download

```bash
# Download for your platform
curl -LO https://github.com/omniedgeio/omniedge/releases/download/v1.0.0/omniedge-v1.0.0-YOUR_PLATFORM.zip
unzip omniedge-v1.0.0-YOUR_PLATFORM.zip
sudo mv omniedge /usr/local/bin/

# Verify
omniedge version
```

## 🙏 Contributors

Thank you to all contributors who made this release possible!

---

**Full Changelog**: https://github.com/omniedgeio/omniedge/compare/v0.3.0...v1.0.0
