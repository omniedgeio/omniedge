# OmniEdge Plugin Management UI/UX Proposal

**Version:** 1.0  
**Date:** January 2026  
**Status:** Design Proposal

---

## Executive Summary

This document proposes a UX-friendly plugin management interface for the OmniEdge desktop tray application. The design follows existing patterns from `App.tsx` and `App.css`, maintaining the 320px fixed-width tray menu style with macOS-native aesthetics.

---

## Design Principles

1. **Consistency** - Match existing collapsible sections (Exit Nodes pattern)
2. **Minimal Footprint** - Plugins section collapses when not in use
3. **Progressive Disclosure** - List view → Details view → Settings view
4. **Accessibility** - Keyboard navigation, ARIA labels, focus states
5. **Platform Agnostic** - Works on macOS, Windows, Linux with platform fonts

---

## User Flows

### Flow 1: Browse Installed Plugins
```
Dashboard → Click "Plugins" section → View plugin list → Toggle enable/disable
```

### Flow 2: View Plugin Details
```
Plugin list → Click plugin row → Expand inline details (name, version, author, type)
```

### Flow 3: Configure Plugin Settings
```
Plugin details → Click "Settings" → Show plugin-specific config fields
```

### Flow 4: Install New Plugin
```
Plugin list → Click "+" button → Native file picker → Load .wasm file
```

### Flow 5: Remove Plugin
```
Plugin details → Click "Remove" → Confirmation → Plugin uninstalled
```

---

## Wireframes

### Collapsed State (Default)
```
┌──────────────────────────────────────────────────┐
│  ... existing dashboard content ...              │
├──────────────────────────────────────────────────┤
│  ▸ Exit Nodes                                    │
├──────────────────────────────────────────────────┤
│  ▸ Plugins                              (2)  [+] │
└──────────────────────────────────────────────────┘
```
- Chevron indicates collapsible section
- Badge shows active plugin count
- [+] button opens file picker for install

### Expanded State - Plugin List
```
┌──────────────────────────────────────────────────┐
│  ▾ Plugins                              (2)  [+] │
├──────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────┐    │
│  │  ● Auth Provider                    [ON] │    │
│  │    OAuth 2.0 integration              ▸  │    │
│  └──────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────┐    │
│  │  ○ Traffic Monitor               [OFF]   │    │
│  │    Real-time bandwidth stats          ▸  │    │
│  └──────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────┐    │
│  │  ⚠ Compliance Logger             [ERR]   │    │
│  │    Error: Config missing              ▸  │    │
│  └──────────────────────────────────────────┘    │
├──────────────────────────────────────────────────┤
│  ... footer ...                                  │
└──────────────────────────────────────────────────┘
```

**Legend:**
- ● = Enabled (green)
- ○ = Disabled (gray)
- ⚠ = Error (red)
- [ON]/[OFF] = iOS-style toggle switch
- ▸ = Expandable for details

### Expanded State - Plugin Details (Inline)
```
┌──────────────────────────────────────────────────┐
│  ▾ Plugins                              (2)  [+] │
├──────────────────────────────────────────────────┤
│  ┌──────────────────────────────────────────┐    │
│  │  ● Auth Provider                    [ON] │    │
│  │    OAuth 2.0 integration              ▾  │    │
│  ├──────────────────────────────────────────┤    │
│  │  VERSION     1.2.0                       │    │
│  │  AUTHOR      OmniEdge Team               │    │
│  │  TYPE        Authentication              │    │
│  │  PERMISSIONS network, config             │    │
│  ├──────────────────────────────────────────┤    │
│  │  [  Settings  ]     [  Remove  ]         │    │
│  └──────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────┐    │
│  │  ○ Traffic Monitor               [OFF]   │    │
│  │    Real-time bandwidth stats          ▸  │    │
│  └──────────────────────────────────────────┘    │
└──────────────────────────────────────────────────┘
```

### Plugin Settings View (Full Screen)
```
┌──────────────────────────────────────────────────┐
│  ◀ Back         Auth Provider Settings           │
├──────────────────────────────────────────────────┤
│                                                  │
│  OAUTH CONFIGURATION                             │
│  ┌──────────────────────────────────────────┐    │
│  │ Client ID                                │    │
│  │ ┌────────────────────────────────────┐   │    │
│  │ │ my-client-id-here                  │   │    │
│  │ └────────────────────────────────────┘   │    │
│  └──────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────┐    │
│  │ Auth Endpoint                            │    │
│  │ ┌────────────────────────────────────┐   │    │
│  │ │ https://auth.example.com           │   │    │
│  │ └────────────────────────────────────┘   │    │
│  └──────────────────────────────────────────┘    │
│  ┌──────────────────────────────────────────┐    │
│  │ Auto-refresh tokens              [ON]    │    │
│  └──────────────────────────────────────────┘    │
│                                                  │
│  [        Save Configuration        ]            │
│                                                  │
└──────────────────────────────────────────────────┘
```

