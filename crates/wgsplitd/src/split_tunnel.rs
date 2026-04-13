use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;
use log::info;
use wgsplit_common::error::Result;
use wgsplit_common::types::SplitTunnelingSettings;

use crate::cgroup::CgroupManager;
use crate::process_monitor::{ProcessMonitor, start_process_monitor_loop};
use crate::dns_resolver::DnsResolveLoop;
use crate::host_routes::HostRoutesManager;
use crate::routing::RoutingManager;
use crate::nftables_mgr::NftablesManager;

pub struct SplitTunnelManager {
    fwmark: u32,
    table: u32,
    cgroup: Arc<std::sync::Mutex<CgroupManager>>,
    process_monitor: Arc<std::sync::Mutex<ProcessMonitor>>,
    host_routes: Arc<RwLock<HostRoutesManager>>,
    nftables: Arc<std::sync::Mutex<NftablesManager>>,
    settings: Arc<RwLock<SplitTunnelingSettings>>,
    dns_loop_cancel: Arc<std::sync::Mutex<Option<Arc<AtomicBool>>>>,
}

impl SplitTunnelManager {
    pub fn new(fwmark: u32, table: u32) -> Result<Self> {
        let routing = RoutingManager::new(fwmark, table);
        let cgroup = CgroupManager::new()?;
        let process_monitor = ProcessMonitor::new(Vec::new());
        let host_routes = HostRoutesManager::new(routing);
        let nftables = NftablesManager::new(fwmark);

        Ok(Self {
            fwmark,
            table,
            cgroup: Arc::new(std::sync::Mutex::new(cgroup)),
            process_monitor: Arc::new(std::sync::Mutex::new(process_monitor)),
            host_routes: Arc::new(RwLock::new(host_routes)),
            nftables: Arc::new(std::sync::Mutex::new(nftables)),
            settings: Arc::new(RwLock::new(SplitTunnelingSettings::default())),
            dns_loop_cancel: Arc::new(std::sync::Mutex::new(None)),
        })
    }

    pub fn apply_settings(&self, settings: &SplitTunnelingSettings) -> Result<()> {
        if !settings.enabled {
            self.disable()?;
            return Ok(());
        }

        {
            let mut cgroup = self.cgroup.lock().unwrap();
            cgroup.enable()?;
        }

        let app_paths: Vec<String> = match settings.mode {
            wgsplit_common::types::SplitTunnelingMode::Exclusive => settings.apps.direct.clone(),
            wgsplit_common::types::SplitTunnelingMode::Inclusive => settings.apps.vpn.clone(),
        };

        {
            let mut monitor = self.process_monitor.lock().unwrap();
            monitor.update_app_list(app_paths.clone());
            let matched = monitor.scan_all();
            let cgroup = self.cgroup.lock().unwrap();
            for pid in matched {
                if let Err(e) = cgroup.add_pid(pid) {
                    log::warn!("Failed to add PID {pid} to cgroup: {e}");
                }
            }
        }

        start_process_monitor_loop(
            self.process_monitor.clone(),
            self.cgroup.clone(),
        );

        if !settings.domains.vpn.is_empty() || !settings.domains.direct.is_empty() {
            self.start_dns_loop(settings)?;
        }

        {
            let mut s = self.settings.blocking_write();
            *s = settings.clone();
        }

        info!("Split tunneling enabled (mode={:?})", settings.mode);
        Ok(())
    }

    pub fn on_tunnel_connected(&self, iface: &str) -> Result<()> {
        let mut host_routes = self.host_routes.blocking_write();
        host_routes.set_vpn_interface(iface);
        Ok(())
    }

    pub fn on_tunnel_disconnected(&self) -> Result<()> {
        let mut host_routes = self.host_routes.blocking_write();
        host_routes.clear_vpn_interface();
        host_routes.clear_all_routes()?;
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        {
            let mut cgroup = self.cgroup.lock().unwrap();
            cgroup.disable()?;
        }

        {
            let mut host_routes = self.host_routes.blocking_write();
            host_routes.clear_all_routes()?;
        }

        {
            let nft = self.nftables.lock().unwrap();
            nft.teardown()?;
        }

        info!("Split tunneling disabled");
        Ok(())
    }

    fn start_dns_loop(&self, settings: &SplitTunnelingSettings) -> Result<()> {
        {
            let mut cancel = self.dns_loop_cancel.lock().unwrap();
            if let Some(ref token) = *cancel {
                token.store(true, Ordering::SeqCst);
            }
        }

        let cancel_token = Arc::new(AtomicBool::new(false));
        {
            let mut cancel = self.dns_loop_cancel.lock().unwrap();
            *cancel = Some(cancel_token.clone());
        }

        let vpn_domains = settings.domains.vpn.clone();
        let direct_domains = settings.domains.direct.clone();
        let interval_secs = settings.domains.resolve_interval_secs;
        let host_routes = self.host_routes.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create DNS resolver runtime");
            rt.block_on(async move {
                let resolver = crate::dns_resolver::DnsResolver::new()
                    .expect("Failed to create DNS resolver");

                let mut resolve_and_update = || async {
                    if cancel_token.load(Ordering::SeqCst) {
                        return;
                    }

                    let mut all_resolved = std::collections::HashMap::new();
                    let mut all_ok = true;
                    let expected_count = vpn_domains.len() + direct_domains.len();

                    let vpn_results = resolver.resolve_all(&vpn_domains).await;
                    for host in &vpn_results {
                        if cancel_token.load(Ordering::SeqCst) {
                            return;
                        }
                        if host.ips.is_empty() {
                            all_ok = false;
                        } else {
                            all_resolved.insert(format!("vpn:{}", host.domain), host.ips.clone());
                        }
                    }

                    let direct_results = resolver.resolve_all(&direct_domains).await;
                    for host in &direct_results {
                        if cancel_token.load(Ordering::SeqCst) {
                            return;
                        }
                        if host.ips.is_empty() {
                            all_ok = false;
                        } else {
                            all_resolved.insert(format!("direct:{}", host.domain), host.ips.clone());
                        }
                    }

                    if !all_ok && expected_count > 0 {
                        log::warn!("DNS resolution incomplete ({}/{} domains), keeping existing routes", all_resolved.len(), expected_count);
                        return;
                    }

                    if !all_resolved.is_empty() {
                        let mut hr = host_routes.write().await;
                        if let Err(e) = hr.update_routes(&all_resolved) {
                            log::error!("Failed to update host routes: {e}");
                        }
                    }
                };

                resolve_and_update().await;

                let mut interval_timer = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs as u64));
                loop {
                    tokio::select! {
                        _ = interval_timer.tick() => {
                            if cancel_token.load(Ordering::SeqCst) {
                                info!("DNS loop cancelled");
                                break;
                            }
                            resolve_and_update().await;
                        }
                    }
                }
            });
        });

        Ok(())
    }
}
