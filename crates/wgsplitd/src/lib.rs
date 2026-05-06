mod config;
mod wireguard;
mod routing;
mod nftables_mgr;
mod process_monitor;
mod cgroup;
mod dns_resolver;
mod dns_mgr;
mod host_routes;
mod split_tunnel;
mod ipc;
mod tunnel_store;

use std::sync::Arc;
use tokio::sync::RwLock;
use log::info;

pub use config::DaemonConfig;
pub use tunnel_store::TunnelStore;
pub use wireguard::WireGuardManager;
pub use split_tunnel::SplitTunnelManager;
pub use ipc::IpcServer;
pub use dns_mgr::DnsManager;

pub struct AppState {
    pub config: std::sync::RwLock<DaemonConfig>,
    pub tunnel_store: TunnelStore,
    pub wg_manager: WireGuardManager,
    pub split_tunnel: SplitTunnelManager,
    pub active_tunnel: RwLock<Option<String>>,
    pub dns_manager: std::sync::Mutex<DnsManager>,
}

impl AppState {
    pub fn new(config: DaemonConfig) -> anyhow::Result<Self> {
        let _socket_path = config.socket_path.clone();
        let tunnel_dir = config.tunnel_dir.clone();
        let routing_table = config.routing_table;
        let fwmark = config.fwmark;
        let tunnel_store = TunnelStore::new(&tunnel_dir)?;
        let wg_manager = WireGuardManager::new(routing_table, fwmark, fwmark + 1);
        let split_tunnel = SplitTunnelManager::new(
            fwmark,
            routing_table,
        )?;
        Ok(Self {
            config: std::sync::RwLock::new(config),
            tunnel_store,
            wg_manager,
            split_tunnel,
            active_tunnel: RwLock::new(None),
            dns_manager: std::sync::Mutex::new(DnsManager::new()),
        })
    }

    pub fn cleanup_stale(&self) {
        self.wg_manager.cleanup_stale();
    }
}

pub async fn run(config: DaemonConfig) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config)?);
    run_with_state(state).await
}

pub async fn run_with_state(state: Arc<AppState>) -> anyhow::Result<()> {
    state.cleanup_stale();

    info!("wgsplitd starting");

    let socket_path = state.config.read().unwrap().socket_path.clone();
    let ipc_server = IpcServer::new(state.clone(), &socket_path)?;
    ipc_server.run().await
}
