use std::fs;
use std::path::Path;
use wgsplit_common::error::{Result, WgsplitError};
use wgsplit_common::types::TunnelConfig;

pub struct TunnelStore {
    dir: String,
}

impl TunnelStore {
    pub fn new(dir: &str) -> Result<Self> {
        fs::create_dir_all(dir)?;
        Ok(Self { dir: dir.to_string() })
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let entries = fs::read_dir(&self.dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "conf").unwrap_or(false) {
                if let Some(name) = path.file_stem() {
                    names.push(name.to_string_lossy().to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn get(&self, name: &str) -> Result<TunnelConfig> {
        self.validate_name(name)?;
        let path = self.conf_path(name);
        if !path.exists() {
            return Err(WgsplitError::TunnelNotFound(name.to_string()));
        }
        let contents = fs::read_to_string(&path)?;
        self.parse_conf(&contents, name)
    }

    pub fn add(&self, config: &TunnelConfig) -> Result<()> {
        self.validate_name(&config.name)?;
        let path = self.conf_path(&config.name);
        if path.exists() {
            return Err(WgsplitError::TunnelExists(config.name.clone()));
        }
        self.write_conf(config)
    }

    pub fn update(&self, config: &TunnelConfig) -> Result<()> {
        self.validate_name(&config.name)?;
        let path = self.conf_path(&config.name);
        if !path.exists() {
            return Err(WgsplitError::TunnelNotFound(config.name.clone()));
        }
        self.write_conf(config)
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        self.validate_name(name)?;
        let path = self.conf_path(name);
        if !path.exists() {
            return Err(WgsplitError::TunnelNotFound(name.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    pub fn import_conf(&self, name: &str, config_text: &str) -> Result<TunnelConfig> {
        self.validate_name(name)?;
        let path = self.conf_path(name);
        if path.exists() {
            return Err(WgsplitError::TunnelExists(name.to_string()));
        }
        let config = self.parse_conf(config_text, name)?;
        self.write_conf(&config)?;
        Ok(config)
    }

    fn conf_path(&self, name: &str) -> std::path::PathBuf {
        Path::new(&self.dir).join(format!("{name}.conf"))
    }

    fn write_conf(&self, config: &TunnelConfig) -> Result<()> {
        let contents = self.serialize_conf(config)?;
        fs::write(self.conf_path(&config.name), contents)?;
        Ok(())
    }

    fn serialize_conf(&self, config: &TunnelConfig) -> Result<String> {
        let mut s = String::new();
        s.push_str("[Interface]\n");
        s.push_str(&format!("PrivateKey = {}\n", config.interface.private_key));
        for addr in &config.interface.address {
            s.push_str(&format!("Address = {addr}\n"));
        }
        for dns in &config.interface.dns {
            s.push_str(&format!("DNS = {dns}\n"));
        }
        if let Some(mtu) = config.interface.mtu {
            s.push_str(&format!("MTU = {mtu}\n"));
        }
        s.push_str("Table = off\n\n");
        for peer in &config.peers {
            s.push_str("[Peer]\n");
            s.push_str(&format!("PublicKey = {}\n", peer.public_key));
            if let Some(ref psk) = peer.preshared_key {
                s.push_str(&format!("PresharedKey = {psk}\n"));
            }
            s.push_str(&format!("Endpoint = {}\n", peer.endpoint));
            s.push_str(&format!("AllowedIPs = {}\n", peer.allowed_ips.join(", ")));
            if let Some(ka) = peer.persistent_keepalive {
                s.push_str(&format!("PersistentKeepalive = {ka}\n"));
            }
            s.push('\n');
        }
        Ok(s)
    }

    fn parse_conf(&self, contents: &str, name: &str) -> Result<TunnelConfig> {
        let _interface = None::<()>;
        let mut peers: Vec<wgsplit_common::types::PeerConfig> = Vec::new();
        let mut current_peer: Option<wgsplit_common::types::PeerConfig> = None;

        let mut private_key = String::new();
        let mut address = Vec::new();
        let mut dns = Vec::new();
        let mut mtu: Option<u16> = None;

        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match line {
                "[Interface]" => {
                    if let Some(p) = current_peer.take() {
                        peers.push(p);
                    }
                }
                "[Peer]" => {
                    if let Some(p) = current_peer.take() {
                        peers.push(p);
                    }
                    current_peer = Some(wgsplit_common::types::PeerConfig {
                        public_key: String::new(),
                        preshared_key: None,
                        endpoint: String::new(),
                        allowed_ips: Vec::new(),
                        persistent_keepalive: None,
                    });
                }
                _ => {
                    if let Some((key, value)) = line.split_once('=') {
                        let key = key.trim();
                        let value = value.trim();
                        if current_peer.is_some() {
                            if let Some(ref mut p) = current_peer {
                                match key {
                                    "PublicKey" => p.public_key = value.to_string(),
                                    "PresharedKey" => p.preshared_key = Some(value.to_string()),
                                    "Endpoint" => p.endpoint = value.to_string(),
                                    "AllowedIPs" => {
                                        p.allowed_ips = value.split(',')
                                            .map(|s| s.trim().to_string())
                                            .collect();
                                    }
                                    "PersistentKeepalive" => {
                                        p.persistent_keepalive = value.parse().ok();
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            match key {
                                "PrivateKey" => private_key = value.to_string(),
                                "Address" => address.push(value.to_string()),
                                "DNS" => dns.push(value.to_string()),
                                "MTU" => mtu = value.parse().ok(),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        if let Some(p) = current_peer.take() {
            peers.push(p);
        }

        Ok(TunnelConfig {
            name: name.to_string(),
            interface: wgsplit_common::types::InterfaceConfig {
                private_key,
                address,
                dns,
                mtu,
                table: wgsplit_common::types::TableOption::Off,
            },
            peers,
        })
    }

    fn validate_name(&self, name: &str) -> Result<()> {
        if name.is_empty() || name.len() > 64 {
            return Err(WgsplitError::Config("Tunnel name must be 1-64 characters".into()));
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(WgsplitError::Config(
                "Tunnel name can only contain alphanumeric characters, hyphens, and underscores".into(),
            ));
        }
        Ok(())
    }
}