---

## Component Breakdown

### New Components (Inline in App.tsx)

| Component | Description | Lines Est. |
|-----------|-------------|------------|
| `PluginsSection` | Collapsible container with header | ~30 |
| `PluginItem` | Single plugin row with toggle | ~50 |
| `PluginDetails` | Expanded info panel | ~40 |
| `PluginSettingsView` | Full-screen settings | ~80 |

**Total estimated:** ~200 lines added to App.tsx

### State Variables Required

```typescript
// Plugin management state
const [isPluginsExpanded, setIsPluginsExpanded] = useState(false);
const [plugins, setPlugins] = useState<PluginInfo[]>([]);
const [expandedPluginId, setExpandedPluginId] = useState<string | null>(null);
const [showPluginSettings, setShowPluginSettings] = useState(false);
const [activePluginSettings, setActivePluginSettings] = useState<string | null>(null);
const [pluginSettingsData, setPluginSettingsData] = useState<Record<string, any>>({});
const [isInstallingPlugin, setIsInstallingPlugin] = useState(false);
```

### Type Definitions

```typescript
interface PluginInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  plugin_type: 'event' | 'auth' | 'policy' | 'data_triage' | 'qos' | 'pdm' | 'compliance';
  enabled: boolean;
  status: 'active' | 'disabled' | 'error';
  error_message?: string;
  permissions: string[];
  config_schema?: PluginConfigField[];
}

interface PluginConfigField {
  key: string;
  label: string;
  field_type: 'text' | 'password' | 'boolean' | 'number' | 'select';
  default_value?: any;
  options?: string[];  // For select type
  required: boolean;
}
```

---

## CSS Classes Required

```css
/* Plugin Section - Collapsible Header (matches .collapsible-header) */
.plugins-section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 14px;
  cursor: pointer;
}

.plugin-count-badge {
  background: var(--accent-blue);
  color: white;
  padding: 2px 8px;
  border-radius: 10px;
  font-size: 10px;
  font-weight: 700;
  margin-right: 8px;
}

.plugin-add-btn {
  width: 22px;
  height: 22px;
  border-radius: 6px;
  background: rgba(0, 122, 255, 0.1);
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: 0.2s;
}

.plugin-add-btn:hover {
  background: rgba(0, 122, 255, 0.2);
}

/* Plugin List */
.plugins-list {
  padding: 0 10px 12px;
}

/* Plugin Item Card */
.plugin-item {
  margin: 4px 0;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.3);
  border: 0.5px solid var(--divider);
  overflow: hidden;
  transition: 0.2s;
}

@media (prefers-color-scheme: dark) {
  .plugin-item {
    background: rgba(255, 255, 255, 0.05);
  }
}

.plugin-item.is-enabled {
  background: rgba(52, 199, 89, 0.05);
  border-color: rgba(52, 199, 89, 0.2);
}

.plugin-item.has-error {
  background: rgba(255, 59, 48, 0.05);
  border-color: rgba(255, 59, 48, 0.2);
}

/* Plugin Item Header Row */
.plugin-item-header {
  padding: 10px 12px;
  display: flex;
  align-items: center;
  cursor: pointer;
}

.plugin-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  margin-right: 10px;
  flex-shrink: 0;
}

.plugin-status-dot.active {
  background: var(--status-green);
  box-shadow: 0 0 6px rgba(52, 199, 89, 0.4);
}

.plugin-status-dot.disabled {
  background: var(--divider);
}

.plugin-status-dot.error {
  background: var(--status-red);
  box-shadow: 0 0 6px rgba(255, 59, 48, 0.4);
}

.plugin-info {
  flex: 1;
  min-width: 0;
}

.plugin-name {
  font-size: 13px;
  font-weight: 600;
  margin-bottom: 2px;
}

.plugin-description {
  font-size: 11px;
  opacity: 0.5;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.plugin-error-text {
  font-size: 11px;
  color: var(--status-red);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* Plugin Toggle (reuses .ios-switch.small) */

/* Plugin Details Panel */
.plugin-details-panel {
  padding: 8px 12px 12px;
  border-top: 0.5px solid var(--divider);
  animation: reveal 0.2s ease-out;
}

.plugin-detail-row {
  display: flex;
  justify-content: space-between;
  padding: 4px 0;
}

.plugin-detail-label {
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  opacity: 0.4;
}

.plugin-detail-value {
  font-size: 11px;
  text-align: right;
  max-width: 180px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.plugin-permissions {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  justify-content: flex-end;
}

.permission-tag {
  background: rgba(0, 122, 255, 0.1);
  color: var(--accent-blue);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 9px;
  font-weight: 600;
}

/* Plugin Action Buttons */
.plugin-actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}

.plugin-action-btn {
  flex: 1;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 11px;
  font-weight: 600;
  border: none;
  cursor: pointer;
  transition: 0.2s;
}

.plugin-action-btn.settings {
  background: var(--accent-blue);
  color: white;
}

.plugin-action-btn.remove {
  background: rgba(255, 59, 48, 0.1);
  color: var(--status-red);
}

.plugin-action-btn.remove:hover {
  background: rgba(255, 59, 48, 0.2);
}

/* Plugin Settings View (Full Screen) */
.plugin-settings-view {
  padding: 0;
}

.plugin-settings-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  border-bottom: 0.5px solid var(--divider);
}

.plugin-settings-back {
  opacity: 0.5;
  cursor: pointer;
  transition: 0.2s;
}

.plugin-settings-back:hover {
  opacity: 1;
}

.plugin-settings-title {
  font-size: 13px;
  font-weight: 600;
}

.plugin-settings-content {
  padding: 14px;
}

.plugin-config-section {
  margin-bottom: 16px;
}

.plugin-config-label {
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  opacity: 0.4;
  margin-bottom: 8px;
}

.plugin-config-field {
  margin-bottom: 12px;
}

.plugin-config-field label {
  display: block;
  font-size: 11px;
  margin-bottom: 4px;
  opacity: 0.7;
}

.plugin-config-input {
  width: 100%;
  padding: 8px 10px;
  border: 0.5px solid var(--divider);
  border-radius: 6px;
  font-size: 12px;
  background: rgba(255, 255, 255, 0.5);
  transition: 0.2s;
}

@media (prefers-color-scheme: dark) {
  .plugin-config-input {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text-primary-dark);
  }
}

.plugin-config-input:focus {
  outline: none;
  border-color: var(--accent-blue);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}

.plugin-config-toggle-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
}

.plugin-save-btn {
  width: 100%;
  padding: 12px;
  background: var(--accent-blue);
  color: white;
  border: none;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: 0.2s;
  margin-top: 16px;
}

.plugin-save-btn:hover {
  box-shadow: 0 4px 12px rgba(0, 122, 255, 0.3);
}

/* Empty State */
.plugins-empty {
  padding: 24px;
  text-align: center;
  opacity: 0.4;
}

.plugins-empty-icon {
  margin-bottom: 8px;
}

.plugins-empty-text {
  font-size: 12px;
}
```

