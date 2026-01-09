import { useState, useEffect, useRef, useCallback } from 'react';
import './App.css';
import { Events, Browser } from "@wailsio/runtime";
import * as BridgeService from "../bindings/omniedge-desktop/bridgeservice.js";
import { QRCodeSVG } from 'qrcode.react';

function App() {
    const [status, setStatus] = useState('disconnected');
    const [virtualIP, setVirtualIP] = useState('');
    const [deviceName, setDeviceName] = useState('');
    const [networkName, setNetworkName] = useState('');
    const [networks, setNetworks] = useState([]);
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [profile, setProfile] = useState(null);
    const [logo, setLogo] = useState(''); // logo not currently used in the render?
    const [securityKey, setSecurityKey] = useState('');
    const [error, setError] = useState('');
    const [isLoading, setIsLoading] = useState(true); // Start loading while checking auto-login
    const [activeNetwork, setActiveNetwork] = useState(null);
    const [expandedNetworks, setExpandedNetworks] = useState({});
    const [networkDevices, setNetworkDevices] = useState({});
    const [qrInfo, setQrInfo] = useState(null);
    const [isQrMode, setIsQrMode] = useState(false);
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
    }, [isLoggedIn, networks, expandedNetworks, isLoading, resizeToContent, isQrMode]);

    useEffect(() => {
        BridgeService.GetLogos().then(setLogo);
        BridgeService.GetDeviceName().then(setDeviceName);

        Events.On("status-changed", (event) => {
            const newStatus = event.data;
            setStatus(newStatus);
            refreshConnectionInfo();
        });

        // Login Listeners (Shared between QR and Browser)
        Events.On("login-success", () => {
            handleSuccessfulLogin();
        });
        Events.On("login-failed", (event) => {
            setError("Login failed: " + event.data);
            setIsQrMode(false);
            setQrInfo(null);
            setIsLoading(false);
        });

        // Legacy QR Login Listeners
        Events.On("qr-login-success", () => {
            handleSuccessfulLogin();
        });

        Events.On("qr-login-failed", (event) => {
            setError("QR Login failed: " + event.data);
            setIsQrMode(false);
            setQrInfo(null);
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
            setIsQrMode(false);
            setQrInfo(null);
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

    const handleLogin = async () => {
        setIsLoading(true);
        setError('');
        try {
            const result = await BridgeService.Login(securityKey);
            if (result.success) {
                handleSuccessfulLogin();
            } else {
                setError(result.message);
            }
        } catch (err) {
            setError("Login failed. Check security key.");
        }
        setIsLoading(false);
    };

    const handleBrowserLogin = async () => {
        setIsLoading(true);
        setError('');
        try {
            const result = await BridgeService.StartBrowserLogin();
            if (result.success) {
                // We don't call handleSuccessfulLogin here yet, 
                // we wait for the "login-success" event from the backend
                setQrInfo(result.info);
                setError("Please complete the login in your browser...");
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

    const handleStartQrLogin = async () => {
        setIsLoading(true);
        setError('');
        try {
            const result = await BridgeService.StartQRLogin();
            if (result.success) {
                setQrInfo(result.info);
                setIsQrMode(true);
            } else {
                setError(result.message);
            }
        } catch (err) {
            setError("Failed to start QR login");
        }
        setIsLoading(false);
    };

    const handleCancelQrLogin = () => {
        BridgeService.CancelQRLogin();
        setIsQrMode(false);
        setQrInfo(null);
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
        // Always fetch devices when expanding to ensure status is synchronized
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

    if (!isLoggedIn) {
        return (
            <div className="app" ref={appRef}>
                <div className="login-view">
                    <div style={{ fontSize: 24, color: '#007AFF', marginBottom: 16 }}>⬡</div>
                    <h2 style={{ fontSize: 16, fontWeight: 700, marginBottom: 4 }}>OmniEdge</h2>

                    {!isQrMode ? (
                        <>
                            <p style={{ marginBottom: 20, fontSize: 12, opacity: 0.6 }}>Log in to your mesh network</p>
                            <input
                                type="password"
                                placeholder="Security Key"
                                className="security-input"
                                value={securityKey}
                                onChange={(e) => setSecurityKey(e.target.value)}
                                onKeyDown={(e) => e.key === 'Enter' && handleLogin()}
                            />
                            <button className="btn-primary" style={{ cursor: 'pointer' }} onClick={handleLogin}>
                                {isLoading ? 'Connecting...' : 'Log in'}
                            </button>
                            <div style={{ marginTop: 12 }}>
                                <button className="btn-primary" style={{ cursor: 'pointer' }} onClick={handleBrowserLogin}>
                                    Log in with Browser
                                </button>
                            </div>
                        </>
                    ) : (
                        <div className="qr-container" style={{ textAlign: 'center' }}>
                            <p style={{ marginBottom: 16, fontSize: 12, opacity: 0.6 }}>Scan with OmniEdge Mobile App</p>
                            <div style={{ background: 'white', padding: 12, borderRadius: 8, display: 'inline-block', marginBottom: 16 }}>
                                {qrInfo && <QRCodeSVG value={qrInfo.qr_data} size={150} />}
                            </div>
                            <button className="btn-secondary" style={{ width: '100%', cursor: 'pointer' }} onClick={handleCancelQrLogin}>
                                Cancel
                            </button>
                        </div>
                    )}

                    {error && <div className="error-text" style={{ marginTop: 10 }}>{error}</div>}
                </div>
                <div className="divider"></div>
                <div className="menu-item quit-row" onClick={() => BridgeService.Quit()}>
                    <span>Quit</span>
                    <span className="shortcut">⌘Q</span>
                </div>
            </div>
        );
    }

    return (
        <div className="app" ref={appRef}>
            {/* Native Login/Logout Toggle */}
            <div className="menu-item" onClick={handleLogout}>
                Log out as {profile?.name || 'User'}
            </div>
            <div className="divider"></div>

            {/* Logical Section: This Device Dashboard */}
            <div className="detail-section no-hover">
                <div className="detail-line">
                    <span className="detail-label">Network:</span>
                    <span className="detail-value truncate">
                        {status === 'connected' ? networkName : 'Not connected'}
                    </span>
                </div>
                <div className="detail-line">
                    <span className="detail-label">Device:</span>
                    <span className="detail-value truncate">{deviceName}</span>
                </div>
                <div className="detail-line">
                    <span className="detail-label">IP:</span>
                    <span className="detail-value">{virtualIP || '---.---.---.---'}</span>
                </div>
            </div>

            <div className="divider"></div>

            {/* Logical Section: network selection */}
            <div className="menu-item subheader">
                My Virtual Networks
            </div>

            {networks.map(net => {
                const isExpanded = expandedNetworks[net.id];
                const isActive = activeNetwork === net.id || status === 'connected' && networkName === net.name;

                return (
                    <div key={net.id}>
                        <div className="menu-item" onClick={() => toggleNetworkExpand(net.id)}>
                            <div className="network-row">
                                <span className="truncate" style={{ fontWeight: isActive ? '500' : '400' }}>{net.name}</span>
                                <div className="chevron" style={{ transform: isExpanded ? 'rotate(90deg)' : 'none', transition: 'transform 0.1s' }}>
                                    <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                                        <polyline points="9 18 15 12 9 6"></polyline>
                                    </svg>
                                </div>
                            </div>
                        </div>
                        {isExpanded && (
                            <div className="network-detail">
                                <div className="detail-header">
                                    <span style={{ fontSize: 12, fontWeight: 500, opacity: 0.7 }}>Connection</span>
                                    <div
                                        className={`ios-switch ${isActive ? 'on' : ''}`}
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            isActive ? handleDisconnect() : handleConnect(net.id);
                                        }}
                                        style={{ transform: 'scale(1.2)', transformOrigin: 'right' }}
                                    >
                                        <div className="dot"></div>
                                    </div>
                                </div>
                                <div className="divider" style={{ margin: '4px 0' }}></div>
                                <div style={{ padding: '8px 0' }}>
                                    {(networkDevices[net.id] || []).map(dev => (
                                        <div key={dev.id || dev.virtual_ip}>
                                            <div className="device-grid">
                                                <div className="device-name">
                                                    <span className="expand-icon">
                                                        {dev.has_sub_devices ? "−" : ""}
                                                    </span>
                                                    <span className="truncate">{dev.name}</span>
                                                </div>
                                                <div className="device-ip">{dev.virtual_ip}</div>
                                                <div className={`device-status ${dev.online ? 'status-online' : 'status-offline'}`}>
                                                    <span className="status-dot"></span>
                                                    {dev.online ? 'Online' : 'Offline'}
                                                </div>
                                            </div>

                                            {/* Sub-devices support */}
                                            {dev.has_sub_devices && (dev.sub_devices || []).map(sub => (
                                                <div key={sub.id} className="device-grid sub-device">
                                                    <div className="device-name">
                                                        <span className="truncate">{sub.name}</span>
                                                    </div>
                                                    <div className="device-ip">{sub.ip}</div>
                                                    <div className={`device-latency ${sub.latency < 50 ? 'latency-fast' : sub.latency < 100 ? 'latency-medium' : 'latency-slow'}`}>
                                                        {sub.latency} ms
                                                    </div>
                                                </div>
                                            ))}
                                        </div>
                                    ))}
                                    {(!networkDevices[net.id] || networkDevices[net.id].length === 0) && (
                                        <div className="nested-item" style={{ opacity: 0.4 }}>
                                            <span>No other devices found</span>
                                        </div>
                                    )}
                                </div>
                            </div>
                        )}
                    </div>
                );
            })}

            <div className="divider"></div>

            {/* Logical Section: Utilities matching native OmniMainMenu.swift */}
            <div className="footer">
                <div className="menu-item utility-item" onClick={() => openURL('https://connect.omniedge.io/dashboard')}>
                    Dashboard ...
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
