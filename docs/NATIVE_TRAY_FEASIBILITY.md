# Feasibility Study: Native Tray Conversion for OmniEdge

This report evaluates the transition of the OmniEdge Desktop application from a custom-drawn UI window to a **Native System Tray application** with dynamic sub-menus.

## 1. Executive Summary
Converting to a native tray application will significantly improve the "background" feel of OmniEdge on Windows and macOS. By leveraging Tauri v2's native menu capabilities, we can provide quick access to virtual networks and device statuses without opening the main dashboard.

## 2. Mockup Concepts (Banana Pro)

````carousel
![Native Tray Concept v1](/C:/Users/yongq/.gemini/antigravity/brain/46e9f728-0182-430d-8ea3-a4d13202c0c8/native_tray_menu_mockup_v1_1769515911364.png)
<!-- slide -->
![Comparison: Custom UI vs Native Menu](/C:/Users/yongq/.gemini/antigravity/brain/46e9f728-0182-430d-8ea3-a4d13202c0c8/native_tray_menu_mockup_v2_comparison_1769515935943.png)
````

## 3. Technical Review: `tauri-tray-app` vs `omniedge`

| Component          | `tauri-tray-app` (Reference) | `omniedge` (Target)         |
| :----------------- | :--------------------------- | :-------------------------- |
| **Tauri Version**  | v1.x (`SystemTray`)          | v2.x (`TrayIcon`, `Menu`)   |
| **Menu Strategy**  | Static + Dynamic Rebuilds    | High-performance `Menu` API |
| **OS Integration** | Basic Left/Right Click       | full System Tray support    |

## 4. Proposed Implementation Architecture

### A. Dynamic Menu Building
Instead of a static menu, we will implement a `build_tray_menu` function in Rust that:
1.  Fetches networks via `ConnectionManager`.
2.  Creates a `Submenu` for each network.
3.  Populates sub-menus with `MenuItem`s for each device (with status indicators).
4.  Adds a "Connect/Disconnect" toggle at the network level.

### B. Event Bridging
Native menu clicks will trigger the same business logic as the React UI:
- `MenuItemClick(id)` where `id` is `connect_{network_id}`.
- This ensures consistency between the Tray and the Window.

### C. Resource Optimization
By moving the primary interaction to the native tray, we can keep the WebView (React) hidden or even uninitialized until explicitly requested, reducing memory footprint by ~60-100MB while idle.

## 5. Feasibility Verdict: **Highly Feasible**
Tauri v2 provides first-class support for this. The main effort will be in the Rust layer to handle dynamic menu rebuilding when network states change (e.g., a device goes offline).

**Recommended Next Steps:**
1. Refactor `lib.rs` to move menu construction into a separate module.
2. Implement a listener for `ConnectionState` changes to trigger menu refreshes.
3. Add native sub-menu support for Virtual Networks.
