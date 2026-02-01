# OmniEdge Release Notes

## v2.2.1 (2026-02-01)

### Desktop UI Improvements

- **Data Collection Plugin**: Added "DEMO" label to Data Collection plugin in the Plugins section
- **Native Window Decorations**: Data Collection window now uses native window decorations with standard close button
- **Window Visibility**: Removed transparency from Data Collection window for better visibility

### Build Changes

- **Removed ARMv7 Support**: Dropped `armv7-unknown-linux-musleabihf` target from CLI release builds
  - Raspberry Pi 3 and older 32-bit ARM devices are no longer supported
  - Use ARM64 builds for Raspberry Pi 4/5 and other modern ARM devices

### Compatibility

- **OmniNervous**: Requires v0.3.1 or later
- **Existing Networks**: Fully backward compatible with v2.2.0 networks

---

## v2.2.0 (2026-01-31)

### WASM Plugin System

This release introduces a **WebAssembly-based plugin system** that allows extending OmniEdge functionality through secure, sandboxed plugins.

### New Features

#### Plugin Runtime
- **WASM Component Model**: Plugins are compiled to WebAssembly components using the WASI Preview 2 standard
- **Secure Sandboxing**: Each plugin runs in an isolated sandbox with no direct system access
- **Capability-Based Security**: Plugins declare required capabilities in their manifest; users approve permissions on install

#### Supported Capabilities

| Capability | Description | Security Level |
|------------|-------------|----------------|
| `network-status` | Read VPN connection state | Low |
| `peer-info` | Access peer list and connection info | Low |
| `event-hooks` | Subscribe to VPN lifecycle events | Low |
| `logging` | Write to application logs | Low |
| `key-value-store` | Persist plugin-specific data | Medium |
| `notifications` | Display system notifications | Medium |
| `http-client` | Make outbound HTTP requests | High |

#### Event Hooks

Plugins can subscribe to the following events:

| Event | Description |
|-------|-------------|
| `on_state_change` | VPN connection state changed |
| `on_peer_discovered` | New peer joined the network |
| `on_peer_disconnected` | Peer left the network |
| `on_network_change` | Switched to different virtual network |
| `on_stats_update` | Periodic statistics update |

#### Desktop UI Integration
- **Plugin Management UI**: Install, enable, disable, and uninstall plugins from Settings
- **File Picker**: Install plugins from `.zip` archives via file dialog
- **Plugin Discovery**: Automatic detection of installed plugins on startup

#### CLI Plugin Commands (New)

The CLI now has full plugin management support, bringing feature parity with the desktop app:

```bash
# List all installed plugins
omniedge plugin list

# Install a plugin from ZIP file
omniedge plugin install ./my-plugin.zip

# Uninstall a plugin
omniedge plugin uninstall com.example.my-plugin

# Enable/disable plugins
omniedge plugin enable com.example.my-plugin
omniedge plugin disable com.example.my-plugin

# Show plugin details
omniedge plugin info com.example.my-plugin

# Reload a plugin (apply config changes)
omniedge plugin reload com.example.my-plugin

# Discover plugins in the plugins directory
omniedge plugin discover
```

| Command | Description |
|---------|-------------|
| `omniedge plugin list` | List all installed plugins with status |
| `omniedge plugin install <path>` | Install plugin from ZIP archive |
| `omniedge plugin uninstall <id>` | Remove a plugin completely |
| `omniedge plugin enable <id>` | Enable a disabled plugin |
| `omniedge plugin disable <id>` | Disable a plugin |
| `omniedge plugin info <id>` | Show detailed plugin information |
| `omniedge plugin reload <id>` | Reload a plugin |
| `omniedge plugin discover` | Scan plugins directory for new plugins |

### Helper Service Improvements

#### Security Hardening
- **Increased Buffer Size**: Request/response buffer increased from 4KB to 16KB for larger messages
- **Read Timeout**: Added 30-second timeout to prevent indefinite blocking from misbehaving clients
- **Truncation Detection**: Requests exceeding buffer size are detected and rejected gracefully
- **Safe Serialization**: Removed `unwrap()` calls; uses fallback JSON on serialization errors
- **Generic Error Messages**: Detailed errors logged server-side; clients receive generic messages

#### Concurrency Fix
- **Single-Request Model**: Each pipe connection handles one request and closes, preventing "All pipe instances are busy" errors on Windows
- **Test Script**: Added `scripts/test-helper.ps1` for validating helper service functionality

### Technical Details

