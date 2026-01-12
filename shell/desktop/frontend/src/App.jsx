import { useState, useEffect, useRef, useCallback } from 'react';
import './App.css';
import { Events, Browser } from "@wailsio/runtime";
import * as BridgeService from "../bindings/omniedge-desktop/bridgeservice.js";
import logo from './assets/images/logo-universal.png';

function App() {
    const [status, setStatus] = useState('disconnected');
    const [virtualIP, setVirtualIP] = useState('');
    const [deviceName, setDeviceName] = useState('');
    const [networkName, setNetworkName] = useState('');
    const [connectedNetworkID, setConnectedNetworkID] = useState('');
    const [networks, setNetworks] = useState([]);
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [profile, setProfile] = useState(null);
    const [error, setError] = useState('');
    const [isLoading, setIsLoading] = useState(true);
    const [isConnecting, setIsConnecting] = useState(false);
    const [activeNetwork, setActiveNetwork] = useState(null);
    const [expandedNetworks, setExpandedNetworks] = useState({});
    const [networkDevices, setNetworkDevices] = useState({});
    const [isBecomingExitNode, setIsBecomingExitNode] = useState(false);
    const [isExitNodesExpanded, setIsExitNodesExpanded] = useState(false);
    const [isWaitingForBrowser, setIsWaitingForBrowser] = useState(false);
    const appRef = useRef(null);

    // Resize window to fit content
    const resizeToContent = useCallback(() => {
        if (appRef.current) {
            const height = appRef.current.scrollHeight + -5; // Add padding
            BridgeService.ResizeWindow(height);
        }
    }, []);

    // Resize on content changes
    useEffect(() => {
        const timer = setTimeout(resizeToContent, 100); // Delay to ensure render
        return () => clearTimeout(timer);
    }, [isLoggedIn, networks, expandedNetworks, isLoading, isConnecting, resizeToContent, isWaitingForBrowser, isExitNodesExpanded, isBecomingExitNode]);

    useEffect(() => {
        BridgeService.GetDeviceName().then(setDeviceName);
        Events.On("status-changed", (event) => {
            const newStatus = event.data;
            setStatus(newStatus);
            refreshConnectionInfo();
        });

        // Login Listeners
        Events.On("login-success", () => {
            handleSuccessfulLogin();
        });

        Events.On("login-failed", (event) => {
            setError("Login failed: " + event.data);
            setIsWaitingForBrowser(false);
            setIsLoading(false);
        });

        BridgeService.GetIsExitNode().then(setIsBecomingExitNode);

        BridgeService.GetStatus().then(currStatus => {
            setStatus(currStatus);
            refreshConnectionInfo();
        });

        // Try auto-login using saved tokens (Keychain)
        BridgeService.TryAutoLogin().then(result => {
            if (result.success) {
                handleSuccessfulLogin();
            }
            setIsLoading(false);
        }).catch(() => {
            setIsLoading(false);
        });
    }, []);

    const handleSuccessfulLogin = async () => {
        setIsLoading(true);
        try {
            const userProfile = await BridgeService.GetProfile();
            setProfile(userProfile);
            const nets = await BridgeService.GetNetworks();
            const netsArray = nets || [];
            setNetworks(netsArray);
            setIsLoggedIn(true);
            setIsWaitingForBrowser(false);

            // Auto-connect flow: If not already connected to a network, connect to the first available one
            const currentStatus = await BridgeService.GetStatus();
            const currentNetID = await BridgeService.GetConnectedNetworkID();

            if (currentStatus === 'disconnected' && !currentNetID && netsArray.length > 0) {
                console.log("Auto-connecting to first network:", netsArray[0].name);
                handleConnect(netsArray[0].id);
            }
        } catch (err) {
            console.error("handleSuccessfulLogin failed:", err);
            setError("Failed to load profile after login.");
        } finally {
            setIsLoading(false);
            setIsWaitingForBrowser(false);
        }
    };

    const refreshConnectionInfo = async () => {
        const vIP = await BridgeService.GetVirtualIP();
        setVirtualIP(vIP);
        const netName = await BridgeService.GetConnectedNetworkName();
        setNetworkName(netName);
        const netID = await BridgeService.GetConnectedNetworkID();
        setConnectedNetworkID(netID);
    };

    const handleBrowserLogin = async () => {
        setIsLoading(true);
        setError('');
        try {
            const result = await BridgeService.StartBrowserLogin();
            if (result.success) {
                setIsWaitingForBrowser(true);
                setError("");
                setIsLoading(false);
            } else {
                setError(result.message);
                setIsLoading(false);
            }
        } catch (err) {
            setError("Browser login failed.");
            setIsLoading(false);
        }
    };

    const handleCancelBrowserLogin = () => {
        // BridgeService.CancelBrowserLogin(); // If available
        setIsWaitingForBrowser(false);
        setError("");
    };

    const handleLogout = () => {
        BridgeService.Disconnect();
        setIsLoggedIn(false);
        setProfile(null);
        setNetworks([]);
        setActiveNetwork(null);
        setConnectedNetworkID('');
        setError('');
        BridgeService.ClearTokens();
    };

    const handleConnect = async (networkId) => {
        setIsConnecting(true);
        setError('');
        setActiveNetwork(networkId); // Optimistic update
        try {
            await BridgeService.Connect(networkId);
            // Refresh info immediately after connect call
            await refreshConnectionInfo();
            setError('');
        } catch (err) {
            console.error(err);
            setActiveNetwork(null); // Rollback
            setError("Connection failed: " + (err.message || "Unknown error"));
        }
        setIsConnecting(false);
    };

    const handleDisconnect = async () => {
        setIsConnecting(true);
        setError('');
        const prevNetwork = activeNetwork;
        setActiveNetwork(null); // Optimistic update
        try {
            await BridgeService.Disconnect();
            await refreshConnectionInfo();
            setError('');
        } catch (err) {
            console.error(err);
            setActiveNetwork(prevNetwork); // Rollback
        }
        setIsConnecting(false);
    };

    const toggleNetworkExpand = async (networkId) => {
        if (!networkId) return;
        const isExpanded = !!expandedNetworks[networkId];
        setExpandedNetworks({ ...expandedNetworks, [networkId]: !isExpanded });
        if (!isExpanded && isLoggedIn) {
            try {
                const devs = await BridgeService.GetNetworkDevices(networkId);
                setNetworkDevices(prev => ({ ...prev, [networkId]: devs || [] }));
            } catch (err) {
                console.error('toggleNetworkExpand - error:', err);
            }
        }
    };

    // Auto-refresh devices for expanded networks OR active network if exit nodes expanded
    useEffect(() => {
        const refreshInterval = setInterval(async () => {
            if (!isLoggedIn) return;

            const networksToRefresh = new Set(Object.keys(expandedNetworks).filter(id => expandedNetworks[id]));
            if (isExitNodesExpanded && activeNetwork) {
                networksToRefresh.add(activeNetwork);
            }

            for (const networkId of networksToRefresh) {
                if (!networkId) continue;
                try {
                    const devs = await BridgeService.GetNetworkDevices(networkId);
                    setNetworkDevices(prev => ({ ...prev, [networkId]: devs || [] }));
                } catch (err) {
                    console.error('Auto-refresh devices error:', err);
                    if (err.message && err.message.includes('not logged in')) {
                        setIsLoggedIn(false);
                    }
                }
            }
        }, 10000);

        return () => clearInterval(refreshInterval);
    }, [expandedNetworks, isExitNodesExpanded, activeNetwork, isLoggedIn]);

    // Sync activeNetwork based on connection status
    useEffect(() => {
        if (networkName && networks.length > 0) {
            const active = networks.find(n => n.name === networkName);
            if (active && activeNetwork !== active.id) {
                setActiveNetwork(active.id);
            }
        } else if (!networkName && status === 'disconnected') {
            setActiveNetwork(null);
        }
    }, [networkName, networks, activeNetwork, status]);

    // Initial fetch for active network devices
    useEffect(() => {
        if (isLoggedIn && activeNetwork && !networkDevices[activeNetwork]) {
            BridgeService.GetNetworkDevices(activeNetwork).then(devs => {
                setNetworkDevices(prev => ({ ...prev, [activeNetwork]: devs || [] }));
            }).catch(err => {
                console.error(err);
                if (err.message && err.message.includes('not logged in')) {
                    setIsLoggedIn(false);
                }
            });
        }
    }, [activeNetwork, isLoggedIn]);

    const handleToggleIsExitNode = async (e) => {
        e.stopPropagation();
        const newVal = !isBecomingExitNode;
        setIsBecomingExitNode(newVal);
        await BridgeService.SetIsExitNode(newVal);
    };

    const handleSelectExitNode = async (exitNodeId) => {
        if (!activeNetwork) return;
        try {
            setIsConnecting(true);
            await BridgeService.SetExitNode(activeNetwork, exitNodeId);
            // Refresh networks to get updated selected_exit_node_id
            const nets = await BridgeService.GetNetworks();
            setNetworks(nets || []);
        } catch (err) {
            console.error(err);
            setError("Failed to set exit node.");
        } finally {
            setIsConnecting(false);
        }
    };

    const openURL = (url) => {
        Browser.OpenURL(url);
    };

    return (
        <div className="app" ref={appRef}>
            {/* Header with Logo and Top-Right Action */}
            <div className="app-header">
                <div className="header-left">
                    <span className="app-name">OmniEdge</span>
                    <span className={`status-pill ${status === 'connected' ? 'online' : ''}`}>
                        {status === 'connected' ? 'Connected' : 'Disconnected'}
                    </span>
                </div>
                <div className="header-right">
                    <div className="login-status-container" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <span style={{ fontSize: '12px', opacity: 0.8, fontWeight: 500 }}>
                            {isLoggedIn ? 'Signed In' : 'Sign In'}
                        </span>
                        <div
                            className={`ios-switch header-toggle ${isLoggedIn || isWaitingForBrowser ? 'on' : ''}`}
                            onClick={isLoggedIn ? handleLogout : (isWaitingForBrowser ? handleCancelBrowserLogin : handleBrowserLogin)}
                        >
                            <div className="dot">
                                {(isWaitingForBrowser || isLoading) && <div className="loader-mini" style={{ width: '12px', height: '12px', border: '2px solid rgba(0,0,0,0.1)', borderTopColor: 'var(--accent-blue)' }}></div>}
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div className="main-content">
                {isWaitingForBrowser && (
                    <div className="status-banner">
                        <span className="banner-text">Waiting for browser login...</span>
                        <span className="banner-cancel" onClick={handleCancelBrowserLogin}>Cancel</span>
                    </div>
                )}

                {error && <div className="error-banner">{error}</div>}

                {!isLoggedIn ? (
                    <div className="logged-out-view">
                        <div className="placeholder-hero">
                            <div className="hero-gradient"></div>
                            <p> Secure P2P mesh networking for AI devices, IoT, and edge computing</p>
                        </div>
                        <div className="locked-info">
                            <div className="divider"></div>
                            <div className="detail-section disabled">
                                <div className="detail-line aligned-row">
                                    <span className="detail-label">Status</span>
                                    <span className="detail-value status-pill">Offline</span>
                                </div>
                                <div className="detail-line aligned-row">
                                    <span className="detail-label">Virtual IP</span>
                                    <span className="detail-value mono">---.---.---.---</span>
                                </div>
                            </div>
                            <div className="divider"></div>
                            <div className="profile-header-row">
                                <span className="profile-email-text truncate">{profile?.email || ' '}</span>
                            </div>
                            <div className="divider"></div>
                            <div className="subheader">Virtual Networks</div>
                            <div className="empty-state">
                                <span>No networks available. Please log in.</span>
                            </div>
                        </div>
                    </div>
                ) : (
                    <div className="dashboard-view">
                        <div className="divider"></div>
                        <div className="detail-section no-hover">
                            <div className="detail-line aligned-row">
                                <span className="detail-label">This Device</span>
                                <span className="detail-value mono">{virtualIP || '---.---.---.---'}</span>
                            </div>
                            {status === 'connected' && (
                                <div className="detail-line aligned-row">
                                    <span className="detail-label">Network</span>
                                    <span className="detail-value truncate">{networkName}</span>
                                </div>
                            )}
                        </div>
                        <div className="divider"></div>
                        <div className="profile-header-row dashboard">
                            <div className="profile-avatar-container">
                                <div className="profile-chip-tiny">
                                    <span className="profile-initial" style={{ fontSize: '9px' }}>{profile?.email?.[0]?.toUpperCase() || 'U'}</span>
                                </div>
                                <div className="user-status-indicator online mini"></div>
                            </div>
                            <span className="profile-email-text truncate">{profile?.email}</span>
                        </div>
                        <div className="divider"></div>
                        <div className="subheader">Virtual Networks</div>

                        <div className="networks-list">
                            {networks.map(net => {
                                const isExpanded = expandedNetworks[net.id];
                                const isActive = connectedNetworkID === net.id || activeNetwork === net.id;

                                return (
                                    <div key={net.id} className="network-item-container">
                                        <div className={`menu-item ${isActive ? 'menu-item--active' : ''}`} onClick={() => toggleNetworkExpand(net.id)}>
                                            <div className="network-row">
                                                <div className="network-info">
                                                    {isActive && <div className="active-dot"></div>}
                                                    <span className="truncate" style={{ fontWeight: isActive ? '600' : '400' }}>{net.name}</span>
                                                </div>
                                                <div className="chevron" style={{ transform: isExpanded ? 'rotate(90deg)' : 'none' }}>
                                                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                                                        <polyline points="9 18 15 12 9 6"></polyline>
                                                    </svg>
                                                </div>
                                            </div>
                                        </div>
                                        {isExpanded && (
                                            <div className="network-detail">
                                                <div className="detail-header">
                                                    <span className="detail-header-label">Connection</span>
                                                    <div
                                                        className={`ios-switch ${isActive ? 'on' : ''}`}
                                                        onClick={(e) => {
                                                            e.stopPropagation();
                                                            isActive ? handleDisconnect() : handleConnect(net.id);
                                                        }}
                                                    >
                                                        <div className="dot"></div>
                                                    </div>
                                                </div>
                                                <div className="divider-dashed" />
                                                <div className="device-list-container">
                                                    {(networkDevices[net.id] || []).map(dev => (
                                                        <div key={dev.id || dev.virtual_ip} className="device-item">
                                                            <div className="device-grid">
                                                                <div className="device-name-container">
                                                                    <span className={`status-dot-mini ${dev.online ? 'online' : ''}`}></span>
                                                                    <span className="truncate">{dev.name}</span>
                                                                </div>
                                                                <div className="device-ip-mini">{dev.virtual_ip}</div>
                                                            </div>
                                                        </div>
                                                    ))}
                                                    {(!networkDevices[net.id] || networkDevices[net.id].length === 0) && (
                                                        <div className="no-devices">No other devices online</div>
                                                    )}
                                                </div>
                                            </div>
                                        )}
                                    </div>
                                );
                            })}
                        </div>
                    </div>
                )}
            </div>
            {isLoggedIn && (
                <>
                    <div className="divider"></div>
                    <div className="subheader-row" onClick={() => setIsExitNodesExpanded(!isExitNodesExpanded)} style={{ cursor: 'pointer' }}>
                        <div className="subheader">Exit Nodes</div>
                        <div className="chevron" style={{ transform: isExitNodesExpanded ? 'rotate(90deg)' : 'none' }}>
                            <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                                <polyline points="9 18 15 12 9 6"></polyline>
                            </svg>
                        </div>
                    </div>

                    {isExitNodesExpanded && (
                        <div className="exit-nodes-content">
                            <div className="menu-item no-hover" style={{ height: '32px', paddingLeft: '36px' }}>
                                <span className="detail-header-label" style={{ fontSize: '12px', opacity: 0.9 }}>Run as Exit Node</span>
                                <div
                                    className={`ios-switch ${isBecomingExitNode ? 'on' : ''}`}
                                    onClick={handleToggleIsExitNode}
                                >
                                    <div className="dot"></div>
                                </div>
                            </div>

                            <div className="divider-dashed" />

                            {activeNetwork ? (
                                <div className="exit-node-selection">
                                    <div
                                        className={`exit-node-option ${!networks.find(n => n.id === activeNetwork)?.selected_exit_node_id ? 'active' : ''}`}
                                        onClick={() => handleSelectExitNode('')}
                                    >
                                        <div className="selection-indicator">
                                            {!networks.find(n => n.id === activeNetwork)?.selected_exit_node_id && (
                                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round">
                                                    <polyline points="20 6 9 17 4 12"></polyline>
                                                </svg>
                                            )}
                                        </div>
                                        <span style={{ fontSize: '13px' }}>No exit node</span>
                                    </div>

                                    <div className="available-label">Available Exit Nodes</div>
                                    <div className="exit-node-list">
                                        {(networkDevices[activeNetwork] || [])
                                            .filter(dev => dev.is_exit_node && dev.exit_node_enabled && !dev.is_me)
                                            .map(dev => {
                                                const isSelected = networks.find(n => n.id === activeNetwork)?.selected_exit_node_id === dev.id;
                                                return (
                                                    <div
                                                        key={dev.id}
                                                        className={`exit-node-option ${isSelected ? 'active' : ''} ${!dev.online ? 'offline' : ''}`}
                                                        onClick={dev.online ? () => handleSelectExitNode(dev.id) : undefined}
                                                    >
                                                        <div className="selection-indicator">
                                                            {isSelected && (
                                                                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="round" strokeLinejoin="round">
                                                                    <polyline points="20 6 9 17 4 12"></polyline>
                                                                </svg>
                                                            )}
                                                        </div>
                                                        <div className="exit-node-info">
                                                            <span className="truncate" style={{ fontSize: '13px', fontWeight: isSelected ? '500' : '400' }}>{dev.name}</span>
                                                            <span className="exit-node-ip mono">{dev.virtual_ip}</span>
                                                        </div>
                                                        {!dev.online && <span className="offline-tag">Offline</span>}
                                                    </div>
                                                );
                                            })}
                                        {!(networkDevices[activeNetwork] || []).some(dev => dev.is_exit_node && dev.exit_node_enabled) && (
                                            <div className="no-exit-nodes">No available exit nodes in this network</div>
                                        )}
                                    </div>
                                </div>
                            ) : (
                                <div className="exit-node-placeholder">
                                    Connect to a network to select an exit node
                                </div>
                            )}
                        </div>
                    )}
                </>
            )}

            <div className="divider"></div>
            <div className="app-footer">
                <div className="menu-item" onClick={() => openURL('https://connect.omniedge.io/dashboard/virtual-networks')}>
                    <span>Dashboard...</span>
                </div>
                <div className="menu-item quit-row" onClick={() => BridgeService.Quit()}>
                    <span>Quit</span>
                    <span className="shortcut">⌘Q</span>
                </div>
            </div>
        </div>
    );
}

export default App;
