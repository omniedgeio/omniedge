use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Authenticating,
    Authenticated,
    Joining,
    Joined,
    Connecting,
    Connected,
    Reconnecting,
    Stopping,
    Error(String),
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::Disconnected
    }
}
