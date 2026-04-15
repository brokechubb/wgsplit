use std::process::Command;
use base64::Engine;
use log::{info, warn, debug};
use wgsplit_common::error::{Result, WgsplitError};
use wgsplit_common::types::{TunnelConfig, TunnelStats};

pub struct WireGuardManager {
    routing_table: u32,
    fwmark: u32,
}

impl WireGuardManager {
    pub fn new(routing_table: u32, fwmark: u32) -> Self {
        Self { routing_table, fwmark }
    }

    pub fn connect(&self, config: &TunnelConfig, tunnel_dir: &str) -> Result<String> {
        let iface = self.interface_name(&config.name);

        if self.interface_exists(&iface) {
            info!("Interface {iface} exists, attempting cleanup before connect");
            let _ = self.disconnect(&iface);
        }

        let result = self.connect_inner(config, tunnel_dir, &iface);
        if result.is_err() {
            info!("Connect failed, cleaning up interface {iface}");
            let _ = self.disconnect(&iface);
        }
        result
    }

    fn connect_inner(&self, config: &TunnelConfig, tunnel_dir: &str, iface: &str) -> Result<String> {
        self.run_cmd("ip", &[
            "link", "add", iface, "type", "wireguard",
        ])?;

        self.run_cmd("wg", &[
            "set", iface,
            "private-key", &self.write_keyfile(&config.interface.private_key, tunnel_dir, &config.name, "private")?,
            "listen-port", "0",
        ])?;

        for addr in &config.interface.address {
            self.run_cmd("ip", &["address", "add", addr, "dev", iface])?;
        }

        if let Some(mtu) = config.interface.mtu {
            self.run_cmd("ip", &["link", "set", iface, "mtu", &mtu.to_string()])?;
        }

        self.run_cmd("ip", &["link", "set", iface, "up"])?;

        for peer in &config.peers {
            let allowed_ips = peer.allowed_ips.join(",");
            let mut args_owned: Vec<String> = vec![
                "set".into(), iface.into(),
                "peer".into(), peer.public_key.clone(),
                "endpoint".into(), peer.endpoint.clone(),
                "allowed-ips".into(), allowed_ips,
            ];
            if let Some(ref psk) = peer.preshared_key {
                let psk_path = self.write_keyfile(psk, tunnel_dir, &config.name, "psk")?;
                args_owned.push("preshared-key".into());
                args_owned.push(psk_path);
            }
            if let Some(ka) = peer.persistent_keepalive {
                args_owned.push("persistent-keepalive".into());
                args_owned.push(ka.to_string());
            }
            let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();
            self.run_cmd("wg", &args_ref)?;
        }

        self.setup_routing(iface, config)?;

        info!("Connected tunnel '{}' on interface {iface}", config.name);
        Ok(iface.to_string())
    }

    pub fn disconnect(&self, iface: &str) -> Result<()> {
        if !self.interface_exists(iface) {
            warn!("Interface {iface} does not exist, nothing to disconnect");
            return Ok(());
        }

        self.cleanup_routing(iface)?;

        self.run_cmd("ip", &["link", "set", iface, "down"])?;
        self.run_cmd("ip", &["link", "del", iface])?;

        info!("Disconnected tunnel on interface {iface}");
        Ok(())
    }

