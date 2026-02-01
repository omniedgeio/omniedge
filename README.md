# OmniEdge

> Zero-Config P2P Mesh VPN for AI, Robotics, and Edge Computing

[![Release](https://img.shields.io/github/v/release/omniedgeio/omniedge?style=flat-square)](https://github.com/omniedgeio/omniedge/releases)
[![Release CLI](https://img.shields.io/github/actions/workflow/status/omniedgeio/omniedge/release.yml?label=CLI%20Build&style=flat-square)](https://github.com/omniedgeio/omniedge/actions/workflows/release.yml)
[![Release Desktop](https://img.shields.io/github/actions/workflow/status/omniedgeio/omniedge/desktop-release.yml?label=Desktop%20Build&style=flat-square)](https://github.com/omniedgeio/omniedge/actions/workflows/desktop-release.yml)
[![E2E Tests](https://img.shields.io/github/actions/workflow/status/omniedgeio/omniedge/e2e.yml?label=E2E%20Tests&style=flat-square)](https://github.com/omniedgeio/omniedge/actions/workflows/e2e.yml)
[![License](https://img.shields.io/github/license/omniedgeio/omniedge?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/tauri-v2-blue?style=flat-square&logo=tauri&logoColor=white)](https://tauri.app/)
[![OmniNervous](https://img.shields.io/badge/OmniNervous-v0.3.1-green?style=flat-square)](https://github.com/omniedgeio/OmniNervous)
<br/>
[![Platform](https://img.shields.io/badge/platform-linux%20%7C%20macos%20%7C%20windows-blue?style=flat-square)](#supported-platforms)
[![Discord](https://img.shields.io/discord/1234567890?color=5865F2&label=discord&logo=discord&logoColor=white&style=flat-square)](https://discord.gg/d4faRPYj)

## The Problem

Building distributed AI systems is hard. Connecting robots, edge devices, and cloud infrastructure across networks is even harder:

- Your robot fleet can't communicate behind NAT
- Federated learning nodes need secure, low-latency links
- Remote debugging AI devices requires complex VPN setup
- Multi-agent systems need peer discovery without central servers
- Humanoid robot teleoperation demands deterministic, jitter-free networking

**OmniEdge solves this with a single binary.**

## Why Researchers & Developers Choose OmniEdge

| Challenge                     | OmniEdge Solution                              |
| ----------------------------- | ---------------------------------------------- |
| NAT traversal                 | Automatic hole punching + relay fallback, 99%+ success |
| Symmetric NAT                 | Zero-knowledge relay for hardest NAT types     |
| Latency-critical AI inference | WireGuard encryption, ~0.3ms overhead          |
| Deterministic networking      | 6-Sigma stability (Cpk 2.92) for teleoperation |
| Secure model transfer         | End-to-end encrypted mesh                      |
| Heterogeneous devices         | Single binary for x86, ARM64, RISC-V           |
| Air-gapped labs               | Self-hosted nucleus mode (no cloud dependency) |

## Performance: Industrial-Grade Stability

Validated through [50-run longitudinal testing](https://github.com/omniedgeio/OmniNervous/blob/main/Capability_test/cloud_test_50_run_paper.md) using Process Capability Analysis (Cpk):

| Metric                      | OmniEdge Tunnel    | Raw Internet | Improvement          |
| --------------------------- | ------------------ | ------------ | -------------------- |
| **Latency**                 | 54.69ms            | 54.36ms      | +0.3ms overhead      |
| **Latency Stability (Cpk)** | **2.92 (6-Sigma)** | 6.47         | Near-deterministic   |
| **Throughput**              | **484.7 Mbps**     | 344.1 Mbps   | **+140.8%**          |
| **Jitter (StdDev)**         | 0.057ms            | 0.026ms      | Bounded, predictable |

> **What this means**: Cpk > 2.0 indicates industrial-grade process capability. OmniEdge provides deterministic, jitter-controlled networking suitable for real-time robot control and latency-sensitive AI inference.

## Perfect For

### Robotics

- **Humanoid Teleoperation**: Deterministic latency for real-time control loops
- **Robot Swarms**: Mesh networking for multi-robot coordination
- **ROS 2 Integration**: Seamless DDS discovery across networks
- **Remote Debugging**: SSH into any robot without port forwarding

### AI & Machine Learning

- **Federated Learning**: Secure gradient exchange between edge nodes
- **Distributed Inference**: Split models across Jetson/Pi clusters
- **MLOps Pipelines**: Deploy models to edge devices seamlessly
- **GPU Cluster Access**: Connect to remote training infrastructure

### Research

- **Multi-Agent Systems**: P2P communication for agent coordination
- **Edge Computing**: Connect fog nodes to cloud transparently
- **IoT Testbeds**: Instant mesh for sensor networks
- **Reproducible Experiments**: Consistent networking across trials

## Quick Start (60 seconds)

```bash
# Install on any Linux device (Jetson, Pi, server)
curl -fsSL https://raw.githubusercontent.com/omniedgeio/omniedge/main/scripts/omniedge-install.sh | bash

# Start and connect (interactive login on first run)
sudo omniedge start

# That's it. Your devices can now reach each other by virtual IP.
```

## Supported Hardware

| Device                           | Architecture | Status       |
| -------------------------------- | ------------ | ------------ |
| NVIDIA Jetson (Nano/Xavier/Orin) | ARM64        | Tested       |
| Raspberry Pi 3/4/5               | ARM64/ARMv7  | Tested       |
| Intel NUC / x86 Servers          | x86_64       | Tested       |
| Apple Silicon (M1/M2/M3)         | ARM64        | Tested       |
| RISC-V Boards                    | riscv64      | Experimental |
| OpenWrt Routers                  | Various      | Community    |

## Supported Platforms

### CLI (`omniedge-cli`)

| Platform    | Architecture                          | Package Formats                        |
| ----------- | ------------------------------------- | -------------------------------------- |
| **Linux**   | x86_64, ARM64, ARMv7, RISC-V          | `.tar.gz`, `.deb`, `.rpm`, `.AppImage` |
| **macOS**   | x86_64 (Intel), ARM64 (Apple Silicon) | `.tar.gz`                              |
| **Windows** | x86_64                                | `.zip`                                 |

### Desktop (`omniedge-desktop`)

| Platform    | Architecture  | Package Formats     |
| ----------- | ------------- | ------------------- |
| **Windows** | x86_64        | `.msi`, `.exe`      |
| **macOS**   | x86_64, ARM64 | `.dmg`              |
| **Linux**   | x86_64        | `.deb`, `.AppImage` |

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Your AI Network                          │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │ Jetson Orin  │    │  Robot Fleet │    │    Cloud     │       │
│  │  10.147.1.1  │◄──►│  10.147.1.x  │◄──►│  10.147.1.x  │       │
│  │  ( Edges )   │    │   (Edges)    │    │   (Edges)    │       │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘       │
│         │                   │                   │               │
│         └───────────────────┼───────────────────┘               │
│                             │                                   │
│                   ┌─────────▼─────────┐                         │
│                   │   OmniEdge Mesh   │                         │
│                   │    (WireGuard)    │                         │
│                   │                   │                         │
│                   │  - E2E Encrypted  │                         │
│                   │  - NAT Traversal  │                         │
│                   │  - Relay Fallback │                         │
│                   │  - 6σ Stability   │                         │
│                   └───────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

## Plugin System (v2.2.0)

OmniEdge now supports a **WASM-based plugin system** that allows extending functionality without modifying the core application.

### Features

| Feature | Description |
|---------|-------------|
| **WASM Sandboxing** | Plugins run in secure WebAssembly sandboxes |
| **Event Hooks** | React to VPN state changes, peer events, network changes |
| **Capability-Based Security** | Fine-grained permissions per plugin |
| **Hot Reload** | Install, enable, disable plugins without restart |
| **Cross-Platform** | Single plugin binary works on all platforms |

### Plugin Capabilities

| Capability | Description |
|------------|-------------|
| `network-status` | Read VPN connection state |
| `peer-info` | Access peer list and status |
| `event-hooks` | Subscribe to VPN events |
| `http-client` | Make outbound HTTP requests |
| `key-value-store` | Persist plugin data |
| `notifications` | Show system notifications |
| `logging` | Write to application logs |

### Installing Plugins

```bash
# Via CLI (v2.2.0)
omniedge plugin list                    # List installed plugins
omniedge plugin install ./plugin.zip    # Install from ZIP file
omniedge plugin uninstall <plugin-id>   # Remove a plugin
omniedge plugin enable <plugin-id>      # Enable a plugin
omniedge plugin disable <plugin-id>     # Disable a plugin
omniedge plugin info <plugin-id>        # Show plugin details
omniedge plugin reload <plugin-id>      # Reload a plugin
omniedge plugin discover                # Discover plugins in directory

# Via Desktop UI
# Settings > Plugins > Install Plugin > Select .zip file

# Plugin directory
# Windows: %APPDATA%\OmniEdge\plugins\
# macOS:   ~/Library/Application Support/OmniEdge/plugins/
# Linux:   ~/.local/share/omniedge/plugins/
```

### Creating Plugins

See [examples/plugins/hello-world](examples/plugins/hello-world) for a complete example.

```rust
// Minimal plugin structure
wit_bindgen::generate!({
    world: "omniedge-plugin",
    exports: {
        world: MyPlugin,
    },
});

struct MyPlugin;

impl Guest for MyPlugin {
    fn on_load() -> Result<(), String> {
        Ok(())
    }
}
```

## NAT Traversal (v2.1.0)

OmniEdge automatically handles complex network environments with multi-layer NAT traversal:

### How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                    NAT Traversal Stack                          │
├─────────────────────────────────────────────────────────────────┤
│  1. STUN Detection    → Identify NAT type (Open/Cone/Symmetric) │
│  2. UDP Hole Punching → Direct P2P for compatible NAT types     │
│  3. Port Mapping      → UPnP/NAT-PMP to open router ports       │
│  4. Relay Fallback    → Zero-knowledge relay for symmetric NAT  │
└─────────────────────────────────────────────────────────────────┘
```

### NAT Type Compatibility

| Your NAT | Peer NAT | Connection Method |
|----------|----------|-------------------|
| Open/Full Cone | Any | Direct P2P |
| Restricted Cone | Open/Full/Restricted | Direct P2P |
| Port-Restricted | Open/Full Cone | Direct P2P |
| Symmetric | Open/Full Cone | Direct P2P (usually) |
| Symmetric | Symmetric | **Relay** (automatic) |

### Features

| Feature | Description |
|---------|-------------|
| **Auto NAT Detection** | STUN-based detection on connection |
| **Relay Fallback** | Zero-knowledge relay for symmetric NAT |
| **Port Mapping** | UPnP/NAT-PMP/PCP support |
| **Encrypted Signaling** | X25519 + XSalsa20-Poly1305 |
| **IPv6 Dual-Stack** | Happy Eyeballs (RFC 8305) |

### Configuration

```bash
# Check your NAT type and settings
omniedge status

# Configure NAT traversal
omniedge config show              # View current settings
omniedge config relay on          # Enable relay fallback
omniedge config portmap on        # Enable UPnP/NAT-PMP
omniedge config ipv6 prefer       # Prefer IPv6 when faster
omniedge config encrypt on        # Enable encrypted signaling
```

## Operating Modes

| Mode               | Description           | Use Case                       |
| ------------------ | --------------------- | ------------------------------ |
| **edge** (default) | VPN client            | Connect devices to mesh        |
| **nucleus**        | Signaling server only | Self-hosted relay, no cloud    |
| **dual**           | VPN + signaling       | Central hub + mesh participant |

```bash
# Edge mode - Join an existing network
sudo omniedge start -n <network_id>

# Nucleus mode - Run your own signaling server (air-gapped labs)
sudo omniedge start --mode nucleus --port 51821 --secret "YourLabSecret123"

# Dual mode - Hub node that also participates in mesh
sudo omniedge start -n <network_id> --mode dual --secret "YourSecret123456"
```

## Self-Hosted Mode (Air-Gapped Labs)

Run completely offline with no cloud dependency:

```bash
# On your central server (e.g., lab gateway)
sudo omniedge start --mode nucleus --port 51821 --secret "LabSecret2026!"

# On edge devices - they discover each other through your nucleus
# Configure network settings via the dashboard or API
```

**Use cases:**
- Secure research environments
- Industrial robotics with network isolation
- Defense and government applications
- Privacy-critical deployments

## CLI Reference

```bash
# Basic operations
omniedge start                    # Connect to first available network
omniedge start -n <network_id>    # Connect to specific network
omniedge status                   # Check connection status
omniedge stop                     # Disconnect

# Authentication
omniedge start -s <security_key>  # Non-interactive login (CI/CD)

# Exit node (route traffic through a peer)
omniedge start -x                 # Run as exit node
omniedge start -e <peer_ip>       # Use specific exit node
omniedge start --no-exit-node     # Disable exit node

# NAT traversal configuration (v2.1.0)
omniedge config show              # Show current network settings
omniedge config relay on|off      # Enable/disable relay fallback
omniedge config relay server <url> # Set custom relay server
omniedge config portmap on|off    # Enable/disable UPnP/NAT-PMP
omniedge config ipv6 on|off|prefer # IPv6 settings
omniedge config encrypt on|off    # Encrypted signaling
omniedge config reset             # Reset to defaults

# Plugin management (v2.2.0)
omniedge plugin list              # List installed plugins
omniedge plugin install <path>    # Install plugin from ZIP
omniedge plugin uninstall <id>    # Remove a plugin
omniedge plugin enable <id>       # Enable a plugin
omniedge plugin disable <id>      # Disable a plugin
omniedge plugin info <id>         # Show plugin details
omniedge plugin reload <id>       # Reload a plugin
omniedge plugin discover          # Discover plugins in directory

# Advanced modes
omniedge start --mode nucleus --port 51821 --secret "..."  # Signaling server
omniedge start --mode dual --secret "..."                  # Hub + client
```

## V2 Migration Notice

OmniEdge V2 is a complete rewrite in Rust, replacing the legacy Go/n2n implementation:

| Aspect     | V1 (Legacy)                                                      | V2 (Current)                  |
| ---------- | ---------------------------------------------------------------- | ----------------------------- |
| Language   | Go                                                               | Rust                          |
| Protocol   | n2n                                                              | OmniNervous (WireGuard-based) |
| License    | GPL-3.0                                                          | Apache-2.0 / MIT              |
| Repository | [omniedge-legacy](https://github.com/omniedgeio/omniedge-legacy) | This repository               |

## Research & Citations

Using OmniEdge in your research? We'd love to hear about it.

```bibtex
@software{omniedge2026,
  title = {OmniEdge: Zero-Config P2P Mesh VPN for Edge Computing},
  author = {OmniEdge Team},
  year = {2026},
  url = {https://github.com/omniedgeio/omniedge},
  note = {Industrial-grade stability (Cpk 2.92) validated through 50-run longitudinal testing}
}
```

### Related Publications

- [OmniNervous Protocol Stability Analysis](https://github.com/omniedgeio/OmniNervous/blob/main/Capability_test/cloud_test_50_run_paper.md) - 50-run Cpk validation study

## Built With

- **[Rust](https://www.rust-lang.org/)** - Memory safety, zero-cost abstractions
- **[WireGuard](https://www.wireguard.com/)** - Modern, audited cryptography
- **[OmniNervous](https://github.com/omniedgeio/OmniNervous)** - High-performance P2P daemon with NAT traversal
  - STUN-based NAT detection
  - Relay fallback for symmetric NAT
  - UPnP/NAT-PMP/PCP port mapping
  - Happy Eyeballs (RFC 8305) for IPv6
- **[Tauri](https://tauri.app/)** - Lightweight desktop apps

## Community

- [Discord](https://discord.gg/d4faRPYj) - Ask questions, share projects
- [GitHub Issues](https://github.com/omniedgeio/omniedge/issues) - Bug reports and feature requests
- [Twitter](https://twitter.com/omniedgeio) - Updates and announcements

## License

Dual-licensed under [Apache License 2.0](LICENSE-APACHE) and [MIT License](LICENSE-MIT). 

Use freely in academic and commercial projects. See [LICENSING.md](LICENSING.md) for details.

---

**Built for the machines that build the future.**

[Website](https://connect.omniedge.io) | [Docs](https://connect.omniedge.io/docs) | [Discord](https://discord.gg/d4faRPYj) | [Twitter](https://twitter.com/omniedgeio)