---

## Tauri Commands Required

### Backend Interface (Rust)

```rust
// Add to ui/desktop/src-tauri/src/lib.rs

use omni_plugin::{PluginManager, PluginInfo, PluginConfig};

#[tauri::command]
async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<PluginInfo>, String> {
    let manager = state.plugin_manager.lock().await;
    Ok(manager.list_plugins())
}

#[tauri::command]
async fn get_plugin_info(state: State<'_, AppState>, plugin_id: String) -> Result<PluginInfo, String> {
    let manager = state.plugin_manager.lock().await;
    manager.get_plugin(&plugin_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn enable_plugin(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let mut manager = state.plugin_manager.lock().await;
    manager.enable_plugin(&plugin_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn disable_plugin(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let mut manager = state.plugin_manager.lock().await;
    manager.disable_plugin(&plugin_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_plugin(state: State<'_, AppState>, path: String) -> Result<PluginInfo, String> {
    let mut manager = state.plugin_manager.lock().await;
    manager.install_plugin_from_path(&path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn uninstall_plugin(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    let mut manager = state.plugin_manager.lock().await;
    manager.uninstall_plugin(&plugin_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_plugin_config(state: State<'_, AppState>, plugin_id: String) -> Result<PluginConfig, String> {
    let manager = state.plugin_manager.lock().await;
    manager.get_plugin_config(&plugin_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_plugin_config(
    state: State<'_, AppState>, 
    plugin_id: String, 
    config: serde_json::Value
) -> Result<(), String> {
    let mut manager = state.plugin_manager.lock().await;
    manager.set_plugin_config(&plugin_id, config)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_plugin_config_schema(
    state: State<'_, AppState>, 
    plugin_id: String
) -> Result<Vec<PluginConfigField>, String> {
    let manager = state.plugin_manager.lock().await;
    manager.get_plugin_config_schema(&plugin_id)
        .map_err(|e| e.to_string())
}
```

### Command Registration

```rust
// In main.rs or lib.rs
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    list_plugins,
    get_plugin_info,
    enable_plugin,
    disable_plugin,
    install_plugin,
    uninstall_plugin,
    get_plugin_config,
    set_plugin_config,
    get_plugin_config_schema,
])
```

