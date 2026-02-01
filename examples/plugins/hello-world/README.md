# Hello World OmniEdge Plugin

An example event plugin demonstrating the OmniEdge plugin system.

## Overview

This plugin implements the `event-plugin-world` interface and logs all VPN lifecycle events:

- **State Changes**: When the VPN connects, disconnects, or changes state
- **Peer Discovery**: When new peers are found on the network
- **Peer Disconnection**: When peers leave the network
- **Network Events**: When joining/leaving networks
- **Statistics Updates**: Periodic connection statistics

## Building

### Prerequisites

1. Install Rust and the WASM target:
   ```bash
   rustup target add wasm32-wasip1
   ```

2. Install cargo-component (this may take 5-10 minutes on first install):
   ```bash
   cargo install cargo-component
   ```

   > **Note**: cargo-component has many dependencies and takes a while to compile.
   > If installation times out, simply re-run the command - it will continue from where it left off.

### Build

```bash
# Navigate to the plugin directory
cd examples/plugins/hello-world

# Debug build
cargo component build

# Release build  
cargo component build --release
```

The compiled plugin will be at:
- Debug: `target/wasm32-wasip1/debug/hello_world_plugin.wasm`
- Release: `target/wasm32-wasip1/release/hello_world_plugin.wasm`

### Verification (without cargo-component)

To verify the Rust code syntax without building the WASM component:
```bash
# This checks syntax but won't verify WIT bindings
cargo check --target wasm32-wasip1
```

## Installation

1. Copy the `.wasm` file and `manifest.toml` to your OmniEdge plugins directory:
   ```bash
   mkdir -p ~/.omniedge/plugins/installed/com.omniedge.hello-world/
   cp target/wasm32-wasip1/release/hello_world_plugin.wasm ~/.omniedge/plugins/installed/com.omniedge.hello-world/plugin.wasm
   cp manifest.toml ~/.omniedge/plugins/installed/com.omniedge.hello-world/
   ```

2. Enable the plugin in OmniEdge settings or via CLI:
   ```bash
   omniedge plugin enable com.omniedge.hello-world
   ```

## Development

### Project Structure

```
hello-world/
├── Cargo.toml          # Rust package manifest with WASM component config
├── manifest.toml       # OmniEdge plugin manifest
├── README.md           # This file
└── src/
    └── lib.rs          # Plugin implementation
```

### Key Concepts

1. **WIT Bindings**: The `wit_bindgen::generate!` macro generates Rust bindings from the WIT interface definitions.

2. **Plugin Traits**: Implement `BasePluginGuest` for lifecycle hooks and `EventPluginGuest` for event handlers.

3. **Host Functions**: Use `omniedge::host::logging` to log messages that appear in the OmniEdge logs.

### Extending

To add more functionality:

1. Add configuration options in `manifest.toml`
2. Use the `config` host interface to read configuration values
3. Use the `kv-store` host interface to persist state between restarts

## License

Apache-2.0
