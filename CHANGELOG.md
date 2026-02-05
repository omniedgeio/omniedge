# Changelog

All notable changes to OmniEdge will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.5.0] - 2026-02-05

### Changed
- **OmniNervous Upgrade**: Updated core networking library from v0.4.0 to v0.5.0
  - Improved userspace WireGuard handshake timing
  - Better memory efficiency in packet processing
  - Foundation for L2 VPN support (Linux-only, feature-gated)

### Added
- **L2 VPN Readiness**: OmniNervous v0.5.0 includes L2 transport module (not yet exposed in CLI)
  - TAP-based Ethernet bridging for Linux
  - L2 fragmentation/reassembly for large frames
  - L2 Prometheus metrics for observability

### Fixed
- Added Windows `nul` artifact to `.gitignore`

## [2.4.0] - 2026-02-05

### Added
- **OpenWrt Package Support**: Native IPK (24.10.x) and APK (25.x) packages for OpenWrt routers
  - Supported architectures: x86_64, aarch64 (ARM64)
  - UCI configuration integration
  - procd init script with proper service management
  - Automatic startup on boot
- **Optional WASM Plugin System**: Plugin support is now a compile-time feature (`wasm-plugins`)
  - Allows building minimal binaries for resource-constrained devices
  - Default enabled on x86_64 and aarch64

### Changed
- **E2E Test Dockerfile**: Updated to use `rust:slim-bookworm` for latest Rust version compatibility
- **Cross-compilation**: Improved cross-rs configuration for aarch64-unknown-linux-musl

### Fixed
- Resolved all clippy lints and warnings across the codebase
- Fixed GitHub Actions workflow boolean conditionals for matrix.use_cross
- Fixed aarch64 musl linking errors by using cross-rs instead of native toolchain

### Removed
- **MIPS Architecture Support**: Removed mipsel and mips targets from OpenWrt builds
  - Rust has removed MIPS from stable rust-std distribution
  - Cranelift (Wasmtime JIT) has no MIPS backend
  - Focus on x86_64 and aarch64 which cover modern router hardware

## [2.3.0] - 2026-02-03

### Added
- **IPv6 Dual-Stack Support**: Full IPv6 connectivity with Happy Eyeballs (RFC 8305)
  - Automatic IPv4/IPv6 selection based on connectivity
  - `omniedge config ipv6 prefer` to prefer IPv6 when available
  - Parallel address resolution for faster connections
- **OpenWrt Package Build System**: GitHub Actions workflow for building OpenWrt packages
  - IPK packages for OpenWrt 24.10.x
  - APK packages for OpenWrt 25.x (snapshot)
  - Multi-architecture support via cross-rs

### Changed
- Updated OmniNervous to v0.4.0 with improved NAT traversal

## [2.2.1] - 2026-01-15

### Added
- **WASM Plugin System**: Extend OmniEdge functionality through secure WebAssembly plugins
  - Sandboxed execution environment
  - Event hooks for VPN state changes
  - Capability-based security model
  - Hot reload support
- Plugin CLI commands: `omniedge plugin list|install|uninstall|enable|disable|info|reload|discover`

### Fixed
- Plugin manager initialization on first run
- Plugin state persistence across restarts

## [2.2.0] - 2026-01-01

### Added
- Desktop application built with Tauri v2
- System tray integration
- Settings UI for network configuration
- Plugin management UI

## [2.1.0] - 2025-12-01

### Added
- **NAT Traversal Improvements**
  - STUN-based NAT type detection
  - Automatic relay fallback for symmetric NAT
  - UPnP/NAT-PMP/PCP port mapping
  - Encrypted signaling with X25519 + XSalsa20-Poly1305
- Network configuration CLI: `omniedge config show|relay|portmap|ipv6|encrypt|reset`

### Changed
- Improved peer discovery reliability
- Better handling of network transitions

## [2.0.0] - 2025-10-01

### Added
- Complete rewrite in Rust (from Go)
- WireGuard-based protocol via OmniNervous
- Multi-mode operation: edge, nucleus, dual
- Self-hosted nucleus mode for air-gapped deployments
- Exit node support

### Changed
- License changed from GPL-3.0 to Apache-2.0/MIT dual license
- New protocol incompatible with v1.x

### Removed
- n2n protocol support
- Legacy Go codebase (moved to omniedge-legacy repository)

---

[Unreleased]: https://github.com/omniedgeio/omniedge/compare/v2.5.0...HEAD
[2.5.0]: https://github.com/omniedgeio/omniedge/compare/v2.4.0...v2.5.0
[2.4.0]: https://github.com/omniedgeio/omniedge/compare/v2.3.0...v2.4.0
[2.3.0]: https://github.com/omniedgeio/omniedge/compare/v2.2.1...v2.3.0
[2.2.1]: https://github.com/omniedgeio/omniedge/compare/v2.2.0...v2.2.1
[2.2.0]: https://github.com/omniedgeio/omniedge/compare/v2.1.0...v2.2.0
[2.1.0]: https://github.com/omniedgeio/omniedge/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/omniedgeio/omniedge/releases/tag/v2.0.0