---

## Accessibility Considerations

### Keyboard Navigation
- Tab order: Header → Plugin items → Toggle switches → Action buttons
- Enter/Space activates toggles and buttons
- Escape closes expanded details

### ARIA Labels
```jsx
<div
  className={`ios-switch small ${plugin.enabled ? 'on' : ''}`}
  role="switch"
  aria-checked={plugin.enabled}
  aria-label={`Enable ${plugin.name} plugin`}
  tabIndex={0}
  onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') togglePlugin(plugin.id) }}
>
```

### Screen Reader Announcements
- Plugin state changes announced via `aria-live="polite"`
- Error states clearly communicated

### Focus Management
- Focus trapped in settings view
- Focus returned to list item after closing details
- Visible focus rings on all interactive elements

---

## Implementation Checklist

### Phase 1: Basic List View
- [ ] Add `isPluginsExpanded` state
- [ ] Add plugins section after Exit Nodes
- [ ] Create `list_plugins` Tauri command
- [ ] Render plugin list with status dots
- [ ] Add enable/disable toggles

### Phase 2: Plugin Details
- [ ] Add `expandedPluginId` state
- [ ] Implement inline details panel
- [ ] Show version, author, type, permissions
- [ ] Add Settings and Remove buttons

### Phase 3: Settings View
- [ ] Add `showPluginSettings` state
- [ ] Create full-screen settings view
- [ ] Implement dynamic form from config schema
- [ ] Add save functionality

### Phase 4: Install/Uninstall
- [ ] Add [+] button to header
- [ ] Integrate Tauri file dialog
- [ ] Handle plugin installation
- [ ] Add uninstall confirmation

### Phase 5: Polish
- [ ] Add loading states
- [ ] Add error handling
- [ ] Test accessibility
- [ ] Test dark mode
- [ ] Platform testing (macOS, Windows, Linux)

---

## Mockup: Complete Plugins View

```
┌─────────────────────────────────────────────────────┐
│ ⬡ OmniEdge                                          │
│ ● Connected                                         │
│                                      Online  [●━━]  │
├─────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────────┐ │
│ │  THIS DEVICE                      MyNetwork     │ │
│ │  10.147.17.42                                   │ │
│ └─────────────────────────────────────────────────┘ │
│                                                     │
│ VIRTUAL NETWORKS                                    │
│ ┌─────────────────────────────────────────────────┐ │
│ │ ● Production Network                        ▸   │ │
│ └─────────────────────────────────────────────────┘ │
│                                                     │
│ ▸ Exit Nodes                                        │
├─────────────────────────────────────────────────────┤
│ ▾ Plugins                               (2)    [+]  │
├─────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────────┐ │
│ │ ● Auth Provider                           [━●]  │ │
│ │   OAuth 2.0 enterprise integration          ▾   │ │
│ ├─────────────────────────────────────────────────┤ │
│ │ VERSION      1.2.0                              │ │
│ │ AUTHOR       OmniEdge Team                      │ │
│ │ TYPE         Authentication                     │ │
│ │ PERMISSIONS  [network] [config] [kv-store]      │ │
│ ├─────────────────────────────────────────────────┤ │
│ │ [    Settings    ]    [    Remove    ]          │ │
│ └─────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────┐ │
│ │ ○ Traffic Monitor                        [○━]   │ │
│ │   Real-time bandwidth statistics            ▸   │ │
│ └─────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────┤
│ [Dashboard]    │    [Debug]    │    [Quit]          │
└─────────────────────────────────────────────────────┘
```

---

## Future Enhancements

1. **Plugin Marketplace** - Browse and install from online repository
2. **Plugin Updates** - Check for and apply plugin updates
3. **Plugin Logs** - View per-plugin execution logs
4. **Plugin Widgets** - Render custom UI panels from plugins
5. **Plugin Dependencies** - Handle plugins that depend on others
6. **Plugin Categories** - Filter/group plugins by type

---

## Appendix: Plugin Type Icons

| Type | Icon | Color |
|------|------|-------|
| Event | ⚡ | Blue |
| Auth | 🔐 | Green |
| Policy | 📋 | Purple |
| DataTriage | 📊 | Orange |
| QoS | ⚡ | Yellow |
| PdM | 🔧 | Gray |
| Compliance | ✓ | Teal |

---

## References

- Current UI: `ui/desktop/src/App.tsx`
- Current Styles: `ui/desktop/src/App.css`
- Plugin API: `crates/omni-plugin/src/manager.rs`
- Plugin Types: `crates/omni-plugin/src/types.rs`
- Documentation: `docs/plugin_system_guide.md`