    pub fn get_stats(&self, iface: &str) -> Result<Option<TunnelStats>> {
        if !self.interface_exists(iface) {
            return Ok(None);
        }

        let mut rx_bytes = 0u64;
        let mut tx_bytes = 0u64;
        let mut latest_handshake = None;

        if let Ok(output) = self.run_cmd_capture("wg", &["show", iface, "transfer"]) {
            for line in output.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    rx_bytes += parts[1].parse().unwrap_or(0);
                    tx_bytes += parts[2].parse().unwrap_or(0);
                }
            }
        }

        if let Ok(output) = self.run_cmd_capture("wg", &["show", iface, "latest-handshakes"]) {
            for line in output.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(ts) = parts[1].parse::<u64>() {
                        if ts > 0 {
                            latest_handshake = Some(ts);
                        }
                    }
                }
            }
        }

        Ok(Some(TunnelStats {
            rx_bytes,
            tx_bytes,
            latest_handshake,
        }))
    }

    pub fn generate_keypair() -> (String, String) {
        use x25519_dalek::StaticSecret;
        use rand::rngs::OsRng;
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = x25519_dalek::PublicKey::from(&secret);
        let private_b64 = base64::engine::general_purpose::STANDARD.encode(secret.to_bytes());
        let public_b64 = base64::engine::general_purpose::STANDARD.encode(public.to_bytes());
        (private_b64, public_b64)
    }

    pub fn interface_name(&self, name: &str) -> String {
        format!("wg-{name}")
    }

    pub fn interface_exists(&self, iface: &str) -> bool {
        self.run_cmd("ip", &["link", "show", iface]).is_ok()
    }

    pub fn cleanup_stale(&self) {
        let output = match self.run_cmd_capture("ip", &["-o", "link", "show"]) {
            Ok(o) => o,
            Err(_) => return,
        };
        for line in output.lines() {
            let line = line.trim();
            let parts: Vec<&str> = line.splitn(2, ": ").collect();
            if parts.len() == 2 {
                let iface = parts[1].split(':').next().unwrap_or("");
                if iface.starts_with("wg-") {
                    info!("Cleaning up stale interface: {iface}");
                    let _ = self.disconnect(iface);
                }
            }
        }
    }

    fn setup_routing(&self, iface: &str, config: &TunnelConfig) -> Result<()> {
        let table = self.routing_table.to_string();
        let fwmark_hex = format!("0x{:x}", self.fwmark);

        self.run_cmd("ip", &[
            "-4", "route", "add", "0.0.0.0/0", "dev", iface, "table", &table,
        ])?;

        self.run_cmd("ip", &[
            "-4", "rule", "add", "fwmark", &fwmark_hex, "table", &table,
        ])?;

        self.run_cmd("ip", &[
            "-4", "rule", "add", "table", "main", "suppress_prefixlength", "0",
        ])?;

        for peer in &config.peers {
            let endpoint_host = peer.endpoint.rsplit(':').nth(1).unwrap_or(&peer.endpoint);
            let endpoint_host = endpoint_host.trim_start_matches('[').trim_end_matches(']');
            if let Ok(ip) = endpoint_host.parse::<std::net::IpAddr>() {
                self.add_endpoint_route(&ip.to_string())?;
            } else {
                debug!("Endpoint {endpoint_host} is a hostname, will resolve via main routing table");
            }
        }

        Ok(())
    }

    fn cleanup_routing(&self, iface: &str) -> Result<()> {
        let table = self.routing_table.to_string();
        let fwmark_hex = format!("0x{:x}", self.fwmark);

        let _ = self.run_cmd("ip", &[
            "-4", "route", "del", "0.0.0.0/0", "dev", iface, "table", &table,
        ]);
        let _ = self.run_cmd("ip", &[
            "-4", "rule", "del", "fwmark", &fwmark_hex, "table", &table,
        ]);
        let _ = self.run_cmd("ip", &[
            "-4", "rule", "del", "table", "main", "suppress_prefixlength", "0",
        ]);

        Ok(())
    }

    fn add_endpoint_route(&self, endpoint_ip: &str) -> Result<()> {
        let default_gw = self.get_default_gateway()?;
        let default_iface = self.get_default_interface()?;

        let _ = self.run_cmd("ip", &[
            "route", "del", endpoint_ip,
        ]);
        self.run_cmd("ip", &[
            "route", "add", endpoint_ip, "via", &default_gw, "dev", &default_iface,
        ]).map_err(|e| WgsplitError::Routing(format!("Failed to add endpoint route for {endpoint_ip}: {e}")))?;

        Ok(())
    }

    fn get_default_gateway(&self) -> Result<String> {
        let output = self.run_cmd_capture("ip", &["route", "show", "default"])?;
        let parts: Vec<&str> = output.split_whitespace().collect();
        let via_idx = parts.iter().position(|&p| p == "via").ok_or_else(|| {
            WgsplitError::Routing("No default gateway found".into())
        })?;
        Ok(parts.get(via_idx + 1).unwrap_or(&"").to_string())
    }

    fn get_default_interface(&self) -> Result<String> {
        let output = self.run_cmd_capture("ip", &["route", "show", "default"])?;
        let parts: Vec<&str> = output.split_whitespace().collect();
        let dev_idx = parts.iter().position(|&p| p == "dev").ok_or_else(|| {
            WgsplitError::Routing("No default interface found".into())
        })?;
        Ok(parts.get(dev_idx + 1).unwrap_or(&"").to_string())
    }

    fn write_keyfile(&self, key: &str, tunnel_dir: &str, name: &str, suffix: &str) -> Result<String> {
        let dir = std::path::Path::new(tunnel_dir).join(".keys");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{name}-{suffix}"));
        std::fs::write(&path, key)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(path.to_string_lossy().to_string())
    }

    fn run_cmd(&self, cmd: &str, args: &[&str]) -> Result<()> {
        debug!("Running: {cmd} {}", args.join(" "));
        let output = Command::new(cmd)
            .args(args)
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(WgsplitError::WireGuard(format!(
                "{cmd} {} failed: {stderr}",
                args.join(" "),
            )));
        }
        Ok(())
    }

    fn run_cmd_capture(&self, cmd: &str, args: &[&str]) -> Result<String> {
        debug!("Running: {cmd} {}", args.join(" "));
        let output = Command::new(cmd).args(args).output()?;
        if !output.status.success() {
            return Err(WgsplitError::WireGuard(format!(
                "{cmd} {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim(),
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
