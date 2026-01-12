# OmniEdge Release Notes
 
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
| Package | Platform |
|---------|----------|
| `omniedge-desktop-1.0.0-windows-amd64.exe` | Windows (NSIS Installer) |
| `omniedge-desktop-1.0.0-macos-arm64.dmg` | macOS (Apple Silicon) |
| `omniedge-desktop-1.0.0-linux-amd64.deb` | Ubuntu/Debian |
| `omniedge-desktop-1.0.0-linux-amd64.rpm` | Fedora/RHEL |
| `omniedge-desktop-1.0.0-linux-amd64.AppImage` | Universal Linux |
| `omniedge-desktop-1.0.0-linux-amd64.flatpak` | Flatpak |
| `omniedge-desktop-1.0.0-linux-amd64-arch.tar.gz` | Arch Linux |

### CLI - Linux
| Package | Architecture |
|---------|--------------|
| `omniedge-v1.0.0-amd64.zip` | x86_64 (Servers, NUCs) |
| `omniedge-v1.0.0-arm64.zip` | ARM64 (Jetson, RPi 4/5) |
| `omniedge-v1.0.0-arm.zip` | ARMv7 (RPi 3) |

### CLI - OpenWrt
| Package | Architecture |
|---------|--------------|
| `omniedge-v1.0.0-openwrt-amd64.zip` | x86_64 Routers |
| `omniedge-v1.0.0-openwrt-arm64.zip` | ARM64 Routers |
| `omniedge-v1.0.0-openwrt-arm.zip` | ARMv7 Routers |
| `omniedge-v1.0.0-openwrt-mips.zip` | MIPS Routers |
| `omniedge-v1.0.0-openwrt-mipsle.zip` | MIPSle Routers |

### CLI - Other Platforms
| Package | Platform |
|---------|----------|
| `omniedge-v1.0.0-macos-arm64.zip` | macOS CLI (Apple Silicon) |
| `omniedge-v1.0.0-freebsd-14.zip` | FreeBSD 14 |
| `omniedge-v1.0.0-riscv64.zip` | RISC-V 64-bit |
| `omniedge-v1.0.0-loongarch64.zip` | LoongArch 64-bit |

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