#### Plugin Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Desktop App                        │
│  ┌───────────────────────────────────────────────┐  │
│  │              Plugin Manager                    │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐       │  │
│  │  │ Plugin  │  │ Plugin  │  │ Plugin  │  ...  │  │
│  │  │  WASM   │  │  WASM   │  │  WASM   │       │  │
│  │  └────┬────┘  └────┬────┘  └────┬────┘       │  │
│  │       │            │            │             │  │
│  │  ┌────▼────────────▼────────────▼────┐       │  │
│  │  │         Wasmtime Runtime          │       │  │
│  │  │    (Secure WASM Sandbox)          │       │  │
│  │  └───────────────────────────────────┘       │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

#### Plugin Manifest Format

```json
{
  "id": "example-plugin",
  "name": "Example Plugin",
  "version": "1.0.0",
  "description": "An example plugin",
  "author": "Your Name",
  "capabilities": ["network-status", "event-hooks", "logging"],
  "entry_point": "plugin.wasm"
}
```

#### New Crate: `omni-plugin`

| Component | Description |
|-----------|-------------|
| `PluginManager` | Manages plugin lifecycle (install, load, unload) |
| `PluginRuntime` | Wasmtime-based WASM execution environment |
| `PluginPackage` | Handles plugin ZIP archives and manifests |
| `PluginManifest` | Strongly-typed manifest parsing and validation |

### New Files

| File | Description |
|------|-------------|
| `crates/omni-plugin/` | New plugin system crate |
| `crates/omni-cli/src/main.rs` | CLI now includes plugin subcommands |
| `examples/plugins/hello-world/` | Example plugin with full source |
| `scripts/test-helper.ps1` | Windows helper service test script |

### Bug Fixes

- **Windows Named Pipe Concurrency**: Fixed "All pipe instances are busy" error when desktop app makes concurrent helper requests
- **Plugin Manifest Format**: Fixed capability format (now uses kebab-case: `event-hooks` instead of `EventHooks`)

### Compatibility

- **OmniNervous**: Requires v0.3.1 or later
- **Wasmtime**: v18.0 (WASI Preview 2 support)
- **Existing Networks**: Fully backward compatible with v2.1.0 networks
- **Plugins**: New feature, requires v2.2.0 desktop app

### Contributors

Thank you to all contributors who made this release possible!

---

## v2.1.0 (2026-01-31)

### Advanced NAT Traversal

This release integrates **OmniNervous v0.3.1**, bringing enterprise-grade NAT traversal capabilities for reliable connectivity in challenging network environments.

### New Features

#### NAT Type Detection
- **Automatic STUN-based Detection**: Identifies your NAT type on connection
- **Five NAT Types Supported**:
  | Type | Description | P2P Success Rate |
  |------|-------------|------------------|
  | Open | No NAT (direct connectivity) | 100% |
  | Full Cone | Easiest NAT for P2P | 95%+ |
  | Restricted Cone | Moderate NAT | 85%+ |
  | Port-Restricted Cone | Strict NAT | 70%+ |
  | Symmetric | Hardest NAT (relay required) | Relay fallback |

#### Relay Fallback
- **Automatic Relay**: When direct P2P fails (symmetric NAT), traffic routes through relay servers
- **Zero-Knowledge Relay**: Only forwards encrypted WireGuard packets, never decrypts
- **Configurable Relay Servers**: Use default or specify custom relay endpoints

#### Port Mapping (UPnP/NAT-PMP)
- **Automatic Port Mapping**: Requests router to open ports for better connectivity
- **Protocol Support**: NAT-PMP (RFC 6886), PCP (RFC 6887), UPnP IGD
- **Improves NAT Type**: Can upgrade "strict" NAT to "open" for direct P2P

#### Encrypted Signaling
- **X25519 Key Exchange**: Secure key negotiation
- **XSalsa20-Poly1305**: Authenticated encryption for signaling messages
- **Backward Compatible**: Works with older clients

#### IPv6 Dual-Stack
- **Native IPv6 Support**: Connect over IPv4 or IPv6
- **Happy Eyeballs (RFC 8305)**: Races IPv4/IPv6 connections, uses fastest
- **Preference Control**: Prefer IPv6 when latency threshold met

### New CLI Commands

#### `omniedge config` - Manage NAT Traversal Settings

