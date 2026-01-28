import { useState, useEffect, useRef, useCallback } from 'react';
import './App.css';
import { invoke } from "@tauri-apps/api/core";
import logo from './assets/logo.png';

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
  const [hasPermission, setHasPermission] = useState(true);
  const [helperInstalling, setHelperInstalling] = useState(false);
  const [myDeviceID, setMyDeviceID] = useState('');
  const [myAPIIP, setMyAPIIP] = useState('');
  const [showSetup, setShowSetup] = useState(false);
  const [showDebug, setShowDebug] = useState(false);
  const [debugData, setDebugData] = useState<any>(null);
  const [copiedIP, setCopiedIP] = useState<string | null>(null);
  const appRef = useRef<HTMLDivElement>(null);

  // Resize window to fit content
  const resizeToContent = useCallback(async () => {
    if (appRef.current) {
      // Use offsetHeight to get the actual rendered height of content
      const contentHeight = appRef.current.offsetHeight;
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
      resizeTimeout = setTimeout(resizeToContent, 20);
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
  }, [isLoggedIn, networks, expandedNetworks, isLoading, isConnecting, resizeToContent, isWaitingForBrowser, isExitNodesExpanded, isBecomingExitNode, error, networkDevices, status, virtualIP, showDebug, showSetup]);

  useEffect(() => {
    const init = async () => {
      try {
        const helperActive = await invoke('check_helper') as boolean;
        const elevated = await invoke('check_is_admin') as boolean;

        const canConnect = helperActive || elevated;
        setHasPermission(canConnect);

        if (!canConnect) {
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

      const currState = await invoke('get_state') as string;
      if (currState.toLowerCase() === 'disconnected' && netsArray.length > 0) {
        handleConnect(netsArray[0].id);
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
      await invoke('open_browser', { url: resp.auth_url });
      const auth: any = await invoke('wait_for_session_login', { sessionId: resp.id });
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

  const handleCancelBrowserLogin = () => {
    setIsWaitingForBrowser(false);
    setError("");
  };

  const handleLogout = () => {
    invoke('disconnect');
    setIsLoggedIn(false);
    setProfile(null);
    setNetworks([]);
    setActiveNetwork(null);
    setConnectedNetworkID('');
    setNetworkName('');
    setError('');
  };

  const handleConnect = async (networkId: string) => {
    if (!hasPermission) {
      setError("Admin rights or background service required.");
      return;
    }
    setIsConnecting(true);
    setError('');
    setActiveNetwork(networkId);
    try {
      await invoke('connect', { networkId, as_exit_node: isBecomingExitNode });
      await refreshConnectionInfo();
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

  const handleInstallHelper = async () => {
    setHelperInstalling(true);
    setError('');
    try {
      await invoke('install_helper');
      // Re-check helper status after installation
      const helperActive = await invoke('check_helper') as boolean;
      if (helperActive) {
        setHasPermission(true);
        setError('');
        setShowSetup(false);
      } else {
        // Double check admin just in case
        const elevated = await invoke('check_is_admin') as boolean;
        setHasPermission(elevated);
      }
    } catch (err: any) {
      setError(`Failed to install helper: ${err.toString()}`);
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
              {status === 'connected' ? 'Secure' : (status === 'connecting' ? 'Connecting...' : 'Disconnected')}
            </span>
          </div>
        </div>
        <div className="header-right">
          <div className="login-status-container" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <span style={{ fontSize: '11px', opacity: 0.6, fontWeight: 500 }}>
              {isLoggedIn ? 'Online' : 'Sign In'}
            </span>
            <div
              className={`ios-switch header-toggle ${isLoggedIn || isWaitingForBrowser ? 'on' : ''}`}
              onClick={isLoggedIn ? handleLogout : (isWaitingForBrowser ? handleCancelBrowserLogin : handleBrowserLogin)}
              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); (isLoggedIn ? handleLogout : (isWaitingForBrowser ? handleCancelBrowserLogin : handleBrowserLogin))(); } }}
              tabIndex={0}
              role="switch"
              aria-checked={isLoggedIn || isWaitingForBrowser}
              aria-label={isLoggedIn ? 'Sign out' : 'Sign in'}
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
                <h2>Background Service Required</h2>
                <p>To provide secure, non-admin VPN connectivity and background operations, OmniEdge needs to install its helper service.</p>

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
                    const helperActive = await invoke('check_helper') as boolean;
                    if (helperActive) {
                      setHasPermission(true);
                      setShowSetup(false);
                      setError('');
                    }
                  }}
                >
                  Check Again
                </button>
                <div className="setup-hint" style={{ marginTop: '12px' }}>Requires a one-time Administrator elevation</div>
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
                <button className="primary-login-btn" onClick={handleBrowserLogin}>Sign In to Start</button>
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
                  <div className="ip-display-large clickable-ip" onClick={() => handleCopyIP(myAPIIP || virtualIP)}>
                    {myAPIIP || virtualIP || '0.0.0.0'}
                    <div className={`copy-hint ${copiedIP === (myAPIIP || virtualIP) ? 'copied' : ''}`}>
                      {copiedIP === (myAPIIP || virtualIP) ? 'Copied!' : 'Click to copy'}
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
                  const isActive = connectedNetworkID === net.id || activeNetwork === net.id;

                  return (
                    <div key={net.id} className={`network-item-wrapper ${isExpanded ? 'is-expanded' : ''} ${isActive ? 'is-active' : ''}`}>
                      <div className="network-menu-item" onClick={() => toggleNetworkExpand(net.id)}>
                        <div className="item-left">
                          <div className={`status-orb ${isActive ? 'active' : ''}`}></div>
                          <span className="network-name-text truncate">{net.name}</span>
                        </div>
                        <div className="item-right">
                          {isActive && <span className="active-label">Connected</span>}
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
                              className={`ios-switch small ${isActive ? 'on' : ''}`}
                              onClick={(e) => {
                                e.stopPropagation();
                                isActive ? handleDisconnect() : handleConnect(net.id);
                              }}
                              onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); isActive ? handleDisconnect() : handleConnect(net.id); } }}
                              tabIndex={0}
                              role="switch"
                              aria-checked={isActive}
                              aria-label={`VPN connection for ${net.name}`}
                            >
                              <div className="dot"></div>
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

                  {activeNetwork ? (
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
