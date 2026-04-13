use std::collections::HashMap;
use log::{info, debug};
use wgsplit_common::error::Result;
use crate::routing::RoutingManager;

pub struct HostRoutesManager {
    routing: RoutingManager,
    vpn_iface: Option<String>,
    current_routes: HashMap<String, String>,
}

impl HostRoutesManager {
    pub fn new(routing: RoutingManager) -> Self {
        Self {
            routing,
            vpn_iface: None,
            current_routes: HashMap::new(),
        }
    }

    pub fn set_vpn_interface(&mut self, iface: &str) {
        self.vpn_iface = Some(iface.to_string());
    }

    pub fn clear_vpn_interface(&mut self) {
        self.vpn_iface = None;
    }

    pub fn update_routes(&mut self, resolved: &HashMap<String, Vec<String>>) -> Result<()> {
        let mut new_routes = HashMap::new();

        for (key, ips) in resolved {
            let (direction, domain) = key.split_once(':').unwrap_or(("vpn", key));
            debug!("Updating routes for {direction}:{domain}: {:?}", ips);

            for ip in ips {
                if self.current_routes.contains_key(ip) {
                    new_routes.insert(ip.clone(), key.clone());
                } else {
                    let route_result = match direction {
                        "vpn" => {
                            if let Some(ref iface) = self.vpn_iface {
                                self.routing.add_host_route_vpn(ip, iface)
                            } else {
                                continue;
                            }
                        }
                        "direct" => {
                            self.routing.add_host_route_direct(ip)
                        }
                        _ => continue,
                    };

                    if let Ok(()) = route_result {
                        new_routes.insert(ip.clone(), key.clone());
                        debug!("Added host route: {ip} → {direction}");
                    }
                }
            }
        }

        for ip in self.current_routes.keys() {
            if !new_routes.contains_key(ip) {
                let _ = self.routing.del_host_route(ip);
                debug!("Removed stale host route: {ip}");
            }
        }

        self.current_routes = new_routes;
        info!("Host routes updated: {} active routes", self.current_routes.len());
        Ok(())
    }

    pub fn clear_all_routes(&mut self) -> Result<()> {
        for ip in self.current_routes.keys() {
            let _ = self.routing.del_host_route(ip);
        }
        self.current_routes.clear();
        Ok(())
    }

    pub fn route_count(&self) -> usize {
        self.current_routes.len()
    }
}
