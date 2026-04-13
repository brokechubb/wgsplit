use thiserror::Error;

#[derive(Debug, Error)]
pub enum WgsplitError {
    #[error("Tunnel '{0}' not found")]
    TunnelNotFound(String),

    #[error("Tunnel '{0}' already exists")]
    TunnelExists(String),

    #[error("No active tunnel")]
    NoActiveTunnel,

    #[error("Tunnel already connected: {0}")]
    AlreadyConnected(String),

    #[error("WireGuard error: {0}")]
    WireGuard(String),

    #[error("Routing error: {0}")]
    Routing(String),

    #[error("Nftables error: {0}")]
    Nftables(String),

    #[error("DNS resolution error: {0}")]
    Dns(String),

    #[error("Cgroup error: {0}")]
    Cgroup(String),

    #[error("Process monitor error: {0}")]
    ProcessMonitor(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, WgsplitError>;