```bash
# Show current network configuration
omniedge config show

# Relay settings
omniedge config relay on          # Enable relay fallback
omniedge config relay off         # Disable relay
omniedge config relay server <url> # Set custom relay server

# Port mapping
omniedge config portmap on        # Enable UPnP/NAT-PMP
omniedge config portmap off       # Disable port mapping

# IPv6 settings
omniedge config ipv6 on           # Enable IPv6
omniedge config ipv6 off          # Disable IPv6
omniedge config ipv6 prefer       # Prefer IPv6 when faster

# Encrypted signaling
omniedge config encrypt on        # Enable encrypted signaling
omniedge config encrypt off       # Disable encryption

# Reset to defaults
omniedge config reset
```

#### Enhanced `omniedge status`

Status command now shows NAT traversal information:

```
Network Configuration:
  NAT Type:           Port-Restricted Cone (strict NAT)
  Relay:              enabled (default server)
  Port Mapping:       enabled (UPnP active)
  IPv6:               enabled (prefer when <50ms faster)
  Encrypted Signaling: enabled
```

### Technical Details

#### OmniNervous Integration

| Component | Version | Description |
|-----------|---------|-------------|
| omninervous | v0.3.1 | Core NAT traversal library |
| NatChecker | STUN | Multi-server NAT detection |
| RelayClient | v0.3.0 | Session-based relay with rate limiting |
| PortMapper | v0.3.0 | NAT-PMP/UPnP/PCP support |
| DualStackSocket | v0.3.0 | IPv4/IPv6 with Happy Eyeballs |

#### Runtime State API (v0.3.1)

New methods for querying NAT traversal status:

```rust
// Query relay statistics
proto.get_relay_stats().await  // -> Option<RelayStats>

// Query port mapping capabilities  
proto.get_portmap_status().await  // -> Option<PortMapCapabilities>

// Check if relay is active
proto.is_using_relay().await  // -> bool

// Check configuration state
proto.is_relay_enabled().await  // -> bool
proto.is_portmap_enabled().await  // -> bool
```

### Configuration

Network settings are stored in `~/.omniedge/config.toml`:

```toml
[network]
relay_enabled = true
relay_server = ""  # Empty = use default
portmap_enabled = true
encrypt_signaling = true
ipv6_enabled = true
prefer_ipv6 = false
ipv6_preference_threshold_ms = 50
```

### Compatibility

