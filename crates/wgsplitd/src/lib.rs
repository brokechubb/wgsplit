mod config;
mod wireguard;
mod routing;
mod nftables_mgr;
mod process_monitor;
mod cgroup;
mod dns_resolver;
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

pub struct AppState {
    pub config: DaemonConfig,
    pub tunnel_store: TunnelStore,
    pub wg_manager: WireGuardManager,
    pub split_tunnel: SplitTunnelManager,
    pub active_tunnel: RwLock<Option<String>>,
}

impl AppState {
    pub fn new(config: DaemonConfig) -> anyhow::Result<Self> {
        let tunnel_store = TunnelStore::new(&config.tunnel_dir)?;
        let wg_manager = WireGuardManager::new(config.routing_table, config.fwmark);
        let split_tunnel = SplitTunnelManager::new(
            config.fwmark,
            config.routing_table,
        )?;
        Ok(Self {
            config,
            tunnel_store,
            wg_manager,
            split_tunnel,
            active_tunnel: RwLock::new(None),
        })
    }

    pub fn cleanup_stale(&self) {
        self.wg_manager.cleanup_stale();
    }
}

pub async fn run(config: DaemonConfig) -> anyhow::Result<()> {
    let state = Arc::new(AppState::new(config)?);

    state.cleanup_stale();

    info!("wgsplitd starting");

    let ipc_server = IpcServer::new(state.clone(), &state.config.socket_path)?;
    ipc_server.run().await
}
