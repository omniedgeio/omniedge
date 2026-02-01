import { useState, useEffect, useRef, useCallback } from 'react';
import './App.css';
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import logo from './assets/logo.png';

// Plugin types
interface PluginInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  plugin_type: string;
  enabled: boolean;
  status: 'active' | 'disabled' | 'error';
  error_message?: string;
  permissions: string[];
}

function App() {
  const [status, setStatus] = useState('disconnected');
  const [virtualIP, setVirtualIP] = useState('');
  const [networkName, setNetworkName] = useState('');
  const [connectedNetworkID, setConnectedNetworkID] = useState('');
  const [networks, setNetworks] = useState<any[]>([]);
  const [isLoggedIn, setIsLoggedIn] = useState(false);
  const [profile, setProfile] = useState<any>(null);
  const [error, setError] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [isConnecting, setIsConnecting] = useState(false);
  const [activeNetwork, setActiveNetwork] = useState<string | null>(null);
  const [expandedNetworks, setExpandedNetworks] = useState<Record<string, boolean>>({});
  const [networkDevices, setNetworkDevices] = useState<Record<string, any[]>>({});
  const [isBecomingExitNode, setIsBecomingExitNode] = useState(false);
  const [isExitNodesExpanded, setIsExitNodesExpanded] = useState(false);
  const [isWaitingForBrowser, setIsWaitingForBrowser] = useState(false);
  const [_hasPermission, setHasPermission] = useState(true);
  const [helperInstalling, setHelperInstalling] = useState(false);
  const [myDeviceID, setMyDeviceID] = useState('');
  const [myAPIIP, setMyAPIIP] = useState('');
  const [showSetup, setShowSetup] = useState(false);
  const [showDebug, setShowDebug] = useState(false);
  const [debugData, setDebugData] = useState<any>(null);
  const [helperDebugInfo, setHelperDebugInfo] = useState<{checked: boolean, active: boolean, error?: string, wrongVersion?: boolean}>({checked: false, active: false});
  const [copiedIP, setCopiedIP] = useState<string | null>(null);
  const appRef = useRef<HTMLDivElement>(null);

  // Plugin management state
  const [isPluginsExpanded, setIsPluginsExpanded] = useState(false);
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [expandedPluginId, setExpandedPluginId] = useState<string | null>(null);
  const [showPluginSettings, setShowPluginSettings] = useState(false);
  const [activePluginSettings, setActivePluginSettings] = useState<string | null>(null);
  const [pluginConfig, setPluginConfig] = useState<Record<string, any>>({});
  const [isPluginLoading, setIsPluginLoading] = useState(false);
  const [pluginToRemove, setPluginToRemove] = useState<string | null>(null);

  // Robot Data Collection state (plugin)
  const [dataCollectionAvailable, setDataCollectionAvailable] = useState(false);
  const [dataCollectionEnabled, setDataCollectionEnabled] = useState(false);

  // Resize window to fit content
  // Track last height to avoid unnecessary resize calls
  const lastHeightRef = useRef<number>(0);

  const resizeToContent = useCallback(async () => {
    if (appRef.current) {
      // Use offsetHeight to get the actual rendered height of content
      const contentHeight = appRef.current.offsetHeight;
      
      // Skip if height hasn't changed significantly (within 5px tolerance)
      if (Math.abs(contentHeight - lastHeightRef.current) < 5) {
        return;
      }
      lastHeightRef.current = contentHeight;
      
      // Call Rust command to resize window (handles both native window and webview)
      try {
        await invoke('resize_window', { height: contentHeight });
      } catch (e) {
        console.error('Failed to resize window:', e);
      }
    }
  }, []);

  // Use MutationObserver to detect DOM changes (more reliable than ResizeObserver for content changes)
  useEffect(() => {
    if (!appRef.current) return;

    let resizeTimeout: ReturnType<typeof setTimeout>;

    const triggerResize = () => {
      clearTimeout(resizeTimeout);
      resizeTimeout = setTimeout(resizeToContent, 100); // Increased debounce to 100ms
    };

    // ResizeObserver for size changes
    const resizeObserver = new ResizeObserver(triggerResize);
    resizeObserver.observe(appRef.current);

    // MutationObserver for DOM structure changes (expand/collapse adds/removes elements)
    const mutationObserver = new MutationObserver(triggerResize);
    mutationObserver.observe(appRef.current, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'style']
    });

    // Initial resize
    resizeToContent();

    return () => {
      resizeObserver.disconnect();
      mutationObserver.disconnect();
      clearTimeout(resizeTimeout);
    };
  }, [resizeToContent]);

  // Trigger resize on key state changes (backup for state-driven changes)
  useEffect(() => {
    const timer = setTimeout(resizeToContent, 50);
    return () => clearTimeout(timer);
  }, [isLoggedIn, networks, expandedNetworks, isLoading, isConnecting, resizeToContent, isWaitingForBrowser, isExitNodesExpanded, isBecomingExitNode, error, networkDevices, status, virtualIP, showDebug, showSetup, isPluginsExpanded, expandedPluginId, showPluginSettings, plugins, dataCollectionEnabled]);

  useEffect(() => {
    const init = async () => {
      try {
        // Check helper status with debug info
        let helperActive = false;
        let helperError: string | undefined;
        let wrongVersion = false;
        
        try {
          helperActive = await invoke('check_helper') as boolean;
          
          // If check_helper returns false, try to get version to see if it's an old helper
          if (!helperActive) {
            try {
              // Try to get version - if this fails with wrong format, it's the old helper
              await invoke('get_helper_version');
            } catch {
              // Old helper detected - ping works but version doesn't
              wrongVersion = true;
            }
          }
        } catch (err: any) {
          helperError = err.toString();
        }
        
        const elevated = await invoke('check_is_admin') as boolean;
        
        // Update helper debug info
        setHelperDebugInfo({
          checked: true,
          active: helperActive,
          error: helperError,
          wrongVersion: wrongVersion,
        });

        const canConnect = helperActive || elevated;
        setHasPermission(canConnect);

        // Always show setup if helper is not running or wrong version
        if (!helperActive) {
          setShowSetup(true);
        }

        const autoLoginSuccess = await invoke('try_auto_login');
        if (autoLoginSuccess) {
          await handleSuccessfulLogin();
        }
      } catch (err) {
        console.error("Initialization failed", err);
      } finally {
        setIsLoading(false);
      }
    };
    init();

    const statusInterval = setInterval(async () => {
      try {
        const currStatus = await invoke('get_state') as string;
        setStatus(currStatus.toLowerCase());
        await refreshConnectionInfo();
      } catch (e) { }
    }, 3000);

    return () => clearInterval(statusInterval);
  }, []); // Run transition only on mount

  const handleSuccessfulLogin = async () => {
    setIsLoading(true);
    try {
      const userProfile: any = await invoke('get_profile');
      setProfile(userProfile);

      const hwid: string = await invoke('get_device_id');
      setMyDeviceID(hwid);

      const nets: any = await invoke('list_networks');
      const netsArray = nets || [];
      setNetworks(netsArray);

      const devs: any[] = await invoke('list_devices');
      const me = devs.find(d => d.hardware_id === hwid);
      if (me) setMyAPIIP(me.virtual_ip);

      setIsLoggedIn(true);
      setIsWaitingForBrowser(false);

      // Only auto-connect if helper is available
      const helperActive = await invoke('check_helper') as boolean;
      if (helperActive) {
        const currState = await invoke('get_state') as string;
        if (currState.toLowerCase() === 'disconnected' && netsArray.length > 0) {
          handleConnect(netsArray[0].id);
        }
      }
    } catch (err: any) {
      console.error("handleSuccessfulLogin failed:", err);
      setError(`Failed to load profile/network: ${err.message || err.toString()}`);
      setIsLoggedIn(false);
    } finally {
      setIsLoading(false);
      setIsWaitingForBrowser(false);
    }
  };

  const refreshConnectionInfo = async () => {
    try {
      const vIP: string = await invoke('get_virtual_ip');
      if (vIP) setVirtualIP(vIP);

      const isExit: boolean = await invoke('is_exit_node');
      setIsBecomingExitNode(isExit);

      if (isLoggedIn) {
        const devs: any[] = await invoke('list_devices');
        const me = devs.find(d => d.hardware_id === myDeviceID);
        if (me && me.virtual_ip) setMyAPIIP(me.virtual_ip);
      }

      const currStatus = (await invoke('get_state') as string).toLowerCase();
      if (currStatus === 'connected' && networks.length > 0) {
        // Try to match current IP to a network range if not already set
        const active = networks.find(n => vIP && vIP.startsWith(n.ip_range?.split('/')[0].split('.').slice(0, 2).join('.')));
        if (active) {
          setNetworkName(active.name);
          setConnectedNetworkID(active.id);
          setActiveNetwork(active.id);
        }
      } else if (currStatus === 'disconnected' || currStatus === 'authenticated') {
        setConnectedNetworkID('');
        if (!isConnecting) {
          setActiveNetwork(null);
        }
      }
    } catch (e) { }
  };

  const handleBrowserLogin = async () => {
    setIsLoading(true);
    setError('');
    try {
      const resp: any = await invoke('start_session_login');
      setIsWaitingForBrowser(true);
      
      // Start waiting for WebSocket token BEFORE opening browser
      // This ensures we don't miss the token if browser login is very fast
      const waitPromise = invoke('wait_for_session_login', { sessionId: resp.id });
      
      // Small delay to ensure WebSocket is connected before browser opens
      await new Promise(resolve => setTimeout(resolve, 500));
      
      // Now open browser
      await invoke('open_browser', { url: resp.auth_url });
      
      // Wait for the token
      const auth: any = await waitPromise;
      if (auth) {
        console.log("Session login successful");
        await handleSuccessfulLogin();
      }
    } catch (err: any) {
      console.error("Session login failed:", err);
      setError(err.message || err.toString());
      setIsWaitingForBrowser(false);
    } finally {
      setIsLoading(false);
      setIsWaitingForBrowser(false);
    }
  };

  const handleCancelBrowserLogin = async () => {
    try {
      await invoke('cancel_session_login');
    } catch (e) {
      console.error('Failed to cancel session login:', e);
    }
    setIsWaitingForBrowser(false);
    setIsLoading(false);
    setError("");
  };

  const handleLogout = async () => {
    try {
      await invoke('logout');
    } catch (e) {
      console.error('Logout failed:', e);
    }
    setIsLoggedIn(false);
    setProfile(null);
    setNetworks([]);
    setActiveNetwork(null);
    setConnectedNetworkID('');
    setNetworkName('');
    setError('');
  };

  const handleConnect = async (networkId: string) => {
    // Prevent multiple simultaneous connection attempts
    if (isConnecting) {
      console.log("Already connecting, ignoring duplicate connect request");
      return;
    }
    
    // First check if helper is installed
    const helperActive = await invoke('check_helper') as boolean;
    if (!helperActive) {
      // Helper not running - prompt user to install
      setHasPermission(false);
      setShowSetup(true);
      setError("Background service is required to connect. Please install the helper service.");
      return;
    }
    
    setIsConnecting(true);
    setActiveNetwork(networkId); // Set immediately to prevent duplicate clicks
    setError('');
    try {
      await invoke('connect', { networkId, as_exit_node: isBecomingExitNode });
      
      // Verify connection actually succeeded by checking state
      const currStatus = await invoke('get_state') as string;
      if (currStatus.toLowerCase() === 'connected') {
        setConnectedNetworkID(networkId);
        const net = networks.find(n => n.id === networkId);
        if (net) setNetworkName(net.name);
        await refreshConnectionInfo();
      } else {
        // Connection didn't actually succeed
        setError("Connection attempt did not succeed. Check debug info for details.");
        setActiveNetwork(null);
        setConnectedNetworkID('');
      }
    } catch (err: any) {
      console.error(`Connection failed:`, err);
      setError(err.message || err.toString());
      // On failure, clear active network so the UI doesn't look connected
      setActiveNetwork(null);
      setConnectedNetworkID('');
    } finally {
      setIsConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    setIsConnecting(true);
    setError('');
    try {
      await invoke('disconnect');
      await refreshConnectionInfo();
      setVirtualIP('');
      setActiveNetwork(null);
      setConnectedNetworkID('');
    } catch (err) {
      console.error("Disconnect failed:", err);
    } finally {
      setIsConnecting(false);
    }
  };

  const toggleNetworkExpand = async (networkId: string) => {
    if (!networkId) return;
    const isExpanded = !!expandedNetworks[networkId];
    setExpandedNetworks({ ...expandedNetworks, [networkId]: !isExpanded });
    if (!isExpanded && isLoggedIn) {
      try {
        const devs: any = await invoke('get_network_devices', { networkId });
        setNetworkDevices(prev => ({ ...prev, [networkId]: devs || [] }));
      } catch (err) {
        console.error(`Failed to fetch devices:`, err);
      }
    }
  };

  const handleToggleIsExitNode = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const newValue = !isBecomingExitNode;
    setIsBecomingExitNode(newValue);
    try {
      await invoke('set_as_exit_node', { enabled: newValue });
    } catch (err) {
      console.error("Failed to toggle exit node status", err);
      // Revert on failure
      setIsBecomingExitNode(!newValue);
    }
  };

  const handleSelectExitNode = async (exitNodeId: string) => {
    if (!activeNetwork) return;
    try {
      setIsConnecting(true);
      const devices = networkDevices[activeNetwork] || [];
      const selectedDevice = devices.find(d => d.id === exitNodeId);
      const exitNodeIp = selectedDevice ? selectedDevice.virtual_ip : '';

      await invoke('set_exit_node', { networkId: activeNetwork, exitNodeId, exitNodeIp });
      const nets: any = await invoke('list_networks');
      setNetworks(nets || []);
    } catch (err) {
      setError("Failed to set exit node.");
    } finally {
      setIsConnecting(false);
    }
  };

  const refreshDevices = async (networkId: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    if (!isLoggedIn) return;
    try {
      const devs: any = await invoke('get_network_devices', { networkId });
      setNetworkDevices(prev => ({ ...prev, [networkId]: devs || [] }));
    } catch (err) {
      console.error(`Failed to refresh devices:`, err);
    }
  };

  const openURL = async (url: string) => {
    await invoke('open_browser', { url });
  };

  const handleCopyIP = (ip: string) => {
    if (!ip) return;
    navigator.clipboard.writeText(ip);
    setCopiedIP(ip);
    setTimeout(() => setCopiedIP(null), 2000);
  };

  // Plugin management functions
  const loadPlugins = async () => {
    try {
      // Use refresh_plugins to re-discover plugins from disk
      const pluginList: PluginInfo[] = await invoke('refresh_plugins');
      setPlugins(pluginList);
    } catch (err) {
      console.error('Failed to load plugins:', err);
    }
  };

  const handleTogglePlugin = async (pluginId: string, enabled: boolean) => {
    try {
      if (enabled) {
        await invoke('disable_plugin', { pluginId });
      } else {
        await invoke('enable_plugin', { pluginId });
      }
      await loadPlugins();
    } catch (err: any) {
      console.error('Failed to toggle plugin:', err);
      setError(`Failed to toggle plugin: ${err.message || err.toString()}`);
    }
  };

  const handlePluginSettings = async (pluginId: string) => {
    try {
      const config: any = await invoke('get_plugin_config', { pluginId });
      setPluginConfig(config || {});
      setActivePluginSettings(pluginId);
      setShowPluginSettings(true);
    } catch (err) {
      console.error('Failed to load plugin config:', err);
    }
  };

  const handleSavePluginConfig = async () => {
    if (!activePluginSettings) return;
    setIsPluginLoading(true);
    try {
      await invoke('set_plugin_config', { 
        pluginId: activePluginSettings, 
        config: pluginConfig 
      });
      setShowPluginSettings(false);
      setActivePluginSettings(null);
      await loadPlugins();
    } catch (err: any) {
      setError(`Failed to save config: ${err.message || err.toString()}`);
    } finally {
      setIsPluginLoading(false);
    }
  };

  const handleRemovePlugin = async (pluginId: string) => {
    setIsPluginLoading(true);
    try {
      await invoke('uninstall_plugin', { pluginId });
      setPluginToRemove(null);
      setExpandedPluginId(null);
      await loadPlugins();
    } catch (err: any) {
      setError(`Failed to remove plugin: ${err.message || err.toString()}`);
    } finally {
      setIsPluginLoading(false);
    }
  };

  const handleInstallPlugin = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: 'Plugin',
          extensions: ['zip', 'wasm']
        }],
        title: 'Select Plugin File'
      });
      
      if (selected) {
        setIsPluginLoading(true);
        setError('');
        await invoke('install_plugin_from_file', { path: selected });
        await loadPlugins();
        setIsPluginLoading(false);
      }
    } catch (e: any) {
      setError(`Failed to install plugin: ${e}`);
      setIsPluginLoading(false);
    }
  };

  // Load plugins when logged in and plugins section is expanded
  useEffect(() => {
    if (isLoggedIn && isPluginsExpanded && plugins.length === 0) {
      loadPlugins();
    }
  }, [isLoggedIn, isPluginsExpanded]);

  // Check data collection availability when logged in
  useEffect(() => {
    if (isLoggedIn) {
      checkDataCollectionAvailable();
    }
  }, [isLoggedIn]);

  // Robot Data Collection functions
  const checkDataCollectionAvailable = async () => {
    try {
      const available = await invoke('is_data_collection_available') as boolean;
      setDataCollectionAvailable(available);
      return available;
    } catch {
      setDataCollectionAvailable(false);
      return false;
    }
  };

  const handleInstallHelper = async () => {
    setHelperInstalling(true);
    setError('');
    try {
      await invoke('install_helper');
      
      // Wait a moment for the service to fully start
      await new Promise(resolve => setTimeout(resolve, 2000));
      
      // Re-check helper status after installation - retry a few times
      let helperActive = false;
      for (let i = 0; i < 5; i++) {
        helperActive = await invoke('check_helper') as boolean;
        if (helperActive) break;
        await new Promise(resolve => setTimeout(resolve, 1000));
      }
      
      // Update debug info
      setHelperDebugInfo({
        checked: true,
        active: helperActive,
        wrongVersion: false,
        error: helperActive ? undefined : 'Helper installed but not responding with correct version',
      });
      
      if (helperActive) {
        setHasPermission(true);
        setError('');
        setShowSetup(false);
        
        // Try auto-login if we have saved credentials
        try {
          const autoLoginSuccess = await invoke('try_auto_login');
          if (autoLoginSuccess) {
            await handleSuccessfulLogin();
          }
        } catch (e) {
          console.log('Auto-login after helper install failed:', e);
        }
      } else {
        setError('Helper was installed but is not responding correctly. Please try "Check Again" or view Debug info.');
      }
    } catch (err: any) {
      setError(`Failed to install helper: ${err.toString()}`);
      setHelperDebugInfo(prev => ({...prev, error: err.toString()}));
    } finally {
      setHelperInstalling(false);
    }
  };

  const handleQuit = async () => {
    await invoke('quit');
  };

  const fetchDebugInfo = async () => {
    try {
      const data = await invoke('get_debug_info');
      setDebugData(data);
    } catch (e) {
      console.error("Failed to fetch debug info", e);
    }
  };

  const handleOpenDebug = () => {
    fetchDebugInfo();
    setShowDebug(true);
  };

  const getStatusColor = () => {
    if (status === 'connected') return '#34c759';
    if (status === 'connecting') return '#ffcc00';
    return '#8e8e93';
  };

  if (showDebug) {
    return (
      <div className="app" ref={appRef}>
        <div className="app-header">
          <div className="header-left">
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <img src={logo} className="logo-img" alt="OmniEdge" />
              <span className="app-name">Debug Info</span>
            </div>
          </div>
          <div className="header-right">
            <button className="secondary-btn mini" onClick={() => setShowDebug(false)}>Back</button>
          </div>
        </div>
        <div className="main-content-scroll">
          <div className="main-content-inner debug-view">
            <div className="debug-section">
              <div className="label-tiny">HELPER STATUS</div>
              <div className="debug-card">
                <div className="debug-line">
                  <span className="debug-label">Active:</span>
                  <span className={`debug-value ${debugData?.helper_active ? 'success' : 'error'}`}>
                    {debugData?.helper_active ? 'YES' : 'NO'}
                  </span>
                </div>
                {debugData?.helper_state && (
                  <>
                    <div className="debug-line">
                      <span className="debug-label">State:</span>
                      <span className="debug-value">{debugData.helper_state.state}</span>
                    </div>
                    <div className="debug-line">
                      <span className="debug-label">VIP:</span>
                      <span className="debug-value mono">{debugData.helper_state.virtual_ip || 'None'}</span>
                    </div>
                  </>
                )}
                {debugData?.helper_message && (
                  <div className="debug-line-error">
                    {debugData.helper_message}
                  </div>
                )}
              </div>
            </div>

            <div className="debug-section">
              <div className="label-tiny">SYSTEM LOGS (LAST 50 LINES)</div>
              <div className="log-container">
                <pre className="log-text">
                  {debugData?.helper_logs || 'No logs available.'}
                </pre>
              </div>
              {debugData?.log_file && <div className="log-file-path truncate">{debugData.log_file}</div>}
            </div>

            <div className="debug-actions">
              <button className="primary-login-btn" style={{ width: '100%' }} onClick={fetchDebugInfo}>Refresh</button>
              <button className="secondary-btn" style={{ width: '100%', marginTop: '8px' }} onClick={() => invoke('open_logs')}>Open Log Folder</button>
            </div>
          </div>
        </div>
      </div>
    );
  }

  // Plugin Settings View (full screen)
  if (showPluginSettings && activePluginSettings) {
    const plugin = plugins.find(p => p.id === activePluginSettings);
    return (
      <div className="app" ref={appRef}>
        <div className="app-header">
          <div className="header-left">
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <img src={logo} className="logo-img" alt="OmniEdge" />
              <span className="app-name">Plugin Settings</span>
            </div>
          </div>
          <div className="header-right">
            <button className="secondary-btn mini" onClick={() => { setShowPluginSettings(false); setActivePluginSettings(null); }}>Back</button>
          </div>
        </div>
        <div className="main-content-scroll">
          <div className="main-content-inner plugin-settings-view">
            {plugin && (
              <>
                <div className="plugin-settings-content">
                  <div className="plugin-config-section">
                    <div className="plugin-config-label">{plugin.name} Configuration</div>
                    
                    {/* Dynamic config fields based on stored config */}
                    {Object.keys(pluginConfig).length > 0 ? (
                      Object.entries(pluginConfig).map(([key, value]) => (
                        <div key={key} className="plugin-config-field">
                          <label>{key.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase())}</label>
                          {typeof value === 'boolean' ? (
                            <div className="plugin-config-toggle-row">
                              <span className="plugin-config-toggle-label">{key}</span>
                              <div
                                className={`ios-switch small ${value ? 'on' : ''}`}
                                onClick={() => setPluginConfig(prev => ({ ...prev, [key]: !value }))}
                                role="switch"
                                aria-checked={value}
                                tabIndex={0}
                              >
                                <div className="dot"></div>
                              </div>
                            </div>
                          ) : (
                            <input
                              type={typeof value === 'number' ? 'number' : 'text'}
                              className="plugin-config-input"
                              value={value as string}
                              onChange={(e) => setPluginConfig(prev => ({ 
                                ...prev, 
                                [key]: typeof value === 'number' ? Number(e.target.value) : e.target.value 
                              }))}
                              placeholder={`Enter ${key}`}
                            />
                          )}
                        </div>
                      ))
                    ) : (
                      <div className="plugins-empty">
                        <div className="plugins-empty-text">No configuration options</div>
                        <div className="plugins-empty-hint">This plugin has no configurable settings</div>
                      </div>
                    )}
                  </div>
                  
                  {Object.keys(pluginConfig).length > 0 && (
                    <button 
                      className="plugin-save-btn" 
                      onClick={handleSavePluginConfig}
                      disabled={isPluginLoading}
                    >
                      {isPluginLoading ? 'Saving...' : 'Save Configuration'}
                    </button>
                  )}
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="app" ref={appRef}>
      <div className="app-header">
        <div className="header-left">
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <img src={logo} className="logo-img" alt="OmniEdge" />
            <span className="app-name">OmniEdge</span>
          </div>
          <div className="status-indicator-row">
            <div className="pulse-dot" style={{ backgroundColor: getStatusColor() }}></div>
            <span className="status-text">
              {status === 'connected' ? 'Connected' : (status === 'connecting' ? 'Connecting...' : 'Disconnected')}
            </span>
          </div>
        </div>
        <div className="header-right">
          <div className="login-status-container" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{ fontSize: '11px', opacity: 0.6, fontWeight: 500 }}>
              {isLoggedIn ? 'Online' : 'Sign In'}
            </span>
            <div
              className={`ios-switch header-toggle ${isLoggedIn || isWaitingForBrowser ? 'on' : ''} ${isLoading ? 'disabled' : ''}`}
              onClick={isLoading ? undefined : (isLoggedIn ? handleLogout : (isWaitingForBrowser ? handleCancelBrowserLogin : handleBrowserLogin))}
              onKeyDown={(e) => { if (!isLoading && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); (isLoggedIn ? handleLogout : (isWaitingForBrowser ? handleCancelBrowserLogin : handleBrowserLogin))(); } }}
              tabIndex={isLoading ? -1 : 0}
              role="switch"
              aria-checked={isLoggedIn || isWaitingForBrowser}
              aria-label={isLoggedIn ? 'Sign out' : 'Sign in'}
              aria-disabled={isLoading}
            >
              <div className="dot">
                {(isWaitingForBrowser || isLoading) && <div className="loader-mini" style={{ borderTopColor: 'var(--accent-blue)' }}></div>}
              </div>
            </div>
          </div>
        </div>
      </div>

      <div className="main-content-scroll">
        <div className="main-content-inner">
          {isWaitingForBrowser && (
            <div className="status-banner">
              <span className="banner-text">Verify identity in browser...</span>
              <span className="banner-cancel" onClick={handleCancelBrowserLogin}>Cancel</span>
            </div>
          )}

          {error && (
            <div className="error-banner">
              <span className="error-text-content">{error}</span>
              <div className="error-actions">
                <span className="error-dismiss" onClick={() => setError('')}>Dismiss</span>
                <span className="install-badge clickable" onClick={handleOpenDebug}>Debug</span>
              </div>
            </div>
          )}

          {showSetup ? (
            <div className="setup-view">
              <div className="setup-hero">
                <div className="setup-icon">
                  <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path>
                    <path d="M12 8v4"></path>
                    <path d="M12 16h.01"></path>
                  </svg>
                </div>
                <h2>{helperDebugInfo.wrongVersion ? 'Helper Update Required' : 'Background Service Required'}</h2>
                <p>
                  {helperDebugInfo.wrongVersion 
                    ? 'An older version of the helper service was detected. Please install the updated helper to ensure compatibility.'
                    : 'To provide secure, non-admin VPN connectivity and background operations, OmniEdge needs to install its helper service.'}
                </p>

                {helperDebugInfo.wrongVersion && (
                  <div className="warning-banner">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
                      <line x1="12" y1="9" x2="12" y2="13"></line>
                      <line x1="12" y1="17" x2="12.01" y2="17"></line>
                    </svg>
                    <span>Old helper version detected - connections will not work until updated</span>
                  </div>
                )}

                <div className="setup-benefits">
                  <div className="benefit-item">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--status-green)" strokeWidth="2.5">
                      <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                    <span>No Administrator prompt on every start</span>
                  </div>
                  <div className="benefit-item">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--status-green)" strokeWidth="2.5">
                      <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                    <span>Seamless background connectivity</span>
                  </div>
                </div>

                <button
                  className="primary-login-btn setup-btn"
                  onClick={handleInstallHelper}
                  disabled={helperInstalling}
                >
                  {helperInstalling ? (
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <div className="loader-mini" style={{ borderTopColor: 'white' }}></div>
                      Installing...
                    </div>
                  ) : "Enable Background Service"}
                </button>
                <button
                  className="secondary-btn"
                  style={{ width: '100%', marginTop: '8px' }}
                  onClick={async () => {
                    setHelperDebugInfo(prev => ({...prev, checked: false}));
                    try {
                      const helperActive = await invoke('check_helper') as boolean;
                      setHelperDebugInfo({checked: true, active: helperActive, error: undefined});
                      if (helperActive) {
                        setHasPermission(true);
                        setShowSetup(false);
                        setError('');
                      }
                    } catch (err: any) {
                      setHelperDebugInfo({checked: true, active: false, error: err.toString()});
                    }
                  }}
                >
                  Check Again
                </button>
                <div className="setup-hint" style={{ marginTop: '12px' }}>Requires a one-time Administrator elevation</div>
                
                {/* Debug Information Section */}
                <div className="setup-debug-section">
                  <div className="setup-debug-header" onClick={() => setShowDebug(!showDebug)}>
                    <span>Debug Information</span>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" style={{ transform: showDebug ? 'rotate(90deg)' : 'none', transition: '0.2s' }}>
                      <polyline points="9 18 15 12 9 6"></polyline>
                    </svg>
                  </div>
                  {showDebug && (
                    <div className="setup-debug-content">
                      <div className="debug-row">
                        <span className="debug-label">Helper Status:</span>
                        <span className={`debug-value ${helperDebugInfo.active ? 'success' : 'error'}`}>
                          {!helperDebugInfo.checked ? 'Checking...' : (helperDebugInfo.active ? 'Running (v2)' : (helperDebugInfo.wrongVersion ? 'Wrong Version' : 'Not Running'))}
                        </span>
                      </div>
                      {helperDebugInfo.wrongVersion && (
                        <div className="debug-row">
                          <span className="debug-label">Issue:</span>
                          <span className="debug-value error">Old Go helper detected, needs Rust v2 helper</span>
                        </div>
                      )}
                      <div className="debug-row">
                        <span className="debug-label">Socket Path:</span>
                        <span className="debug-value mono">/var/run/omniedge-helper.sock</span>
                      </div>
                      <div className="debug-row">
                        <span className="debug-label">Helper Binary:</span>
                        <span className="debug-value mono">/Library/PrivilegedHelperTools/io.omniedge.helper</span>
                      </div>
                      <div className="debug-row">
                        <span className="debug-label">LaunchDaemon:</span>
                        <span className="debug-value mono">/Library/LaunchDaemons/io.omniedge.helper.plist</span>
                      </div>
                      {helperDebugInfo.error && (
                        <div className="debug-row error">
                          <span className="debug-label">Error:</span>
                          <span className="debug-value">{helperDebugInfo.error}</span>
                        </div>
                      )}
                      <div className="debug-actions-row">
                        <button className="mini-btn" onClick={() => invoke('open_logs')}>View Logs</button>
                        <button className="mini-btn" onClick={async () => {
                          const data = await invoke('get_debug_info');
                          setDebugData(data);
                          console.log('Debug data:', data);
                          alert(JSON.stringify(data, null, 2));
                        }}>Full Debug</button>
                      </div>
                    </div>
                  )}
                </div>
              </div>
            </div>
          ) : !isLoggedIn ? (
            <div className="logged-out-view">
              <div className="placeholder-hero">
                <div className="hero-gradient"></div>
                <div className="hero-icon-container">
                  <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="var(--accent-blue)" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" style={{ opacity: 0.8 }}>
                    <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"></path>
                  </svg>
                </div>
                <p>Private mesh networking for AI, IoT, and edge devices</p>
                <button className="primary-login-btn" onClick={handleBrowserLogin} disabled={isLoading || isWaitingForBrowser}>Sign In to Start</button>
              </div>
              <div className="locked-info">
                <div className="divider"></div>
                <div className="detail-section disabled">
                  <div className="detail-line aligned-row">
                    <span className="detail-label">Virtual IP</span>
                    <span className="detail-value mono">---.---.---.---</span>
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="dashboard-view">
              <div className="this-device-card">
                <div className="card-bg-glow"></div>
                <div className="card-content">
                  <div className="card-top-row">
                    <span className="card-label">This Device</span>
                    {status === 'connected' && <span className="network-badge">{networkName}</span>}
                  </div>
                  <div className="ip-display-large clickable-ip" onClick={() => handleCopyIP(virtualIP || myAPIIP)}>
                    {virtualIP || myAPIIP || '0.0.0.0'}
                    <div className={`copy-hint ${copiedIP === (virtualIP || myAPIIP) ? 'copied' : ''}`}>
                      {copiedIP === (virtualIP || myAPIIP) ? 'Copied!' : 'Click to copy'}
                    </div>
                  </div>
                </div>
              </div>

              <div className="profile-bar">
                <div className="profile-chip-circle">
                  <span className="profile-initial">{profile?.email?.[0]?.toUpperCase() || 'U'}</span>
                </div>
                <span className="profile-email truncate">{profile?.email}</span>
                <div className="user-online-dot"></div>
              </div>

              <div className="section-header">
                <span>Virtual Networks</span>
              </div>

              <div className="networks-list">
                {networks.map(net => {
                  const isExpanded = expandedNetworks[net.id];
                  // Only show as active if status is 'connected' AND this is the connected network
                  const isActive = status === 'connected' && (connectedNetworkID === net.id);
                  // Show as connecting if we're actively trying to connect to this network
                  const isThisConnecting = isConnecting && activeNetwork === net.id;

                  return (
                    <div key={net.id} className={`network-item-wrapper ${isExpanded ? 'is-expanded' : ''} ${isActive ? 'is-active' : ''} ${isThisConnecting ? 'is-connecting' : ''}`}>
                      <div className="network-menu-item" onClick={() => toggleNetworkExpand(net.id)}>
                        <div className="item-left">
                          <div className={`status-orb ${isActive ? 'active' : ''} ${isThisConnecting ? 'connecting' : ''}`}></div>
                          <span className="network-name-text truncate">{net.name}</span>
                        </div>
                        <div className="item-right">
                          <div className="chevron-icon" style={{ transform: isExpanded ? 'rotate(90deg)' : 'none' }}>
                            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                              <polyline points="9 18 15 12 9 6"></polyline>
                            </svg>
                          </div>
                        </div>
                      </div>

                      {isExpanded && (
                        <div className="network-expand-content">
                          <div className="control-row">
                            <span className="control-label">VPN Connection</span>
                            <div
                              className={`ios-switch small ${isActive ? 'on' : ''} ${isThisConnecting ? 'connecting' : ''}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                if (isThisConnecting) return; // Don't allow toggle while connecting
                                isActive ? handleDisconnect() : handleConnect(net.id);
                              }}
                              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); if (!isThisConnecting) { isActive ? handleDisconnect() : handleConnect(net.id); } } }}
                              tabIndex={0}
                              role="switch"
                              aria-checked={isActive}
                              aria-label={`VPN connection for ${net.name}`}
                            >
                              <div className="dot">
                                {isThisConnecting && <div className="loader-mini" style={{ borderTopColor: 'var(--accent-blue)' }}></div>}
                              </div>
                            </div>
                          </div>

                          <div className="device-section">
                            <div className="device-section-header">
                              <span>Devices</span>
                              <div className="refresh-btn" onClick={(e) => refreshDevices(net.id, e)}>
                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                                  <path d="M23 4v6h-6"></path>
                                  <path d="M1 20v-6h6"></path>
                                  <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
                                </svg>
                              </div>
                            </div>
                            <div className="device-list">
                              {(networkDevices[net.id] || []).map(dev => (
                                <div key={dev.id} className="device-row">
                                  <div className="dev-info">
                                    <div className={`online-dot ${dev.online ? 'active' : ''}`}></div>
                                    <span className="dev-name truncate">{dev.name}</span>
                                  </div>
                                  <span
                                    className={`dev-ip mono clickable-ip ${copiedIP === dev.virtual_ip ? 'copied' : ''}`}
                                    onClick={() => handleCopyIP(dev.virtual_ip)}
                                  >
                                    {copiedIP === dev.virtual_ip ? 'Copied!' : (dev.virtual_ip || '---.---.---.---')}
                                  </span>
                                </div>
                              ))}
                              {networkDevices[net.id] && networkDevices[net.id].length === 0 && (
                                <div className="empty-devices">No other devices found</div>
                              )}
                              {!networkDevices[net.id] && (
                                <div className="loading-devices">Loading members...</div>
                              )}
                            </div>
                          </div>
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>

              <div className="collapsible-header" onClick={() => setIsExitNodesExpanded(!isExitNodesExpanded)}>
                <span className="section-title">Exit Nodes</span>
                <div className="chevron-icon" style={{ transform: isExitNodesExpanded ? 'rotate(90deg)' : 'none' }}>
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="9 18 15 12 9 6"></polyline>
                  </svg>
                </div>
              </div>

              {isExitNodesExpanded && (
                <div className="exit-node-pane">
                  <div className="toggle-row">
                    <span className="toggle-info">Run this device as Exit Node</span>
                    <div
                      className={`ios-switch mini ${isBecomingExitNode ? 'on' : ''}`}
                      onClick={handleToggleIsExitNode}
                      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleToggleIsExitNode(e as any); } }}
                      tabIndex={0}
                      role="switch"
                      aria-checked={isBecomingExitNode}
                      aria-label="Run this device as exit node"
                    >
                      <div className="dot"></div>
                    </div>
                  </div>

                  {activeNetwork && (
                    <div className="exit-node-selection-area">
                      <div
                        className={`exit-option-item ${!networks.find(n => n.id === activeNetwork)?.selected_exit_node_id ? 'is-selected' : ''}`}
                        onClick={() => handleSelectExitNode('')}
                      >
                        <div className="radio-circle">
                          {!networks.find(n => n.id === activeNetwork)?.selected_exit_node_id && <div className="radio-inner"></div>}
                        </div>
                        <span>Direct Connection (No Exit Node)</span>
                      </div>

                      <div className="label-tiny">AVAILABLE EXIT NODES</div>
                      <div className="exit-node-scroller">
                        {(networkDevices[activeNetwork] || [])
                          .filter(dev => dev.is_exit_node && dev.exit_node_enabled && dev.id !== profile?.id)
                          .map(dev => {
                            const isSelected = networks.find(n => n.id === activeNetwork)?.selected_exit_node_id === dev.id;
                            return (
                              <div
                                key={dev.id}
                                className={`exit-option-item ${isSelected ? 'is-selected' : ''} ${!dev.online ? 'is-offline' : ''}`}
                                onClick={dev.online ? () => handleSelectExitNode(dev.id) : undefined}
                              >
                                <div className="radio-circle">
                                  {isSelected && <div className="radio-inner"></div>}
                                </div>
                                <div className="exit-item-text">
                                  <span className="exit-dev-name truncate">{dev.name}</span>
                                  <span className="exit-dev-ip">{dev.virtual_ip}</span>
                                </div>
                                {!dev.online && <span className="offline-pill">Offline</span>}
                              </div>
                            );
                          })}
                        {!(networkDevices[activeNetwork] || []).some(dev => dev.is_exit_node && dev.exit_node_enabled) && (
                          <div className="empty-exit-nodes">No devices configured as exit nodes</div>
                        )}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

              {/* Plugins Section - Show only when logged in */}
              {isLoggedIn && !showSetup && (
                <>
                  <div className="plugins-section-header" onClick={() => setIsPluginsExpanded(!isPluginsExpanded)}>
                <div className="plugins-header-left">
                  <div className="chevron-icon" style={{ transform: isPluginsExpanded ? 'rotate(90deg)' : 'none' }}>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="9 18 15 12 9 6"></polyline>
                    </svg>
                  </div>
                  <span className="section-title">Plugins</span>
                  {(plugins.length > 0 || dataCollectionAvailable) && (
                    <span className={`plugin-count-badge ${(plugins.filter(p => p.enabled).length + (dataCollectionEnabled ? 1 : 0)) === 0 ? 'inactive' : ''}`}>
                      {plugins.filter(p => p.enabled).length + (dataCollectionEnabled ? 1 : 0)}
                    </span>
                  )}
                </div>
                <div className="plugins-header-right">
                  <button 
                    className="plugin-add-btn" 
                    onClick={(e) => { e.stopPropagation(); handleInstallPlugin(); }}
                    title="Install plugin"
                  >
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <line x1="12" y1="5" x2="12" y2="19"></line>
                      <line x1="5" y1="12" x2="19" y2="12"></line>
                    </svg>
                  </button>
                </div>
              </div>

              {isPluginsExpanded && (
                <div className="plugins-pane">
                  {/* Plugin List */}
                  {plugins.length === 0 && !dataCollectionAvailable ? (
                        <div className="plugins-empty">
                          <div className="plugins-empty-icon">
                            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" opacity="0.3">
                              <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"></path>
                            </svg>
                          </div>
                          <div className="plugins-empty-text">No plugins installed</div>
                          <div className="plugins-empty-hint">Plugins extend OmniEdge functionality</div>
                        </div>
                      ) : (
                        <div className="plugins-list">
                          {/* Data Collection Plugin */}
                          {dataCollectionAvailable && (
                            <div className={`plugin-item ${dataCollectionEnabled ? 'is-enabled' : ''}`}>
                              <div className="plugin-item-header">
                                <div className={`plugin-status-dot ${dataCollectionEnabled ? 'active' : 'disabled'}`}></div>
                                <div className="plugin-info">
                                  <div className="plugin-name">
                                    Data Collection
                                    <span className="demo-badge">DEMO</span>
                                  </div>
                                  <div className="plugin-description">Collect and manage robot training data</div>
                                </div>
                                <div className="plugin-item-right">
                                  <div
                                    className={`ios-switch small ${dataCollectionEnabled ? 'on' : ''}`}
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      setDataCollectionEnabled(!dataCollectionEnabled);
                                    }}
                                    onKeyDown={(e) => { 
                                      if (e.key === 'Enter' || e.key === ' ') { 
                                        e.preventDefault(); 
                                        e.stopPropagation(); 
                                        setDataCollectionEnabled(!dataCollectionEnabled);
                                      } 
                                    }}
                                    tabIndex={0}
                                    role="switch"
                                    aria-checked={dataCollectionEnabled}
                                    aria-label="Enable Data Collection plugin"
                                  >
                                    <div className="dot"></div>
                                  </div>
                                  <button 
                                    className="plugin-open-btn"
                                    onClick={() => invoke('open_data_collection_window')}
                                    disabled={!dataCollectionEnabled}
                                  >
                                    Open
                                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                                      <polyline points="9 18 15 12 9 6"></polyline>
                                    </svg>
                                  </button>
                                </div>
                              </div>
                            </div>
                          )}

                          {/* Other Installed Plugins */}
                          {plugins.map(plugin => {
                            const isExpanded = expandedPluginId === plugin.id;
                            const isRemoving = pluginToRemove === plugin.id;
                            
                            return (
                              <div 
                                key={plugin.id} 
                                className={`plugin-item ${plugin.enabled ? 'is-enabled' : ''} ${plugin.status === 'error' ? 'has-error' : ''} ${isExpanded ? 'is-expanded' : ''}`}
                              >
                                <div 
                                  className="plugin-item-header" 
                                  onClick={() => setExpandedPluginId(isExpanded ? null : plugin.id)}
                                >
                                  <div className={`plugin-status-dot ${plugin.enabled ? 'active' : 'disabled'}`}></div>
                                  <div className="plugin-info">
                                    <div className="plugin-name">{plugin.name}</div>
                                    {plugin.status === 'error' && plugin.error_message ? (
                                      <div className="plugin-error-text">{plugin.error_message}</div>
                                    ) : (
                                      <div className="plugin-description">{plugin.description}</div>
                                    )}
                                  </div>
                                  <div className="plugin-item-right">
                                    <div
                                      className={`ios-switch small ${plugin.enabled ? 'on' : ''}`}
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        handleTogglePlugin(plugin.id, plugin.enabled);
                                      }}
                                      onKeyDown={(e) => { 
                                        if (e.key === 'Enter' || e.key === ' ') { 
                                          e.preventDefault(); 
                                          e.stopPropagation(); 
                                          handleTogglePlugin(plugin.id, plugin.enabled); 
                                        } 
                                      }}
                                      tabIndex={0}
                                      role="switch"
                                      aria-checked={plugin.enabled}
                                      aria-label={`Enable ${plugin.name} plugin`}
                                    >
                                      <div className="dot"></div>
                                    </div>
                                    <div className="chevron-icon" style={{ transform: isExpanded ? 'rotate(90deg)' : 'none' }}>
                                      <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                                        <polyline points="9 18 15 12 9 6"></polyline>
                                      </svg>
                                    </div>
                                  </div>
                                </div>

                                {isExpanded && (
                                  <div className="plugin-details-panel">
                                    <div className="plugin-detail-row">
                                      <span className="plugin-detail-label">Version</span>
                                      <span className="plugin-detail-value">{plugin.version}</span>
                                    </div>
                                    <div className="plugin-detail-row">
                                      <span className="plugin-detail-label">Author</span>
                                      <span className="plugin-detail-value">{plugin.author}</span>
                                    </div>
                                    <div className="plugin-detail-row">
                                      <span className="plugin-detail-label">Type</span>
                                      <span className="plugin-type-tag">{plugin.plugin_type}</span>
                                    </div>
                                    {plugin.permissions.length > 0 && (
                                      <div className="plugin-detail-row">
                                        <span className="plugin-detail-label">Permissions</span>
                                        <div className="plugin-permissions">
                                          {plugin.permissions.map(perm => (
                                            <span key={perm} className="permission-tag">{perm}</span>
                                          ))}
                                        </div>
                                      </div>
                                    )}

                                    {!isRemoving ? (
                                      <div className="plugin-actions">
                                        <button 
                                          className="plugin-action-btn settings" 
                                          onClick={() => handlePluginSettings(plugin.id)}
                                        >
                                          Settings
                                        </button>
                                        <button 
                                          className="plugin-action-btn remove" 
                                          onClick={() => setPluginToRemove(plugin.id)}
                                        >
                                          Remove
                                        </button>
                                      </div>
                                    ) : (
                                      <div className="plugin-remove-confirm">
                                        <div className="plugin-remove-confirm-text">
                                          Are you sure you want to remove this plugin?
                                        </div>
                                        <div className="plugin-remove-confirm-actions">
                                          <button 
                                            className="plugin-remove-confirm-btn cancel" 
                                            onClick={() => setPluginToRemove(null)}
                                          >
                                            Cancel
                                          </button>
                                          <button 
                                            className="plugin-remove-confirm-btn confirm" 
                                            onClick={() => handleRemovePlugin(plugin.id)}
                                            disabled={isPluginLoading}
                                          >
                                            {isPluginLoading ? 'Removing...' : 'Remove'}
                                          </button>
                                        </div>
                                      </div>
                                    )}
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      )}
                </div>
              )}
                </>
              )}
        </div>
      </div>

      <div className="app-footer-new">
        <div className="footer-item" onClick={() => openURL('https://connect.omniedge.io/dashboard')}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
            <line x1="9" y1="3" x2="9" y2="21"></line>
          </svg>
          <span>Dashboard</span>
        </div>
        <div className="footer-divider"></div>
        <div className="footer-item" onClick={handleOpenDebug}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="16" x2="12" y2="12"></line>
            <line x1="12" y1="8" x2="12.01" y2="8"></line>
          </svg>
          <span>Debug</span>
        </div>
        <div className="footer-divider"></div>
        <div className="footer-item quit" onClick={handleQuit}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"></path>
            <polyline points="16 17 21 12 16 7"></polyline>
            <line x1="21" y1="12" x2="9" y2="12"></line>
          </svg>
          <span>Quit</span>
        </div>
      </div>
    </div>
  );
}

export default App;
