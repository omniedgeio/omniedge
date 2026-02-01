//! Widget system for UI plugins
//!
//! Provides sandboxed UI extension points in the Tauri desktop application.
//! Widgets run in isolated iframes with controlled communication.

use crate::error::{PluginError, PluginResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tracing::{info, warn};

/// Widget placement in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetPlacement {
    /// Sidebar panel
    Sidebar,
    /// Main content area tab
    MainTab,
    /// Settings page section
    SettingsSection,
    /// Dashboard card
    DashboardCard,
    /// Status bar item
    StatusBar,
    /// Modal/dialog
    Modal,
    /// Floating panel
    FloatingPanel,
}

impl std::fmt::Display for WidgetPlacement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WidgetPlacement::Sidebar => write!(f, "sidebar"),
            WidgetPlacement::MainTab => write!(f, "main-tab"),
            WidgetPlacement::SettingsSection => write!(f, "settings-section"),
            WidgetPlacement::DashboardCard => write!(f, "dashboard-card"),
            WidgetPlacement::StatusBar => write!(f, "status-bar"),
            WidgetPlacement::Modal => write!(f, "modal"),
            WidgetPlacement::FloatingPanel => write!(f, "floating-panel"),
        }
    }
}

/// Widget size constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetSize {
    /// Minimum width in pixels
    #[serde(default)]
    pub min_width: Option<u32>,
    /// Maximum width in pixels
    #[serde(default)]
    pub max_width: Option<u32>,
    /// Minimum height in pixels
    #[serde(default)]
    pub min_height: Option<u32>,
    /// Maximum height in pixels
    #[serde(default)]
    pub max_height: Option<u32>,
    /// Default width
    #[serde(default)]
    pub default_width: Option<u32>,
    /// Default height
    #[serde(default)]
    pub default_height: Option<u32>,
}

impl Default for WidgetSize {
    fn default() -> Self {
        Self {
            min_width: Some(200),
            max_width: Some(800),
            min_height: Some(100),
            max_height: Some(600),
            default_width: Some(300),
            default_height: Some(200),
        }
    }
}

/// Widget manifest (embedded in plugin manifest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetManifest {
    /// Widget ID (unique within plugin)
    pub id: String,
    /// Display name
    pub name: String,
    /// Description
    pub description: String,
    /// Placement in UI
    pub placement: WidgetPlacement,
    /// Entry point HTML file
    pub entry: String,
    /// Size constraints
    #[serde(default)]
    pub size: WidgetSize,
    /// Icon (base64 or URL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Required permissions
    #[serde(default)]
    pub permissions: Vec<WidgetPermission>,
}

/// Widget permissions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetPermission {
    /// Read connection state
    ReadConnectionState,
    /// Read peer information
    ReadPeers,
    /// Read network statistics
    ReadStats,
    /// Read plugin configuration
    ReadConfig,
    /// Write plugin configuration
    WriteConfig,
    /// Show notifications
    ShowNotifications,
    /// Open external links
    OpenExternalLinks,
    /// Store data locally
    LocalStorage,
}

/// Registered widget
#[derive(Debug, Clone)]
pub struct RegisteredWidget {
    /// Full widget ID (plugin_id:widget_id)
    pub full_id: String,
    /// Plugin ID
    pub plugin_id: String,
    /// Widget manifest
    pub manifest: WidgetManifest,
    /// Path to widget bundle directory
    pub bundle_path: PathBuf,
    /// Path to entry HTML file
    pub entry_path: PathBuf,
    /// Whether widget is enabled
    pub enabled: bool,
}

impl RegisteredWidget {
    /// Create from plugin and widget manifest
    pub fn new(plugin_id: &str, manifest: WidgetManifest, bundle_path: PathBuf) -> Self {
        let full_id = format!("{}:{}", plugin_id, manifest.id);
        let entry_path = bundle_path.join(&manifest.entry);

        Self {
            full_id,
            plugin_id: plugin_id.to_string(),
            manifest,
            bundle_path,
            entry_path,
            enabled: true,
        }
    }

    /// Get the iframe sandbox attributes
    pub fn sandbox_attributes(&self) -> String {
        let mut attrs = vec![
            "allow-scripts",
            "allow-same-origin", // Needed for local file access
        ];

        if self
            .manifest
            .permissions
            .contains(&WidgetPermission::OpenExternalLinks)
        {
            attrs.push("allow-popups");
        }

        attrs.join(" ")
    }

