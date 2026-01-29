use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ConnectionState {
    #[default]
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