- **OmniNervous**: Requires v0.3.1 or later
- **Existing Networks**: Fully backward compatible with v2.0.0 networks
- **Mixed Versions**: v2.1.0 clients work with v2.0.0 clients (NAT features just won't be used)

### Bug Fixes

- None in this release (feature release)

### Known Limitations

- Port mapping requires router support (not all routers support UPnP/NAT-PMP)
- Relay adds latency (~10-50ms) compared to direct P2P
- Symmetric NAT on both peers always requires relay

### Contributors

Thank you to all contributors who made this release possible!

---

## v2.0.0 (2026-01-28)

### Complete Rust Rewrite

OmniEdge 2.0 is a **complete ground-up rewrite** from Go/n2n to pure Rust, delivering a modern, high-performance mesh VPN with significantly improved architecture.

### Architecture Changes

| Component | v1.x (Legacy) | v2.0 (New) |
|-----------|---------------|------------|
| Language | Go + C (n2n) | Pure Rust |
| Protocol | n2n supernode/edge | OmniNervous (custom) |
| TUN Driver | tap-windows, tuntap | omni-tun (native) |
| Desktop | Wails v3 | Tauri v2 + React |
| Signaling | n2n supernode | Nucleus server |

### Three-Mode System

The CLI now supports three operational modes via `--mode`:

| Mode | Description | Auth Required | VPN Tunnel | Signaling Server |
|------|-------------|---------------|------------|------------------|
| **edge** (default) | Regular VPN client | Yes | Yes | No |
| **nucleus** | Signaling server only | No | No | Yes |
| **dual** | VPN client + signaling | Yes | Yes | Yes |

```bash
# Edge mode (default) - VPN client
omniedge start -n <network_id>

# Nucleus mode - Standalone signaling server (no VPN, no login required)
omniedge start --mode nucleus --secret "MySecretMin16Chars"

# Dual mode - VPN client + nucleus signaling server
omniedge start -n <network_id> --mode dual --secret "MySecretMin16Chars"
```

### CLI Commands

| Command | Description |
|---------|-------------|
| `omniedge start` | Connect to a network |
| `omniedge stop` | Stop connection and background service |
| `omniedge status` | Show connection status and network info |
| `omniedge scan` | Scan local subnet and upload results |

#### Start Command Options

| Option | Short | Description |
|--------|-------|-------------|
| `--mode <MODE>` | `-m` | Operating mode: `edge` (default), `nucleus`, `dual` |
| `--network-id <ID>` | `-n` | Virtual network ID to join |
| `--as-exit-node` | `-x` | Act as an exit node (allow traffic routing) |
| `--no-exit-node` | | Disable exit node mode |
| `--exit-node <IP>` | `-e` | Use a specific exit node IP |
| `--port <PORT>` | `-p` | UDP port for nucleus server (default: 51820) |
| `--secret <SECRET>` | | Cluster secret for nucleus mode (min 16 chars) |
| `--security-key <KEY>` | `-s` | Security key for CI/server authentication |

#### Global Options

| Option | Short | Description |
|--------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output (show all logs to stderr) |
| `--help` | | Show help |
| `--version` | | Show version |

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
- **Subnet Scanning**: Discover local network hosts
  - `omniedge scan -c 192.168.1.0/24`
- **Custom User Servers**: Users can configure their own nucleus/relay servers via dashboard
- **Status Command**: Check connection status with `omniedge status`
  - Shows virtual IP, network, interface name, and exit node role
  - Displays live data from network interface

### Supported Platforms

#### CLI Binary Targets

| Platform | Architectures |
|----------|---------------|
| Linux | x86_64, aarch64, riscv64 |
| macOS | x86_64 (Intel), aarch64 (Apple Silicon) |
| Windows | x86_64 |

#### Desktop Application

| Platform | Architectures | Formats |
|----------|---------------|---------|
| Linux | x86_64 | .deb, .rpm, .AppImage |
| macOS | x86_64, aarch64 | .dmg |
| Windows | x86_64 | .msi, .exe (NSIS) |

### Package Formats

| Format | Architectures |
|--------|---------------|
| DEB | amd64, arm64 |
| RPM | x86_64, aarch64 |
| AppImage (CLI) | x86_64 |
| DMG | macOS x64, arm64 |
| MSI/NSIS | Windows x64 |
| tar.gz/zip | All platforms |

### Desktop Application

- **Tauri v2**: Modern, lightweight desktop framework
- **React 19 + TypeScript**: Responsive UI with Vite
- **System Tray**: Quick access menu with network switching
- **Dynamic Positioning**: Window follows tray icon
- **Cross-Platform**: Windows, macOS, Linux
- **Helper Binary**: Bundled `omni-helper` for privileged operations

### Breaking Changes

- **Complete Protocol Change**: v2.0 is **not compatible** with v1.x networks
  - All devices in a network must upgrade to v2.0
- **Configuration Format**: New TOML-based config replaces legacy format
- **Architecture**: Complete rewrite from Go/n2n to Rust/OmniNervous
- **TUN Interface**: Interface names changed
  - Linux: `omniedge0` (was `edge0`)
  - macOS: `utun*` (unchanged)
  - Windows: `OmniEdge` (was `tap-omniedge`)

### Migration Guide

1. **Uninstall v1.x**: Remove old OmniEdge installation
2. **Install v2.0**: Download and install new version
3. **Re-authenticate**: Start with `omniedge start` or use security key

### Download Packages

#### CLI - Linux tar.gz
| Package | Architecture |
|---------|--------------|
| `omniedge-cli-v2.0.0-linux-x86_64.tar.gz` | x86_64 |
| `omniedge-cli-v2.0.0-linux-aarch64.tar.gz` | ARM64 |
| `omniedge-cli-v2.0.0-linux-riscv64.tar.gz` | RISC-V 64 |

#### CLI - DEB Packages
| Package | Architecture |
|---------|--------------|
| `omniedge-cli_2.0.0_amd64.deb` | Debian/Ubuntu x64 |
| `omniedge-cli_2.0.0_arm64.deb` | Debian/Ubuntu ARM64 |

#### CLI - RPM Packages
| Package | Architecture |
|---------|--------------|
| `omniedge-cli-2.0.0-1.x86_64.rpm` | Fedora/RHEL x64 |
| `omniedge-cli-2.0.0-1.aarch64.rpm` | Fedora/RHEL ARM64 |

#### CLI - AppImage
| Package | Architecture |
|---------|--------------|
| `omniedge-cli-2.0.0-x86_64.AppImage` | Linux x64 |

#### CLI - Other Platforms
| Package | Platform |
|---------|----------|
| `omniedge-cli-v2.0.0-macos-x86_64.tar.gz` | macOS Intel |
| `omniedge-cli-v2.0.0-macos-aarch64.tar.gz` | macOS Apple Silicon |
| `omniedge-cli-v2.0.0-windows-x86_64.zip` | Windows x64 |

#### Desktop Applications
| Package | Platform |
|---------|----------|
| `omniedge-desktop-2.0.0-windows-x64.msi` | Windows x64 (MSI) |
| `omniedge-desktop-2.0.0-windows-x64-setup.exe` | Windows x64 (NSIS) |
| `omniedge-desktop-2.0.0-macos-x64.dmg` | macOS Intel |
| `omniedge-desktop-2.0.0-macos-arm64.dmg` | macOS Apple Silicon |
| `omniedge-desktop-2.0.0-linux-x64.deb` | Linux x64 DEB |
| `omniedge-desktop-2.0.0-linux-x64.rpm` | Linux x64 RPM |
| `omniedge-desktop-2.0.0-linux-x64.AppImage` | Linux x64 AppImage |

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

#### macOS CLI
```bash
# Download and extract (Apple Silicon)
curl -LO https://github.com/omniedgeio/omniedge/releases/latest/download/omniedge-cli-v2.0.0-macos-aarch64.tar.gz
tar -xzf omniedge-cli-v2.0.0-macos-aarch64.tar.gz
sudo mv omniedge /usr/local/bin/

# Intel Mac
curl -LO https://github.com/omniedgeio/omniedge/releases/latest/download/omniedge-cli-v2.0.0-macos-x86_64.tar.gz
tar -xzf omniedge-cli-v2.0.0-macos-x86_64.tar.gz
sudo mv omniedge /usr/local/bin/
```

### macOS Desktop Installation

1. Download the `.dmg` file for your Mac (ARM64 for Apple Silicon, x64 for Intel)
2. Open the DMG and drag **OmniEdge** to **Applications**
3. Open **OmniEdge** from Applications
4. If you see a security warning:
   - Open **System Settings** → **Privacy & Security**
   - Scroll down and click **Open Anyway** next to the OmniEdge message
   - Click **Open** in the confirmation dialog

### Windows Desktop Installation

1. Download the `.msi` or `-setup.exe` installer
2. Run the installer (may require administrator privileges)
3. Launch OmniEdge from the Start Menu or System Tray

### Linux Desktop Installation

```bash
# DEB (Debian/Ubuntu)
sudo dpkg -i omniedge-desktop-2.0.0-linux-x64.deb

# RPM (Fedora/RHEL)
sudo rpm -i omniedge-desktop-2.0.0-linux-x64.rpm

# AppImage
chmod +x omniedge-desktop-2.0.0-linux-x64.AppImage
./omniedge-desktop-2.0.0-linux-x64.AppImage
```

### Contributors

Thank you to all contributors who made this major release possible!

---

## v1.0.2-beta.0 (2026-01-12)
### New Features & Improvements
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
### New Features
- **CLI Interactive Login**: The CLI now supports OAuth 2.0 Device Flow (`omniedge login`), allowing users to log in via browser without handling passwords directly.
- **Desktop Session Login**: Desktop application now uses seamless browser-based session login for improved security and experience.

### Desktop UI/UX Polish
- **Refined Header**: Implemented the official OmniEdge logo and clean typography.
- **Identity Status**: Added a visual status indicator (green dot) to the user profile chip for clear connection feedback.
- **Active Network**: The currently connected virtual network is now clearly highlighted with a distinct background and indicator in the dashboard list.

### Bug Fixes
- **Backend Stability**: Fixed a critical race condition (Error 1006) in the WebSocket service that caused login failures or "unexpected EOF" errors.
- **Login Flow**: Improved robustness of the browser-based login mechanism.

### CI/CD Improvements
- **macOS Build**: Release workflow now correctly targets Apple Silicon (arm64) for optimized performance on modern Macs.

---

## v1.0.0 (2026-01-08)
Release Notes

**Release Date:** January 8, 2026

## What's New

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

## Download Packages

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

## Breaking Changes

- **macOS**: Now uses `utun` interface (appears as `utunX` instead of `tapX`)
- **macOS amd64**: Removed (n2n library is arch-specific, use arm64 for Apple Silicon)
- **Ubuntu 20.04**: Dropped due to runner availability (use Ubuntu 22.04+)
- **Ubuntu 18.04**: Removed (EOL)
- **Legacy platforms**: i386, ppc64le, s390x removed

## Bug Fixes

- Fixed device status not updating on Linux/Docker/Router platforms
- Fixed hardware UUID mismatch in heartbeat API
- Fixed release workflow missing parameters

## Install / Upgrade

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

## Contributors

Thank you to all contributors who made this release possible!

---

**Full Changelog**: https://github.com/omniedgeio/omniedge/compare/v0.3.0...v1.0.0