    /// Get the Content-Security-Policy for this widget
    pub fn content_security_policy(&self) -> String {
        let mut csp = vec![
            "default-src 'self'",
            "script-src 'self' 'unsafe-inline'",
            "style-src 'self' 'unsafe-inline'",
            "img-src 'self' data: blob:",
        ];

        // Restrict connections
        csp.push("connect-src 'self'");

        csp.join("; ")
    }
}

/// Widget registry
pub struct WidgetRegistry {
    /// Registered widgets by full ID
    widgets: Arc<RwLock<HashMap<String, RegisteredWidget>>>,
    /// Widgets directory
    #[allow(dead_code)]
    widgets_dir: PathBuf,
}

impl WidgetRegistry {
    /// Create a new widget registry
    pub fn new(widgets_dir: impl Into<PathBuf>) -> Self {
        Self {
            widgets: Arc::new(RwLock::new(HashMap::new())),
            widgets_dir: widgets_dir.into(),
        }
    }

    /// Register widgets from a plugin
    pub fn register_plugin_widgets(
        &self,
        plugin_id: &str,
        widget_manifests: Vec<WidgetManifest>,
        bundle_path: &Path,
    ) -> PluginResult<()> {
        let mut widgets = self
            .widgets
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        for manifest in widget_manifests {
            let widget = RegisteredWidget::new(plugin_id, manifest, bundle_path.to_path_buf());

            // Validate entry file exists
            if !widget.entry_path.exists() {
                warn!(
                    "Widget entry file not found: {:?} for widget {}",
                    widget.entry_path, widget.full_id
                );
                continue;
            }

            info!("Registered widget: {}", widget.full_id);
            widgets.insert(widget.full_id.clone(), widget);
        }

        Ok(())
    }

    /// Unregister all widgets from a plugin
    pub fn unregister_plugin_widgets(&self, plugin_id: &str) -> PluginResult<()> {
        let mut widgets = self
            .widgets
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        widgets.retain(|_, w| w.plugin_id != plugin_id);

        info!("Unregistered widgets for plugin: {}", plugin_id);
        Ok(())
    }

    /// Get a widget by full ID
    pub fn get(&self, full_id: &str) -> Option<RegisteredWidget> {
        self.widgets.read().ok()?.get(full_id).cloned()
    }

