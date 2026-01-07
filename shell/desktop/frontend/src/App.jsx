import { useState, useEffect } from 'react';
import './App.css';
import { Login, GetNetworks, Connect, Disconnect, GetStatus, GetVirtualIP } from "../wailsjs/go/bridge/Bridge";
import { EventsOn } from "../wailsjs/runtime/runtime";

function App() {
    const [status, setStatus] = useState('disconnected');
    const [virtualIP, setVirtualIP] = useState('');
    const [networks, setNetworks] = useState([]);
    const [selectedNetwork, setSelectedNetwork] = useState('');
    const [securityKey, setSecurityKey] = useState('');
    const [isLoggedIn, setIsLoggedIn] = useState(false);
    const [error, setError] = useState('');
    const [isLoading, setIsLoading] = useState(false);

    useEffect(() => {
        EventsOn("status-changed", (newStatus) => {
            setStatus(newStatus);
            if (newStatus === 'connected') {
                GetVirtualIP().then(setVirtualIP);
            }
        });
        EventsOn("error", (err) => {
            setError(err);
            setIsLoading(false);
        });
    }, []);

    const handleLogin = async () => {
        setIsLoading(true);
        setError('');
        const result = await Login(securityKey);
        setIsLoading(false);
        if (result.success) {
            setIsLoggedIn(true);
            const nets = await GetNetworks();
            setNetworks(nets || []);
            if (nets && nets.length > 0) {
                setSelectedNetwork(nets[0].id);
            }
        } else {
            setError(result.message);
        }
    };

    const handleConnect = async () => {
        if (!selectedNetwork) return;
        setIsLoading(true);
        setError('');
        try {
            await Connect(selectedNetwork);
        } catch (e) {
            setError(e.toString());
        }
        setIsLoading(false);
    };

    const handleDisconnect = async () => {
        await Disconnect();
        setVirtualIP('');
    };

    const getStatusColor = () => {
        switch (status) {
            case 'connected': return '#00ff88';
            case 'connecting': return '#ffaa00';
            case 'error': return '#ff4444';
            default: return '#666';
        }
    };

    return (
        <div className="app">
            <div className="drag-region"></div>

            <div className="header">
                <div className="logo">
                    <span className="logo-icon">⬡</span>
                    <span className="logo-text">OmniEdge</span>
                </div>
            </div>

            <div className="status-card">
                <div className="status-indicator" style={{ backgroundColor: getStatusColor() }}></div>
                <div className="status-info">
                    <div className="status-label">{status.toUpperCase()}</div>
                    {virtualIP && <div className="virtual-ip">{virtualIP}</div>}
                </div>
            </div>

            {!isLoggedIn ? (
                <div className="login-section">
                    <h2>Connect with Security Key</h2>
                    <input
                        type="password"
                        className="input"
                        placeholder="Enter your security key"
                        value={securityKey}
                        onChange={(e) => setSecurityKey(e.target.value)}
                    />
                    <button
                        className="btn btn-primary"
                        onClick={handleLogin}
                        disabled={isLoading || !securityKey}
                    >
                        {isLoading ? 'Authenticating...' : 'Login'}
                    </button>
                </div>
            ) : (
                <div className="control-section">
                    {networks.length > 0 && (
                        <div className="network-selector">
                            <label>Virtual Network</label>
                            <select
                                className="select"
                                value={selectedNetwork}
                                onChange={(e) => setSelectedNetwork(e.target.value)}
                                disabled={status === 'connected' || status === 'connecting'}
                            >
                                {networks.map(net => (
                                    <option key={net.id} value={net.id}>
                                        {net.name} ({net.ip_range})
                                    </option>
                                ))}
                            </select>
                        </div>
                    )}

                    {status === 'connected' ? (
                        <button className="btn btn-disconnect" onClick={handleDisconnect}>
                            Disconnect
                        </button>
                    ) : (
                        <button
                            className="btn btn-connect"
                            onClick={handleConnect}
                            disabled={isLoading || status === 'connecting'}
                        >
                            {status === 'connecting' ? 'Connecting...' : 'Connect'}
                        </button>
                    )}
                </div>
            )}

            {error && <div className="error-message">{error}</div>}

            <div className="footer">
                <span>Powered by OmniEdge Core</span>
            </div>
        </div>
    );
}

export default App;
