# OmniEdge Migration: Detailed TODO

## Phase 0: Parameter & Flag Alignment
- [x] Review `n2n` and `OmniNervous` flags
    - [x] Map `CommunityName` -> `cluster`
    - [x] Map `SuperNode` -> `nucleus`
    - [x] Map `SecretKey` -> `secret` (PSK)
    - [x] Identify missing: `ExitNodeIP`, `IsExitNode`, `EnableRouting`
- [x] Document final parameter mapping for the Rust core
- [x] Verify CLI parity requirements (join, login, logout, scan, upload)

## Phase 1: Foundation (Rust Core - OmniNervous package)
- [x] **Refactor OmniNervous**
    - [x] Create `refactor/packaging` branch in OmniNervous
    - [x] Create `lib.rs` in `OmniNervous/crates/daemon/src`
    - [x] Export signaling, wg, identity, and peer modules
    - [x] Move CLI-specific logic (clap Parser) to `main.rs`
    - [x] Ensure `Config` can be initialized programmatically
- [x] Initialize Rust Workspace in `crates/`
- [x] Implement `omni-proto` library
    - [x] Use `omninervous` package as dependency
    - [x] Implement mapping for `CommunityName` and `SecretKey`
- [x] Implement `omni-tun` library
    - [x] Use `omninervous` data plane as dependency
    - [x] Implement interface setup and peer management
- [x] Implement `omni-api` library
    - [x] Port `AuthService` (OAuth2/PKCE) from Go
    - [x] Port `NetworkService` (Join/List) from Go

## Phase 2: Connection Manager (The Glue)
- [x] Implement a unified "Connection State Machine" in Rust
- [x] Handle transition: Login -> Join Network -> Signaling Bind -> TUN Up
- [x] Port routing logic from `pkg/core/routing.go` to native Rust

## Phase 3: Desktop Migration (Tauri v2)
- [x] Initialize Tauri v2 in `ui/desktop`
- [x] Port existing React/Vite assets and official icons from legacy codebase
- [x] Implement System Tray and Popover using Tauri native plugins
- [x] Replace bridge-service IPC with Tauri `invoke` commands

## Phase 4: System Logic & Networking
- [x] Global Routing Engine
    - [x] Implement exit node logic in Rust
    - [x] Platform-specific DNS configuration (Linux, macOS, Windows)
- [x] Connection Lifecycle
    - [x] State machine for Auto-reconnect/Handshake timeout

## Phase 5: Licensing & Disposal [DELETE GPL3]
- [x] Verify License compliance (Apache 2.0 / MIT)
- [x] Update `LICENSE` and `LICENSING.md`
- [x] Remove legacy `pkg/`, `cmd/`, `protocol/`, and `n2n` code
- [x] Remove `go.mod`, `go.sum`, and `Makefile`

## Phase 6: Finalization
- [x] Documentation for new architecture (`doc/ARCHITECTURE.md`)
- [x] Multi-platform release builds (CI/CD)
- [x] Mobile shells (iOS/Android) initialization scaffolding

- [/] Cross-platform Makefile
    - [/] CLI: Linux, macOS, Windows (amd64/arm64)
        - [x] Windows (amd64)
    - [x] Desktop: Linux, macOS, Windows (amd64/arm64)
- [ ] CI/CD Pipeline optimization
