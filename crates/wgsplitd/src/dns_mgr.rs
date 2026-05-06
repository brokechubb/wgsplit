use std::fs;
use std::path::Path;
use log::{info, debug, warn};
use wgsplit_common::error::{Result, WgsplitError};

const BACKUP_PATH: &str = "/etc/resolv.conf.wgsplit.bak";

pub struct DnsManager {
    original: Option<String>,
}

impl DnsManager {
    pub fn new() -> Self {
        let original = fs::read_to_string("/etc/resolv.conf").ok();
        Self { original }
    }

    pub fn apply(&mut self, dns_servers: &[String]) -> Result<()> {
        if dns_servers.is_empty() {
            debug!("No DNS servers configured, skipping resolv.conf update");
            return Ok(());
        }

        let expanded: Vec<String> = dns_servers.iter()
            .flat_map(|s| s.split(',').map(|p| p.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();

        if expanded.is_empty() {
            debug!("No DNS servers after expanding commas, skipping resolv.conf update");
            return Ok(());
        }

        if self.original.is_none() {
            self.original = fs::read_to_string("/etc/resolv.conf").ok();
        }

        if let Some(ref original) = self.original {
            if let Err(e) = fs::write(BACKUP_PATH, original) {
                warn!("Failed to back up resolv.conf: {e}");
            } else {
                debug!("Backed up resolv.conf to {BACKUP_PATH}");
            }
        }

        let mut content = String::new();
        content.push_str("# Managed by wgsplitd\n");
        for dns in &expanded {
            content.push_str(&format!("nameserver {dns}\n"));
        }

        let resolv_path = if Path::new("/etc/resolv.conf").is_symlink() {
            match fs::read_link("/etc/resolv.conf") {
                Ok(target) => {
                    if target.is_absolute() {
                        target.to_string_lossy().to_string()
                    } else {
                        format!("/etc/{}", target.to_string_lossy())
                    }
                }
                Err(_) => "/etc/resolv.conf".to_string(),
            }
        } else {
            "/etc/resolv.conf".to_string()
        };

        let _ = fs::remove_file("/etc/resolv.conf");
        fs::write(&resolv_path, &content).map_err(|e| {
            WgsplitError::Other(format!("Failed to write resolv.conf: {e}"))
        })?;

        info!("Updated resolv.conf with DNS servers: {:?}", dns_servers);
        Ok(())
    }

    pub fn restore(&mut self) -> Result<()> {
        if Path::new(BACKUP_PATH).exists() {
            let content = fs::read_to_string(BACKUP_PATH).map_err(|e| {
                WgsplitError::Other(format!("Failed to read resolv.conf backup: {e}"))
            })?;

            let resolv_path = if Path::new("/etc/resolv.conf").is_symlink() {
                match fs::read_link("/etc/resolv.conf") {
                    Ok(target) => {
                        if target.is_absolute() {
                            target.to_string_lossy().to_string()
                        } else {
                            format!("/etc/{}", target.to_string_lossy())
                        }
                    }
                    Err(_) => "/etc/resolv.conf".to_string(),
                }
            } else {
                "/etc/resolv.conf".to_string()
            };

            let _ = fs::remove_file("/etc/resolv.conf");
            fs::write(&resolv_path, &content).map_err(|e| {
                WgsplitError::Other(format!("Failed to restore resolv.conf: {e}"))
            })?;

            let _ = fs::remove_file(BACKUP_PATH);
            info!("Restored original resolv.conf");
        } else {
            debug!("No resolv.conf backup found, skipping restore");
        }
        self.original = None;
        Ok(())
    }
}

impl Drop for DnsManager {
    fn drop(&mut self) {
        if self.original.is_some() {
            let _ = self.restore();
        }
    }
}
