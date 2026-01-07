import { useState, useEffect, useRef } from 'react';
import './App.css';
import { Events, Browser } from "@wailsio/runtime";
import * as BridgeService from "../bindings/omniedge-desktop/bridgeservice.js";

function App() {
    const [status, setStatus] = useState('disconnected');
    const [virtualIP, setVirtualIP] = useState('');
    const [localIP, setLocalIP] = useState('Detecting...');
    const [networks, setNetworks] = useState([]);
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [profile, setProfile] = useState(null);
    const [logo, setLogo] = useState('');
    const [securityKey, setSecurityKey] = useState('');
    const [error, setError] = useState('');
    const [isLoading, setIsLoading] = useState(false);
    const [activeNetwork, setActiveNetwork] = useState(null);
    const [devices, setDevices] = useState([]);
    const [latencies, setLatencies] = useState({});
    const [expandedNetworks, setExpandedNetworks] = useState({});

    const pingInterval = useRef(null);

    useEffect(() => {
        // Fetch logo
        BridgeService.GetLogos().then(setLogo);

        // Listen for status changes
        Events.On("status-changed", (event) => {
            const newStatus = event.data;
            setStatus(newStatus);
            if (newStatus === 'connected') {
                BridgeService.GetVirtualIP().then(setVirtualIP);
            } else if (newStatus === 'disconnected') {
                setVirtualIP('');
            }
        });

        // Initial background checks
        BridgeService.GetLocalIP().then(setLocalIP);
        BridgeService.GetStatus().then(currStatus => {
            setStatus(currStatus);
            if (currStatus === 'connected') {
                BridgeService.GetVirtualIP().then(setVirtualIP);
            }
        });

        return () => stopPingLoop();
    }, []);

    const startPingLoop = () => {
        stopPingLoop();
        pingInterval.current = setInterval(async () => {
            const reachableDevices = devices.filter(d => d.online && d.virtual_ip);
            if (reachableDevices.length > 0) {
                const newLatencies = { ...latencies };
                for (const dev of reachableDevices) {
                    try {
                        const ms = await BridgeService.Ping(dev.virtual_ip);
                        newLatencies[dev.id] = ms;
                    } catch (e) {
                        newLatencies[dev.id] = -1;
                    }
                }
                setLatencies(newLatencies);
            }
        }, 5000);
    };

    const stopPingLoop = () => {
        if (pingInterval.current) {
            clearInterval(pingInterval.current);
            pingInterval.current = null;
        }
    };

    const handleLogin = async () => {
        setIsLoading(true);
        setError('');
        try {
            const result = await BridgeService.Login(securityKey);
            if (result.success) {
                const userProfile = await BridgeService.GetProfile();
                setProfile(userProfile);
                const nets = await BridgeService.GetNetworks();
                setNetworks(nets || []);
                setIsLoggedIn(true);
            } else {
                setError(result.message);
            }
        } catch (err) {
            setError(err.toString());
        }
        setIsLoading(false);
    };

    const handleConnect = async (networkId) => {
        setIsLoading(true);
        try {
            await BridgeService.Connect(networkId);
            setActiveNetwork(networkId);
            const devs = await BridgeService.GetNetworkDevices(networkId);
            setDevices(devs || []);
            startPingLoop();
        } catch (err) {
            setError(err.toString());
        }
        setIsLoading(false);
    };

    const handleDisconnect = async () => {
        setIsLoading(true);
        try {
            await BridgeService.Disconnect();
            setActiveNetwork(null);
            setDevices([]);
            stopPingLoop();
        } catch (err) {
            setError(err.toString());
        }
        setIsLoading(false);
    };

    const toggleNetwork = async (networkId) => {
        const isExpanded = !!expandedNetworks[networkId];
        setExpandedNetworks({ ...expandedNetworks, [networkId]: !isExpanded });
        if (!isExpanded) {
            const devs = await BridgeService.GetNetworkDevices(networkId);
            setDevices(prev => [...prev.filter(d => d.network_id !== networkId), ...(devs || [])]);
        }
    };

    const getLatencyClass = (lx) => {
        if (lx === undefined || lx < 0) return 'latency-bad';
        if (lx < 50) return 'latency-good';
        if (lx < 150) return 'latency-fair';
        return 'latency-bad';
    };

    if (!isLoggedIn) {
        return (
            <div className="app">
                <div className="login-view">
                    {logo ? (
                        <img src={`data:image/png;base64,${logo}`} className="logo-main" alt="OmniEdge" />
                    ) : (
                        <div className="logo-main" style={{ fontSize: 48 }}>⬡</div>
                    )}
                    <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 24 }}>OmniEdge</h2>
                    <div className="login-form">
                        <input
                            type="password"
                            placeholder="Enter Security Key"
                            className="security-input"
                            value={securityKey}
                            onChange={(e) => setSecurityKey(e.target.value)}
                            onKeyDown={(e) => e.key === 'Enter' && handleLogin()}
                        />
                        <button className="btn-primary" onClick={handleLogin} disabled={isLoading || !securityKey}>
                            {isLoading ? 'Authenticating...' : 'Sign In'}
                        </button>
                    </div>
                    {error && <div className="error-text">{error}</div>}
                </div>
            </div>
        );
    }

    return (
        <div className="app">
            <div className="header-logout" onClick={() => setIsLoggedIn(false)}>
                Logout as {profile?.name || 'User'}
            </div>

            <div className="divider-line"></div>

            <main className="main-scroll">
                {/* Active Connection Info */}
                <div className="status-section">
                    <div className="network-active-label">
                        <span>{activeNetwork ? networks.find(n => n.id === activeNetwork)?.name : 'Disconnected'}</span>
                        <span className="this-device-tag">This Device</span>
                    </div>
                    <div className="ip-display">{localIP}</div>
                    {virtualIP && <div className="ip-display virtual-ip-row">{virtualIP}</div>}
                </div>

                <div className="divider-line"></div>

                <div className="section-title">My Virtual Networks</div>

                {networks.map(net => (
                    <div key={net.id}>
                        <div className="menu-row" onClick={() => toggleNetwork(net.id)}>
                            <span className="row-name">{net.name}</span>
                            <div className={`chevron ${expandedNetworks[net.id] ? 'expanded' : ''}`}>
                                <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                                    <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                                </svg>
                            </div>
                        </div>

                        {expandedNetworks[net.id] && (
                            <div className="nested-container">
                                <div className="device-item no-hover" style={{ paddingBottom: 10 }}>
                                    <span className="text-meta" style={{ fontSize: 11 }}>{net.ip_range}</span>
                                    <label className="switch">
                                        <input
                                            type="checkbox"
                                            checked={activeNetwork === net.id}
                                            onChange={() => activeNetwork === net.id ? handleDisconnect() : handleConnect(net.id)}
                                        />
                                        <span className="slider"></span>
                                    </label>
                                </div>

                                {devices.filter(d => d.network_id === net.id || activeNetwork === net.id).map(dev => (
                                    <div key={dev.id} className="device-item">
                                        <div className="device-main">
                                            <span className="device-name">{dev.name}</span>
                                            <span className="device-ip">{dev.virtual_ip}</span>
                                        </div>
                                        <span className={`latency ${getLatencyClass(latencies[dev.id])}`}>
                                            {latencies[dev.id] !== undefined ? `${latencies[dev.id]} ms` : ''}
                                        </span>
                                    </div>
                                ))}
                                {devices.filter(d => d.network_id === net.id).length === 0 && (
                                    <div className="device-item text-meta" style={{ fontSize: 11 }}>No devices online</div>
                                )}
                            </div>
                        )}
                    </div>
                ))}

                <div className="menu-row" onClick={() => Browser.OpenURL("https://omniedge.io/dashboard")}>
                    <span className="row-name">Dashboard...</span>
                </div>
            </main>

            <footer className="footer">
                <div className="footer-row">
                    <span>Auto update</span>
                    <label className="switch" style={{ transform: 'scale(0.8)' }}>
                        <input type="checkbox" defaultChecked />
                        <span className="slider"></span>
                    </label>
                </div>
                <div className="footer-row">Check for update</div>
                <div className="footer-row">About OmniEdge</div>
                <div className="footer-row" onClick={() => Events.Emit("quit")}>
                    <span>Quit</span>
                    <span className="text-meta" style={{ fontSize: 11 }}>⌘Q</span>
                </div>
            </footer>
        </div>
    );
}

export default App;
