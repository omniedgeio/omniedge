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
    const [networks, setNetworks] = useState([]);
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [profile, setProfile] = useState(null);
    const [error, setError] = useState('');
    const [isLoading, setIsLoading] = useState(true);
    const [activeNetwork, setActiveNetwork] = useState(null);
    const [expandedNetworks, setExpandedNetworks] = useState({});
    const [networkDevices, setNetworkDevices] = useState({});
    const [isWaitingForBrowser, setIsWaitingForBrowser] = useState(false);
    const appRef = useRef(null);

    // Resize window to fit content
    const resizeToContent = useCallback(() => {
        if (appRef.current) {
            const height = appRef.current.scrollHeight + 20; // Add padding
            BridgeService.ResizeWindow(height);
        }
    }, []);

    // Resize on content changes
    useEffect(() => {
        const timer = setTimeout(resizeToContent, 100); // Delay to ensure render
        return () => clearTimeout(timer);
    }, [isLoggedIn, networks, expandedNetworks, isLoading, resizeToContent, isWaitingForBrowser]);

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
            setNetworks(nets || []);
            setIsLoggedIn(true);
            setIsWaitingForBrowser(false);
        } catch (err) {
            console.error("handleSuccessfulLogin failed:", err);
            setError("Failed to load profile after login.");
        } finally {
            setIsLoading(false);
        }
    };

    const refreshConnectionInfo = async () => {
        const vIP = await BridgeService.GetVirtualIP();
        setVirtualIP(vIP);
        const netName = await BridgeService.GetConnectedNetworkName();
        setNetworkName(netName);
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
        setIsLoggedIn(false);
        setProfile(null);
        setNetworks([]);
        BridgeService.ClearTokens();
    };

    const handleConnect = async (networkId) => {
        setIsLoading(true);
        try {
            await BridgeService.Connect(networkId);
            setActiveNetwork(networkId);
        } catch (err) {
            console.error(err);
        }
        setIsLoading(false);
    };

    const handleDisconnect = async () => {
        setIsLoading(true);
        try {
            await BridgeService.Disconnect();
            setActiveNetwork(null);
        } catch (err) {
            console.error(err);
        }
        setIsLoading(false);
    };

    const toggleNetworkExpand = async (networkId) => {
        const isExpanded = !!expandedNetworks[networkId];
        setExpandedNetworks({ ...expandedNetworks, [networkId]: !isExpanded });
        if (!isExpanded) {
            try {
                const devs = await BridgeService.GetNetworkDevices(networkId);
                setNetworkDevices(prev => ({ ...prev, [networkId]: devs || [] }));
            } catch (err) {
                console.error('toggleNetworkExpand - error:', err);
            }
        }
    };

    // Auto-refresh devices for expanded networks every 10 seconds
    useEffect(() => {
        const refreshInterval = setInterval(async () => {
            for (const networkId of Object.keys(expandedNetworks)) {
                if (expandedNetworks[networkId]) {
                    try {
                        const devs = await BridgeService.GetNetworkDevices(networkId);
                        setNetworkDevices(prev => ({ ...prev, [networkId]: devs || [] }));
                    } catch (err) {
                        console.error('Auto-refresh devices error:', err);
                    }
                }
            }
        }, 10000);

        return () => clearInterval(refreshInterval);
    }, [expandedNetworks]);

    const openURL = (url) => {
        Browser.OpenURL(url);
    };

    return (
        <div className="app" ref={appRef}>
            {/* Header with Logo and Top-Right Action */}
            <div className="app-header">
                <div className="header-left">
                    <span className="app-name">OmniEdge</span>
                </div>
                <div className="header-right">
                    {!isLoggedIn ? (
                        <button
                            className={`btn-action ${isWaitingForBrowser ? 'waiting' : ''}`}
                            onClick={handleBrowserLogin}
                            disabled={isLoading || isWaitingForBrowser}
                        >
                            {isWaitingForBrowser ? (
                                <div className="loader-mini"></div>
                            ) : (
                                'Log in'
                            )}
                        </button>
                    ) : (
                        <div className="profile-chip" onClick={handleLogout} title={`Log out from ${profile?.email}`}>
                            <span className="profile-initial">{profile?.name?.[0]?.toUpperCase() || 'U'}</span>
                            <div className={`user-status-indicator ${'online'}`}></div>
                        </div>
                    )}
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
                            <p>Private Mesh Network for Everyone</p>
                        </div>
                        <div className="locked-info">
                            <div className="divider"></div>
                            <div className="detail-section disabled">
                                <div className="detail-line">
                                    <span className="detail-label">Status</span>
                                    <span className="status-pill">Offline</span>
                                </div>
                                <div className="detail-line">
                                    <span className="detail-label">Virtual IP</span>
                                    <span className="detail-value mono">---.---.---.---</span>
                                </div>
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
                            <div className="detail-line">
                                <span className="detail-label">Status</span>
                                <span className={`status-pill ${status === 'connected' ? 'online' : ''}`}>
                                    {status === 'connected' ? 'Connected' : 'Disconnected'}
                                </span>
                            </div>
                            <div className="detail-line">
                                <span className="detail-label">Virtual IP</span>
                                <span className="detail-value mono">{virtualIP || '---.---.---.---'}</span>
                            </div>
                            {status === 'connected' && (
                                <div className="detail-line">
                                    <span className="detail-label">Network</span>
                                    <span className="detail-value truncate">{networkName}</span>
                                </div>
                            )}
                        </div>

                        <div className="divider"></div>
                        <div className="subheader">Virtual Networks</div>

                        <div className="networks-list">
                            {networks.map(net => {
                                const isExpanded = expandedNetworks[net.id];
                                const isActive = activeNetwork === net.id || (status === 'connected' && networkName === net.name);

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

            <div className="divider"></div>
            <div className="app-footer">
                <div className="menu-item" onClick={() => openURL('https://connect.omniedge.io/dashboard')}>
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
