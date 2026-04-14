use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TunnelConfig {
    pub name: String,
    pub interface: InterfaceConfig,
    pub peers: Vec<PeerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InterfaceConfig {
    pub private_key: String,
    pub address: Vec<String>,
    #[serde(default)]
    pub dns: Vec<String>,
    #[serde(default)]
    pub mtu: Option<u16>,
    #[serde(default)]
    pub table: TableOption,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TableOption {
    #[default]
    Auto,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerConfig {
    pub public_key: String,
    #[serde(default)]
    pub preshared_key: Option<String>,
    pub endpoint: String,
    pub allowed_ips: Vec<String>,
    #[serde(default)]
    pub persistent_keepalive: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SplitTunnelingSettings {
    pub enabled: bool,
    pub mode: SplitTunnelingMode,
    pub apps: SplitTunnelingApps,
    pub domains: SplitTunnelingDomains,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SplitTunnelingMode {
    Exclusive,
    Inclusive,
}

impl Default for SplitTunnelingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: SplitTunnelingMode::Exclusive,
            apps: SplitTunnelingApps::default(),
            domains: SplitTunnelingDomains::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SplitTunnelingApps {
    #[serde(default)]
    pub vpn: Vec<String>,
    #[serde(default)]
    pub direct: Vec<String>,
}

impl Default for SplitTunnelingApps {
    fn default() -> Self {
        Self {
            vpn: Vec::new(),
            direct: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SplitTunnelingDomains {
    #[serde(default)]
    pub vpn: Vec<String>,
    #[serde(default)]
    pub direct: Vec<String>,
    #[serde(default = "default_resolve_interval")]
    pub resolve_interval_secs: u64,
}

fn default_resolve_interval() -> u64 {
    300
}

impl Default for SplitTunnelingDomains {
    fn default() -> Self {
        Self {
            vpn: Vec::new(),
            direct: Vec::new(),
            resolve_interval_secs: default_resolve_interval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonSettings {
    pub routing_table: u32,
    pub fwmark: u32,
    pub socket_path: String,
    pub tunnel_dir: String,
}

impl Default for DaemonSettings {
    fn default() -> Self {
        Self {
            routing_table: 100,
            fwmark: 0x800,
            socket_path: "/run/wgsplitd.sock".to_string(),
            tunnel_dir: format!("{}/.config/wgsplit/tunnels", std::env::var("HOME").unwrap_or_default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub daemon: DaemonSettings,
    #[serde(default)]
    pub split_tunneling: SplitTunnelingSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            daemon: DaemonSettings::default(),
            split_tunneling: SplitTunnelingSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TunnelStatus {
    pub name: String,
    pub connected: bool,
    pub interface: String,
    pub stats: Option<TunnelStats>,
    pub split_routes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TunnelStats {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub latest_handshake: Option<u64>,
}