    /// Get all widgets
    pub fn list_all(&self) -> Vec<RegisteredWidget> {
        self.widgets
            .read()
            .map(|w| w.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get widgets by placement
    pub fn get_by_placement(&self, placement: WidgetPlacement) -> Vec<RegisteredWidget> {
        self.widgets
            .read()
            .map(|w| {
                w.values()
                    .filter(|widget| widget.manifest.placement == placement && widget.enabled)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get widgets by plugin
    pub fn get_by_plugin(&self, plugin_id: &str) -> Vec<RegisteredWidget> {
        self.widgets
            .read()
            .map(|w| {
                w.values()
                    .filter(|widget| widget.plugin_id == plugin_id)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Enable a widget
    pub fn enable(&self, full_id: &str) -> PluginResult<()> {
        let mut widgets = self
            .widgets
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        match widgets.get_mut(full_id) {
            Some(widget) => {
                widget.enabled = true;
                Ok(())
            }
            None => Err(PluginError::NotFound(full_id.to_string())),
        }
    }

    /// Disable a widget
    pub fn disable(&self, full_id: &str) -> PluginResult<()> {
        let mut widgets = self
            .widgets
            .write()
            .map_err(|e| PluginError::Internal(format!("Lock poisoned: {}", e)))?;

        match widgets.get_mut(full_id) {
            Some(widget) => {
                widget.enabled = false;
                Ok(())
            }
            None => Err(PluginError::NotFound(full_id.to_string())),
        }
    }

    /// Widget count
    pub fn count(&self) -> usize {
        self.widgets.read().map(|w| w.len()).unwrap_or(0)
    }
}

/// Message types for widget communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WidgetMessage {
    /// Request from widget to host
    Request {
        id: String,
        method: String,
        params: serde_json::Value,
    },
    /// Response from host to widget
    Response {
        id: String,
        result: Option<serde_json::Value>,
        error: Option<String>,
    },
    /// Event from host to widget
    Event {
        event: String,
        data: serde_json::Value,
    },
}

/// Widget API methods that can be called from widgets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetApiMethod {
    /// Get current connection state
    GetConnectionState,
    /// Get peer list
    GetPeers,
    /// Get connection statistics
    GetStats,
    /// Get plugin configuration
    GetConfig,
    /// Set plugin configuration
    SetConfig,
    /// Show a notification
    ShowNotification,
    /// Log a message
    Log,
}

impl std::str::FromStr for WidgetApiMethod {
    type Err = PluginError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "getConnectionState" | "get_connection_state" => Ok(Self::GetConnectionState),
            "getPeers" | "get_peers" => Ok(Self::GetPeers),
            "getStats" | "get_stats" => Ok(Self::GetStats),
            "getConfig" | "get_config" => Ok(Self::GetConfig),
            "setConfig" | "set_config" => Ok(Self::SetConfig),
            "showNotification" | "show_notification" => Ok(Self::ShowNotification),
            "log" => Ok(Self::Log),
            _ => Err(PluginError::HostFunctionError(format!(
                "Unknown method: {}",
                s
            ))),
        }
    }
}

/// Widget info for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetInfo {
    pub full_id: String,
    pub plugin_id: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub placement: WidgetPlacement,
    pub enabled: bool,
    pub permissions: Vec<WidgetPermission>,
}

impl From<&RegisteredWidget> for WidgetInfo {
    fn from(widget: &RegisteredWidget) -> Self {
        Self {
            full_id: widget.full_id.clone(),
            plugin_id: widget.plugin_id.clone(),
            id: widget.manifest.id.clone(),
            name: widget.manifest.name.clone(),
            description: widget.manifest.description.clone(),
            placement: widget.manifest.placement,
            enabled: widget.enabled,
            permissions: widget.manifest.permissions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_widget_manifest() -> WidgetManifest {
        WidgetManifest {
            id: "test-widget".to_string(),
            name: "Test Widget".to_string(),
            description: "A test widget".to_string(),
            placement: WidgetPlacement::Sidebar,
            entry: "index.html".to_string(),
            size: WidgetSize::default(),
            icon: None,
            permissions: vec![WidgetPermission::ReadConnectionState],
        }
    }

    #[test]
    fn test_widget_registry() {
        let dir = tempdir().unwrap();
        let registry = WidgetRegistry::new(dir.path());

        // Create a fake bundle directory with entry file
        let bundle_path = dir.path().join("test-plugin");
        std::fs::create_dir_all(&bundle_path).unwrap();
        std::fs::write(bundle_path.join("index.html"), "<html></html>").unwrap();

        let manifest = create_test_widget_manifest();
        registry
            .register_plugin_widgets("com.test.plugin", vec![manifest], &bundle_path)
            .unwrap();

        assert_eq!(registry.count(), 1);

        let widget = registry.get("com.test.plugin:test-widget").unwrap();
        assert_eq!(widget.manifest.name, "Test Widget");
    }

    #[test]
    fn test_widget_by_placement() {
        let dir = tempdir().unwrap();
        let registry = WidgetRegistry::new(dir.path());

        let bundle_path = dir.path().join("test-plugin");
        std::fs::create_dir_all(&bundle_path).unwrap();
        std::fs::write(bundle_path.join("sidebar.html"), "<html></html>").unwrap();
        std::fs::write(bundle_path.join("dashboard.html"), "<html></html>").unwrap();

        let manifests = vec![
            WidgetManifest {
                id: "sidebar".to_string(),
                name: "Sidebar Widget".to_string(),
                description: "".to_string(),
                placement: WidgetPlacement::Sidebar,
                entry: "sidebar.html".to_string(),
                size: WidgetSize::default(),
                icon: None,
                permissions: vec![],
            },
            WidgetManifest {
                id: "dashboard".to_string(),
                name: "Dashboard Widget".to_string(),
                description: "".to_string(),
                placement: WidgetPlacement::DashboardCard,
                entry: "dashboard.html".to_string(),
                size: WidgetSize::default(),
                icon: None,
                permissions: vec![],
            },
        ];

        registry
            .register_plugin_widgets("com.test.plugin", manifests, &bundle_path)
            .unwrap();

        let sidebar_widgets = registry.get_by_placement(WidgetPlacement::Sidebar);
        assert_eq!(sidebar_widgets.len(), 1);

        let dashboard_widgets = registry.get_by_placement(WidgetPlacement::DashboardCard);
        assert_eq!(dashboard_widgets.len(), 1);
    }

    #[test]
    fn test_widget_sandbox_attributes() {
        let manifest = WidgetManifest {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            placement: WidgetPlacement::Sidebar,
            entry: "index.html".to_string(),
            size: WidgetSize::default(),
            icon: None,
            permissions: vec![WidgetPermission::OpenExternalLinks],
        };

        let widget = RegisteredWidget::new("com.test.plugin", manifest, PathBuf::from("/test"));

        let attrs = widget.sandbox_attributes();
        assert!(attrs.contains("allow-popups"));
    }
}
