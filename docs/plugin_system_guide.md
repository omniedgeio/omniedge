# OmniEdge Plugin System Guide

A comprehensive guide to developing, building, and deploying plugins for OmniEdge VPN.

## Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [Plugin Types](#plugin-types)
4. [Development Guide](#development-guide)
5. [Host APIs](#host-apis)
6. [Building Plugins](#building-plugins)
7. [Plugin Manifest](#plugin-manifest)
8. [Installation & Deployment](#installation--deployment)
9. [Security Model](#security-model)
10. [Troubleshooting](#troubleshooting)

---

## Overview

The OmniEdge Plugin System enables dynamic extensibility for the OmniEdge VPN application. Plugins run in WebAssembly (WASM) sandboxes, providing memory-safe isolation while allowing powerful customization.

### Key Features

| Feature | Description |
|---------|-------------|
| **WASM Sandbox** | Memory-safe isolation with capability-based access control |
| **Hot Reload** | Load/unload plugins without VPN restart |
| **Cross-Platform** | Plugins work on Windows, macOS, and Linux |
| **7 Plugin Types** | Event hooks, authentication, policies, QoS, and more |
| **Host APIs** | Logging, configuration, key-value storage, time, HTTP |

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    OmniEdge Application                         │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    Plugin Layer                            │  │
│  │   ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐      │  │
│  │   │  Event  │  │  Auth   │  │ Policy  │  │   QoS   │ ...  │  │
│  │   │ Plugin  │  │ Plugin  │  │ Plugin  │  │ Plugin  │      │  │
│  │   └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘      │  │
│  │        │            │            │            │            │  │
│  │   ┌────┴────────────┴────────────┴────────────┴────┐      │  │
│  │   │              WASM Runtime (wasmtime)            │      │  │
│  │   └─────────────────────────────────────────────────┘      │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ════════════════════ PLUGIN-FREE BOUNDARY ════════════════════ │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              OmniNervous VPN Transport (No Plugins)        │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

> **Important:** Plugins run in OmniEdge's application layer only. The core VPN transport (OmniNervous) remains plugin-free for security and performance.

---

## Quick Start

### Prerequisites

1. **Rust toolchain** (1.75+):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **WASM target**:
   ```bash
   rustup target add wasm32-wasip1
   ```

3. **cargo-component** (WASM component toolchain):
   ```bash
   cargo install cargo-component
   ```
   > Note: First installation takes 5-10 minutes to compile dependencies.

### Create Your First Plugin

1. **Create a new plugin project:**
   ```bash
   mkdir my-plugin && cd my-plugin
   cargo init --lib
   ```

2. **Configure `Cargo.toml`:**
   ```toml
   [package]
   name = "my-plugin"
   version = "0.1.0"
   edition = "2021"

   [workspace]  # Standalone crate

   [lib]
   crate-type = ["cdylib"]

   [dependencies]
   wit-bindgen = "0.36"

   [package.metadata.component]
   package = "mycompany:my-plugin"

   [package.metadata.component.target]
   path = "wit"
   world = "event-plugin-world"
   ```

3. **Create WIT interface (`wit/world.wit`):**
   ```wit
   package omniedge:plugin@1.0.0;

   interface types {
       enum capability {
           event-hooks,
           authentication,
           network-policy,
           data-triage,
           qos-enforcement,
           pdm-reporting,
           compliance,
           ui-widgets,
       }

       variant connection-state {
           disconnected,
           authenticating,
           authenticated,
           joining(string),
           connected(string),
       }

       record peer-info {
           public-key: string,
           virtual-ip: string,
           endpoint: option<string>,
           last-handshake: u64,
           rx-bytes: u64,
           tx-bytes: u64,
       }

       record state-change-event {
           old-state: connection-state,
           new-state: connection-state,
           timestamp: u64,
           network-id: option<string>,
       }

       record network-event {
           event-type: network-event-type,
           network-id: string,
           timestamp: u64,
       }

       enum network-event-type {
           joined,
           left,
           peer-added,
           peer-removed,
       }

       record connection-stats {
           rx-bytes: u64,
           tx-bytes: u64,
           rx-packets: u64,
           tx-packets: u64,
           connected-peers: u32,
           latency-ms: option<f64>,
           uptime-seconds: u64,
       }

       record plugin-error {
           code: string,
           message: string,
       }

       record plugin-manifest {
           id: string,
           name: string,
           version: string,
           author: string,
           description: string,
           capabilities: list<capability>,
       }
   }

   interface logging {
       log-trace: func(message: string);
       log-debug: func(message: string);
       log-info: func(message: string);
       log-warn: func(message: string);
       log-error: func(message: string);
   }

   interface config {
       get-string: func(key: string) -> option<string>;
       get-int: func(key: string) -> option<s64>;
       get-float: func(key: string) -> option<f64>;
       get-bool: func(key: string) -> option<bool>;
       keys: func() -> list<string>;
   }

   interface kv-store {
       set: func(key: string, value: list<u8>) -> result<_, string>;
       get: func(key: string) -> option<list<u8>>;
       delete: func(key: string) -> bool;
       list-keys: func() -> list<string>;
       clear: func() -> result<_, string>;
   }

   interface time {
       now-ms: func() -> u64;
       now-ns: func() -> u64;
   }

   interface base-plugin {
       use types.{plugin-manifest, plugin-error, capability};
       manifest: func() -> plugin-manifest;
       on-load: func() -> result<_, plugin-error>;
       on-unload: func() -> result<_, plugin-error>;
   }

   interface event-plugin {
       use types.{state-change-event, peer-info, network-event, connection-stats};
       on-state-change: func(event: state-change-event);
       on-peer-discovered: func(peer: peer-info);
       on-peer-disconnected: func(peer: peer-info);
       on-network-change: func(event: network-event);
       on-stats-update: func(stats: connection-stats);
   }

   world event-plugin-world {
       import logging;
       import config;
       import kv-store;
       import time;
       export base-plugin;
       export event-plugin;
   }
   ```

4. **Implement the plugin (`src/lib.rs`):**
   ```rust
   wit_bindgen::generate!({
       world: "event-plugin-world",
       path: "wit",
   });

   use omniedge::plugin::types::{
       Capability, ConnectionState, ConnectionStats, NetworkEvent,
       NetworkEventType, PeerInfo, PluginError, PluginManifest, StateChangeEvent,
   };
   use omniedge::plugin::logging;
   use exports::omniedge::plugin::base_plugin::Guest as BasePluginGuest;
   use exports::omniedge::plugin::event_plugin::Guest as EventPluginGuest;

   struct MyPlugin;

   impl BasePluginGuest for MyPlugin {
       fn manifest() -> PluginManifest {
           PluginManifest {
               id: "com.mycompany.my-plugin".to_string(),
               name: "My Plugin".to_string(),
               version: "0.1.0".to_string(),
               author: "My Company".to_string(),
               description: "My first OmniEdge plugin".to_string(),
               capabilities: vec![Capability::EventHooks],
           }
       }

       fn on_load() -> Result<(), PluginError> {
           logging::log_info("Plugin loaded!");
           Ok(())
       }

       fn on_unload() -> Result<(), PluginError> {
           logging::log_info("Plugin unloading!");
           Ok(())
       }
   }

   impl EventPluginGuest for MyPlugin {
       fn on_state_change(event: StateChangeEvent) {
           logging::log_info(&format!("State changed to {:?}", event.new_state));
       }

       fn on_peer_discovered(peer: PeerInfo) {
           logging::log_info(&format!("Peer discovered: {}", peer.virtual_ip));
       }

       fn on_peer_disconnected(peer: PeerInfo) {
           logging::log_warn(&format!("Peer disconnected: {}", peer.virtual_ip));
       }

       fn on_network_change(event: NetworkEvent) {
           logging::log_info(&format!("Network event on {}", event.network_id));
       }

       fn on_stats_update(stats: ConnectionStats) {
           logging::log_debug(&format!("Stats: {} peers", stats.connected_peers));
       }
   }

   export!(MyPlugin);
   ```

5. **Build the plugin:**
   ```bash
   cargo component build --release
   ```

   Output: `target/wasm32-wasip1/release/my_plugin.wasm`

---

## Plugin Types

OmniEdge supports 7 plugin categories, each with specific interfaces and use cases:

### 1. Event Hooks (`event-plugin`)

React to VPN lifecycle events for logging, automation, and integration.

| Event | Description |
|-------|-------------|
| `on_state_change` | VPN connects, disconnects, or changes state |
| `on_peer_discovered` | New peer found on the network |
| `on_peer_disconnected` | Peer leaves the network |
| `on_network_change` | Join/leave network events |
| `on_stats_update` | Periodic connection statistics |

**Use Cases:**
- Slack/Teams notifications on connect/disconnect
- Audit logging to SIEM systems
- Custom telemetry collection

### 2. Authentication (`auth-plugin`)

Custom authentication providers for enterprise SSO.

| Method | Description |
|--------|-------------|
| `supported_methods` | List supported auth methods |
| `authenticate` | Initiate authentication flow |
| `refresh_token` | Refresh expired tokens |

**Use Cases:**
- SAML/Okta/Azure AD integration
- Hardware token authentication
- Custom enterprise identity providers

### 3. Network Policy (`policy-plugin`)

Automatic network and exit node selection based on context.

| Method | Description |
|--------|-------------|
| `select_network` | Choose which network to join |
| `select_exit_node` | Choose exit node for routing |
| `validate_connection` | Allow/deny connections |
| `on_context_change` | React to policy context changes |

**Use Cases:**
- Geo-based network selection
- Time-of-day routing policies
- Compliance-driven exit node selection

### 4. QoS Enforcement (`qos-plugin`)

Traffic classification and prioritization.

| Method | Description |
|--------|-------------|
| `classify_packet` | Classify packet into traffic class |

**Traffic Classes:**
- `ultra-reliable-low-latency` - Teleop control, safety heartbeats (<10ms)
- `standard` - Routine telemetry
- `background` - Bulk logs, OTA downloads
- `drop` - Non-compliant traffic

**Use Cases:**
- Robotics teleop prioritization
- VoIP/video traffic classification
- DSCP tagging for network QoS

### 5. Data Triage (`data-triage-plugin`)

High-bandwidth sensor data buffering for robotics.

| Method | Description |
|--------|-------------|
| `configure_buffer` | Set up ring buffer |
| `register_triggers` | Define trigger conditions |
| `on_trigger` | Handle trigger events |

**Use Cases:**
- ROS2 bag recording on incident
- Edge-side data reduction
- Bandwidth-aware telemetry

### 6. PdM Reporting (`pdm-plugin`)

Predictive maintenance for actuators and motors.

| Method | Description |
|--------|-------------|
| `configure_monitoring` | Set up monitoring parameters |
| `on_actuator_sample` | Process actuator data |
| `compute_health_report` | Generate health report |

**Use Cases:**
- Motor health monitoring
- Bearing wear prediction
- Preventive maintenance scheduling

### 7. Compliance/FL (`compliance-plugin`)

Privacy compliance and federated learning.

| Method | Description |
|--------|-------------|
| `check_compliance` | Check data compliance |
| `get_compliance_mode` | Get current mode (GDPR/HIPAA) |

**Use Cases:**
- GDPR video anonymization
- HIPAA data handling
- Federated learning coordination

---

## Development Guide

### Project Structure

```
my-plugin/
├── Cargo.toml           # Package manifest with component metadata
├── wit/
│   └── world.wit        # WIT interface definitions
├── src/
│   └── lib.rs           # Plugin implementation
├── manifest.toml        # OmniEdge plugin manifest (optional)
└── README.md
```

### Using Host APIs

#### Logging

```rust
use omniedge::plugin::logging;

logging::log_trace("Trace message");
logging::log_debug("Debug message");
logging::log_info("Info message");
logging::log_warn("Warning message");
logging::log_error("Error message");
```

#### Configuration

```rust
use omniedge::plugin::config;

// Read configuration values
if let Some(value) = config::get_string("my-setting") {
    logging::log_info(&format!("Setting: {}", value));
}

let enabled = config::get_bool("feature-enabled").unwrap_or(false);
let threshold = config::get_float("threshold").unwrap_or(0.5);
let count = config::get_int("max-count").unwrap_or(100);

// List all config keys
let keys = config::keys();
```

#### Key-Value Store

Persist state between plugin restarts:

```rust
use omniedge::plugin::kv_store;

// Store data
let data = b"my data".to_vec();
kv_store::set("my-key", data).expect("Failed to store");

// Retrieve data
if let Some(value) = kv_store::get("my-key") {
    let s = String::from_utf8(value).unwrap();
    logging::log_info(&format!("Retrieved: {}", s));
}

// Delete data
kv_store::delete("my-key");

// List all keys
let keys = kv_store::list_keys();

// Clear all data
kv_store::clear().expect("Failed to clear");
```

#### Time

```rust
use omniedge::plugin::time;

let now_ms = time::now_ms();  // Milliseconds since Unix epoch
let now_ns = time::now_ns();  // Nanoseconds since Unix epoch
```

### Error Handling

Return errors from lifecycle methods:

```rust
fn on_load() -> Result<(), PluginError> {
    if some_condition_fails {
        return Err(PluginError {
            code: "INIT_FAILED".to_string(),
            message: "Failed to initialize plugin".to_string(),
        });
    }
    Ok(())
}
```

### State Management

Use a static or thread-local for plugin state:

```rust
use std::sync::Mutex;

static STATE: Mutex<Option<PluginState>> = Mutex::new(None);

struct PluginState {
    counter: u64,
    last_event: Option<String>,
}

impl BasePluginGuest for MyPlugin {
    fn on_load() -> Result<(), PluginError> {
        let mut state = STATE.lock().unwrap();
        *state = Some(PluginState {
            counter: 0,
            last_event: None,
        });
        Ok(())
    }
}

impl EventPluginGuest for MyPlugin {
    fn on_state_change(event: StateChangeEvent) {
        let mut state = STATE.lock().unwrap();
        if let Some(ref mut s) = *state {
            s.counter += 1;
            s.last_event = Some(format!("{:?}", event.new_state));
        }
    }
}
```

---

## Host APIs

### Available Interfaces

| Interface | Description | Available In |
|-----------|-------------|--------------|
| `logging` | Log messages at various levels | All plugins |
| `config` | Read configuration values | All plugins |
| `kv-store` | Persist key-value data | All plugins |
| `time` | Get current timestamps | All plugins |
| `http` | Make HTTP requests (allowlisted) | Auth plugins |
| `vpn-state` | Query VPN state | Policy plugins |

### HTTP Client (Auth Plugins Only)

```rust
use omniedge::host::http::{HttpRequest, HttpMethod};

// Check if URL is allowed
if http::is_allowed("https://api.mycompany.com") {
    let request = HttpRequest {
        method: HttpMethod::Post,
        url: "https://api.mycompany.com/auth".to_string(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
        ],
        body: Some(b"{\"token\": \"...\"}".to_vec()),
        timeout_ms: Some(5000),
    };
    
    match http::send(request) {
        Ok(request_id) => { /* handle async response */ }
        Err(e) => logging::log_error(&format!("HTTP error: {}", e)),
    }
}
```

### VPN State (Policy Plugins Only)

```rust
use omniedge::host::vpn_state;

// Get current connection state
let state = vpn_state::get_state();

// Get connected peers
let peers = vpn_state::get_peers();

// Get available networks
let networks = vpn_state::get_networks();

// Get device info
if let Some(device) = vpn_state::get_device_info() {
    logging::log_info(&format!("Device: {}", device.name));
}

// Get connection stats
let stats = vpn_state::get_stats();
```

---

## Building Plugins

### Debug Build

```bash
cargo component build
```

Output: `target/wasm32-wasip1/debug/my_plugin.wasm`

### Release Build

```bash
cargo component build --release
```

Output: `target/wasm32-wasip1/release/my_plugin.wasm`

### Build Verification

Check the WASM component:

```bash
# Check file size
ls -la target/wasm32-wasip1/release/*.wasm

# Inspect component (requires wasm-tools)
wasm-tools component wit target/wasm32-wasip1/release/my_plugin.wasm
```

### Optimizing Size

Add to `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true           # Link-time optimization
codegen-units = 1    # Single codegen unit
strip = true         # Strip symbols
```

---

## Plugin Manifest

Create `manifest.toml` to describe your plugin:

```toml
[plugin]
id = "com.mycompany.my-plugin"
name = "My Plugin"
version = "0.1.0"
author = "My Company"
description = "Description of what the plugin does"
license = "Apache-2.0"
homepage = "https://github.com/mycompany/my-plugin"
repository = "https://github.com/mycompany/my-plugin"
icon = "assets/icon.png"

[plugin.capabilities]
event-hooks = true
authentication = false
network-policy = false
data-triage = false
qos-enforcement = false
pdm-reporting = false
compliance = false
ui-widgets = false

[plugin.limits]
max-memory-mb = 16
max-execution-time-ms = 50

[config]
log-level = { type = "string", default = "info", description = "Logging level" }
webhook-url = { type = "string", description = "Webhook URL for notifications" }
enabled = { type = "bool", default = true, description = "Enable notifications" }

[dependencies]
# other-plugin = ">=1.0.0"

[http-allowlist]
# hosts = ["api.mycompany.com", "hooks.slack.com"]
```

### Manifest Fields

| Field | Required | Description |
|-------|----------|-------------|
| `id` | Yes | Unique identifier (reverse domain) |
| `name` | Yes | Human-readable name |
| `version` | Yes | Semantic version |
| `author` | Yes | Author or organization |
| `description` | Yes | Short description |
| `license` | No | SPDX license identifier |
| `homepage` | No | Plugin homepage URL |
| `repository` | No | Source repository URL |
| `icon` | No | Path to icon file |
| `capabilities` | Yes | Required capabilities |
| `limits` | No | Resource limits |
| `config` | No | Configuration schema |
| `dependencies` | No | Plugin dependencies |
| `http-allowlist` | No | Allowed HTTP hosts |

---

## Installation & Deployment

### Plugin Directory Structure

```
~/.omniedge/
└── plugins/
    ├── installed/
    │   └── com.mycompany.my-plugin/
    │       ├── plugin.wasm
    │       ├── manifest.toml
    │       └── assets/
    │           └── icon.png
    ├── data/
    │   └── com.mycompany.my-plugin/
    │       └── kv-store.json
    └── config/
        └── plugins.json
```

### Manual Installation

```bash
# Create plugin directory
mkdir -p ~/.omniedge/plugins/installed/com.mycompany.my-plugin/

# Copy plugin files
cp target/wasm32-wasip1/release/my_plugin.wasm \
   ~/.omniedge/plugins/installed/com.mycompany.my-plugin/plugin.wasm
cp manifest.toml \
   ~/.omniedge/plugins/installed/com.mycompany.my-plugin/
```

### CLI Installation (Future)

```bash
# Install from file
omniedge plugin install ./my-plugin.wasm

# Install from registry
omniedge plugin install com.mycompany.my-plugin

# Enable/disable
omniedge plugin enable com.mycompany.my-plugin
omniedge plugin disable com.mycompany.my-plugin

# List plugins
omniedge plugin list

# Remove plugin
omniedge plugin remove com.mycompany.my-plugin
```

---

## Security Model

### Sandbox Constraints

| Resource | Default Limit | Description |
|----------|---------------|-------------|
| Memory | 64 MB | Maximum WASM linear memory |
| Execution Time | 100 ms | Per-callback timeout |
| Fuel | 1,000,000 | Instruction count limit |
| File System | None | No file system access |
| Network | Allowlist only | HTTP to allowlisted hosts only |

### Capability-Based Access

Plugins only have access to interfaces they declare:

```toml
[plugin.capabilities]
event-hooks = true      # Can receive VPN events
authentication = false  # Cannot handle auth
network-policy = false  # Cannot set policies
```

### Signature Verification

Production deployments can require signed plugins:

```toml
# In OmniEdge config
[plugins]
require_signatures = true
trusted_signers = [
    "omniedge-official-key",
    "mycompany-plugin-key",
]
```

### HTTP Allowlist

Auth plugins must declare allowed hosts:

```toml
[http-allowlist]
hosts = [
    "api.mycompany.com",
    "login.microsoftonline.com",
]
```

---

## Troubleshooting

### Common Issues

#### 1. "cargo component not found"

```bash
# Install cargo-component
cargo install cargo-component

# If it times out, re-run (continues from cache)
cargo install cargo-component
```

#### 2. "wasm32-wasip1 target not installed"

```bash
rustup target add wasm32-wasip1
```

#### 3. WIT parse errors

Check your WIT syntax:
- Types must be inside `interface` blocks
- Use `record`, `enum`, `variant` correctly
- Match function signatures exactly

#### 4. Import path errors

After building, check generated bindings:
```bash
cat src/bindings.rs | head -200
```

Use the correct module paths from the generated code.

#### 5. Plugin not loading

Check logs:
```bash
omniedge logs --filter plugin
```

Common causes:
- Invalid manifest
- Missing capabilities
- WASM validation failure

### Debug Tips

1. **Enable debug logging:**
   ```rust
   logging::log_debug("Variable value: ...");
   ```

2. **Check plugin state:**
   ```bash
   cat ~/.omniedge/plugins/config/plugins.json
   ```

3. **Validate WASM:**
   ```bash
   wasm-tools validate target/wasm32-wasip1/release/my_plugin.wasm
   ```

4. **Test locally:**
   Build and test the plugin in isolation before installing.

---

## Example Plugins

### Hello World

Basic event logging plugin demonstrating all event hooks:
- Location: [examples/plugins/hello-world/](../examples/plugins/hello-world/)
- Features: Logs state changes, peer discovery, network events, stats

```bash
# Build the example
cd examples/plugins/hello-world
cargo component build --release
```

---

## API Reference

See the full WIT interface definitions in the omni-plugin crate:
- Types: `crates/omni-plugin/wit/types.wit`
- Host Functions: `crates/omni-plugin/wit/host.wit`
- Plugin Interfaces: `crates/omni-plugin/wit/plugin.wit`

---

## License

Apache-2.0 - See [LICENSE](../LICENSE) for details.
