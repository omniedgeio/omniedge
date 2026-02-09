# Implementation Plan - Fix P2P NAT Connectivity & Signaling Issues

## 1. Issue Analysis
Based on test results (`cloud_test_20260209_170609.json`) and logs, Edge A (macOS) is unable to connect to Edge B. Direct P2P ping fails (N/A) and throughput is 0. 

Key findings:
- **Signaling Silent Failure**: Edge A sends `REGISTER` to Nucleus but never logs `Received REGISTER_ACK`. This means it NEVER receives peer information for Edge B.
- **Potential Encryption Bug**: `omni-proto` (v2.5.0) dispatcher does not handle `MSG_ENCRYPTED` (0x1D), even though `Encrypted Signaling` is enabled by default. If the server sends encrypted signaling, it is silently ignored.
- **HMAC Secret Mismatch**: The API `secret_key` is a hex string. `OmniNervous` uses it as-is for HMAC, which might mismatch if the backend expects raw bytes.
- **Missing Error Logs**: The dispatcher loop in `manager.rs` swallows errors from signaling packet handling, making it hard to diagnose HMAC or decryption failures.

## 2. Proposed Changes

### A. crates/omni-proto/src/lib.rs
1. **Support Encrypted Signaling**:
   - Add a `signaling_encryption` state to `OmniProto`.
   - Update `handle_packet` to detect `MSG_ENCRYPTED` (0x1D).
   - If encrypted, decrypt the packet before processing with `parse_register_ack` or `parse_heartbeat_ack`.
2. **Hex-to-Bytes Secret Key**:
   - Add a utility or use `hex` crate (if available, otherwise manual) to decode the hex secret key to 32 bytes for the `SignalingEncryption` context.

### B. crates/omni-core/src/manager.rs
1. **Dispatcher Error Logging**: 
   - Update the dispatcher loop to log errors returned by `proto_ctrl.handle_packet`.
   - Add a warning log when an unknown signaling message is received (especially type `0x1D` if still unhandled).
2. **NAT Strategy Monitoring**:
   - Add more descriptive logs when a connection strategy is selected.

### C. crates/omni-cli/src/main.rs (or relevant log collection script)
1. **Fix Log Collection**:
   - Update `cloud_test.sh` to correctly collect timestamped daemon logs instead of looking for a static `omniedge.log`.

## 3. Detailed Steps

### Phase 1: Dispatcher Logging (Diagnostic)
- Modify `crates/omni-core/src/manager.rs` to log errors from `proto_ctrl.handle_packet`.
- This will confirm whether we are receiving packets but failing validation.

### Phase 2: Fix Signaling Dispatcher in `omni-proto`
- Update `handle_packet` to support `MSG_ENCRYPTED`.
- Ensure `parse_*_ack` are called with the right secret (raw vs hex).

### Phase 3: Verification
- Run a local simulation or ask the user to re-run the cloud test with the fixes.

## 4. Risks & Mitigations
- **Compatibility**: If we change HMAC from hex-string to raw-bytes, we must ensure the backend (Nucleus) matches. We should verify Node B's behavior if possible.
- **Dependencies**: `omni-proto` might need `hex` crate added to `Cargo.toml`.
