# OmniEdge CLI v0.3.0 Release Notes

**Release Date:** January 6, 2026

## 🎉 What's New

### Native macOS `utun` Support
- **Driverless experience**: No need for Tunnelblick or third-party kernel extensions
- **Apple Silicon optimized**: Native ARM64 support for M1/M2/M3 Macs
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
- **Go 1.21**: Modern Go toolchain
- **OpenWrt SDK 23.05**: Stable router SDK
- **GCC 12.3**: Updated cross-compilers
- **GitHub Actions v4/v5**: Modern CI/CD

## 📦 Download Packages

### Linux
| Package | Architecture |
|---------|--------------|
| `omniedge-v0.3.0-amd64.zip` | x86_64 (Servers, NUCs) |
| `omniedge-v0.3.0-arm64.zip` | ARM64 (Jetson, RPi 4/5) |
| `omniedge-v0.3.0-arm.zip` | ARMv7 (RPi 3) |

### OpenWrt
| Package | Architecture |
|---------|--------------|
| `omniedge-v0.3.0-openwrt-amd64.zip` | x86_64 Routers |
| `omniedge-v0.3.0-openwrt-arm64.zip` | ARM64 Routers |
| `omniedge-v0.3.0-openwrt-arm.zip` | ARMv7 Routers |
| `omniedge-v0.3.0-openwrt-mips.zip` | MIPS Routers |
| `omniedge-v0.3.0-openwrt-mipsle.zip` | MIPSle Routers |

### Emerging Architectures
| Package | Architecture |
|---------|--------------|
| `omniedge-v0.3.0-riscv64.zip` | RISC-V 64-bit |
| `omniedge-v0.3.0-loongarch64.zip` | LoongArch 64-bit |

### Desktop
| Package | Platform |
|---------|----------|
| `omniedge-v0.3.0-macos-latest.zip` | macOS (Apple Silicon) |
| `omniedge-v0.3.0-ubuntu-22.04.zip` | Ubuntu 22.04 LTS |
| `omniedge-v0.3.0-ubuntu-24.04.zip` | Ubuntu 24.04 LTS |
| `omniedge-v0.3.0-freebsd-14.zip` | FreeBSD 14 |

## 🔧 Breaking Changes

- **macOS**: Now uses `utun` interface (appears as `utunX` instead of `tapX`)
- **Ubuntu 20.04**: Dropped due to runner availability (use Ubuntu 22.04+)
- **Ubuntu 18.04**: Removed (EOL)
- **Legacy platforms**: i386, ppc64le, s390x removed

## 🐛 Bug Fixes

- Fixed device status not updating on Linux/Docker/Router platforms
- Fixed hardware UUID mismatch in heartbeat API

## 📋 Upgrade Instructions

```bash
# Download and replace existing binary
curl -LO https://github.com/omniedgeio/omniedge/releases/download/v0.3.0/omniedge-v0.3.0-YOUR_PLATFORM.zip
unzip omniedge-v0.3.0-YOUR_PLATFORM.zip
sudo mv omniedge /usr/local/bin/

# Verify
omniedge version
```

## 🙏 Contributors

Thank you to all contributors who made this release possible!

---

**Full Changelog**: https://github.com/omniedgeio/omniedge/compare/v0.2.4...v0.3.0
