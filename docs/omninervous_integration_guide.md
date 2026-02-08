# OmniNervous Integration Guide

> **Version**: v0.8.8 (February 2026)  
> **Status**: Production Ready

This document outlines the key changes and integration points for OmniNervous since v0.8.0, providing guidance for implementing and leveraging the new features in the OmniEdge ecosystem.

---

## Table of Contents

1. [Version Summary](#version-summary)
2. [Key Features Since v0.8.0](#key-features-since-v080)
3. [Automatic MTU Detection](#automatic-mtu-detection)
4. [Daemon State Management](#daemon-state-management)
5. [Configuration Options](#configuration-options)
6. [CLI Reference](#cli-reference)
7. [Integration Examples](#integration-examples)
8. [Troubleshooting](#troubleshooting)

---

## Version Summary

| Version | Release Date | Key Changes |
|:--------|:-------------|:------------|
| **v0.8.8** | 2026-02-08 | Windows build fix (MTU parameter) |
| **v0.8.7** | 2026-02-08 | Automatic MTU heuristic, daemon state fixes |
| **v0.8.6** | 2026-02-08 | Relay fallback improvements, cloud_test.sh --local-docker |
| **v0.8.5** | 2026-02-08 | High-efficiency zero-copy release (147.6% efficiency) |
| **v0.8.0** | 2026-02-07 | Performance baseline, zero-copy transmission |

---

## Key Features Since v0.8.0

### 1. Automatic MTU Detection (v0.8.7)

OmniNervous now automatically detects when running behind a secondary VPN and adjusts the MTU accordingly to prevent fragmentation issues.

**Problem Solved**: Users running OmniNervous inside another VPN tunnel (corporate VPN, NordVPN, etc.) experienced "black hole" connections where pings worked but large transfers failed.

**Solution**: The daemon heuristically detects existing VPN interfaces and reduces MTU from 1420 to 1280.

### 2. Daemon State Persistence (v0.8.7)

Critical fix for Edge mode reliability:

- **Before**: `pending_pings` and `pending_races` were reset on every loop iteration
- **After**: State persists across the main event loop, enabling proper P2P discovery and relay fallback

### 3. Zero-Copy Transmission (v0.8.5)

Achieved **147.6% baseline efficiency** through:
- Optimized UDP pipelines
- Reduced memory allocations in packet processing
- Direct buffer passing without intermediate copies

---

## Automatic MTU Detection

### How It Works

```
┌─────────────────────────────────────────────────────────────┐
│                     MTU Decision Flow                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. Check --mtu CLI flag                                    │
│     ├─ "auto" → Run VPN detection                          │
│     ├─ <number> → Use explicit value                       │
│     └─ None → Check config file                            │
│                                                             │
│  2. Check config.network.mtu                                │
│     ├─ Some(value) → Use config value                      │
│     └─ None → Run VPN detection                            │
│                                                             │
│  3. VPN Detection Heuristic                                 │
│     ├─ Linux: Check /sys/class/net for tun/wg/tap/ppp      │
│     ├─ macOS: Check ifconfig for utun/ppp                  │
│     └─ Windows: (Future support)                           │
│                                                             │
│  4. Result                                                  │
│     ├─ VPN detected → MTU 1280                             │
│     └─ No VPN → MTU 1420                                   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Detected Interface Prefixes

| OS | Interface Prefixes | Examples |
|:---|:---|:---|
| Linux | `tun`, `wg`, `tap`, `ppp` | `tun0`, `wg0`, `tap0`, `ppp0` |
| macOS | `utun`, `ppp` | `utun0`, `utun1`, `ppp0` |

### Usage Examples

```bash
# Automatic detection (recommended for VPN-over-VPN)
omninervous --nucleus host:51820 --vip 10.200.0.1 --mtu auto

# Force low MTU for problematic networks
omninervous --nucleus host:51820 --vip 10.200.0.1 --mtu 1280

# Force standard MTU for clean networks
omninervous --nucleus host:51820 --vip 10.200.0.1 --mtu 1420
```

---

## Daemon State Management

### Architecture Change

**Before v0.8.7** (Broken):
```rust
loop {
    let mut pending_pings = HashMap::new();  // Reset every iteration!
    let mut pending_races = HashMap::new();  // Reset every iteration!
    let handler = MessageHandler::new(&mut pending_pings, ...);
    // State lost on next iteration
}
```

**After v0.8.7** (Fixed):
```rust
let mut pending_pings = HashMap::new();  // Persists!
let mut pending_races = HashMap::new();  // Persists!

loop {
    tokio::select! {
        _ = fast_interval.tick() => {
            let handler = MessageHandler::new(&mut pending_pings, ...);
            handler.advance_pending_races().await;
            handler.check_relay_fallback().await;
        }
        // ...
    }
}
```

### Impact on P2P Discovery

| Behavior | Before v0.8.7 | After v0.8.7 |
|:---|:---|:---|
| Disco pings | Lost after first tick | Properly tracked |
| Happy Eyeballs races | Never completed | Complete correctly |
| Relay fallback | Never triggered | Triggers after timeout |
| Peer latency tracking | Always reset | Accumulates correctly |

---

## Configuration Options

### Config File (`config.toml`)

```toml
[network]
nucleus = "nucleus.example.com:51820"
cluster = "my-network"

# MTU Configuration (NEW in v0.8.7)
# Options: 
#   - None (default): Auto-detect
#   - 1420: Standard WireGuard MTU
#   - 1280: Safe for VPN-over-VPN
mtu = 1420

# Existing options
prefer_ipv6 = true
happy_eyeballs_delay_ms = 250
stun_servers = ["stun.l.google.com:19302"]
use_builtin_stun = true

[daemon]
port = 51820
interface = "eth0"
log_level = "info"
```

---

## CLI Reference

### New Flags in v0.8.7

| Flag | Type | Default | Description |
|:---|:---|:---|:---|
| `--mtu` | String | Auto-detect | Interface MTU: number or "auto" |

### Full CLI Reference

```
omninervous [OPTIONS]

Options:
  -p, --port <PORT>           UDP port [default: 51820]
  -m, --mode <MODE>           Run mode: 'nucleus' or omit for edge
  -n, --nucleus <NUCLEUS>     Nucleus server address (host:port)
  -c, --cluster <CLUSTER>     Cluster/network name
      --secret <SECRET>       Cluster secret (min 16 chars)
      --init                  Initialize new identity and exit
      --identity <PATH>       Path to identity file
      --config <PATH>         Path to config file
      --vip <VIP>             Virtual IP (e.g., 10.200.0.1)
      --vip6 <VIP6>           IPv6 Virtual IP
      --userspace             Use BoringTun userspace implementation
  -s, --stun <STUN>           STUN servers (repeatable)
      --disable-builtin-stun  Disable Nucleus STUN fallback
      --mtu <MTU>             Interface MTU: number or "auto"
      --transport-mode <MODE> Transport mode: "l3" or "l2"
      --l2-mtu <L2_MTU>       L2 TAP MTU (Linux only)
  -h, --help                  Print help
  -V, --version               Print version
```

---

## Integration Examples

### 1. Edge Node Behind Corporate VPN

```bash
# The user is connected to a corporate VPN (e.g., Cisco AnyConnect)
# OmniNervous automatically detects the existing tunnel and adjusts MTU

sudo omninervous \
  --nucleus vpn.example.com:51820 \
  --cluster robotics-lab \
  --secret "MySecureSecret123" \
  --vip 10.200.0.5 \
  --mtu auto \
  --userspace
```

### 2. IoT Gateway with Fixed MTU

```bash
# For embedded devices with known network constraints

sudo omninervous \
  --nucleus gateway.iot.local:51820 \
  --cluster factory-floor \
  --vip 10.200.1.100 \
  --mtu 1280 \
  --transport-mode l2
```

### 3. Cloud Test Deployment

```bash
# Using the updated cloud_test.sh with auto MTU

./scripts/cloud_test.sh \
  --nucleus 104.x.x.x \
  --node-a 54.x.x.x \
  --node-b 35.x.x.x \
  --ssh-key ~/.ssh/cloud.pem \
  --secret "test-secret-16ch" \
  --userspace
```

The script now automatically passes `--mtu auto` to all nodes.

---

## Troubleshooting

### Issue: Connection works for small packets, fails for large transfers

**Symptom**: `ping` works, but `iperf3` or `ssh` hangs.

**Cause**: MTU is too large, causing fragmentation or black-holing.

**Solution**:
```bash
# Force safe MTU
omninervous --mtu 1280 ...

# Or use automatic detection
omninervous --mtu auto ...
```

### Issue: P2P connection never establishes, always uses relay

**Symptom**: Logs show "Using relay for peer X" even on open networks.

**Cause**: (Before v0.8.7) State reset bug prevented disco pings from completing.

**Solution**: Upgrade to v0.8.7 or later.

### Issue: "Environment: Possible VPN detected" appears unexpectedly

**Symptom**: MTU is set to 1280 when no VPN is running.

**Cause**: Docker or other software created a `tun`/`tap` interface.

**Solution**:
```bash
# Override with explicit MTU
omninervous --mtu 1420 ...
```

---

## Performance Benchmarks

### v0.8.7 Results (AWS Lightsail, Cross-Region)

| Metric | Value | Notes |
|:---|---:|:---|
| **Throughput (IPv4)** | 662.38 Mbps | 147.6% of baseline |
| **Throughput (IPv6)** | 632.25 Mbps | Dual-stack support |
| **Latency** | 46.63 ms | us-east-1 ↔ us-east-2 |
| **Efficiency** | 147.6% | Zero-copy optimizations |

### MTU Impact on Throughput

| MTU | Overhead | Best For |
|:---|:---|:---|
| 1420 | ~3% | Clean networks, maximum speed |
| 1280 | ~5% | VPN-over-VPN, cellular, satellite |
| 1200 | ~7% | Extreme compatibility mode |

---

## Migration Checklist

When upgrading from v0.8.0 to v0.8.8:

- [ ] Update binary to v0.8.8
- [ ] Review logs for "Auto-MTU" messages
- [ ] Add `--mtu auto` to startup scripts if behind VPN
- [ ] Verify P2P connections establish (not just relay)
- [ ] Check `/metrics` endpoint for new MTU-related metrics (future)

---

*© 2026 OmniEdge Inc. Engineering the nervous system of the future.*
