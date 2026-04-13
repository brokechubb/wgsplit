use std::process::Command;
use log::debug;
use wgsplit_common::error::{Result, WgsplitError};

pub struct RoutingManager {
    fwmark: u32,
    table: u32,
}

impl RoutingManager {
    pub fn new(fwmark: u32, table: u32) -> Self {
        Self { fwmark, table }
    }

    pub fn add_host_route_vpn(&self, ip: &str, iface: &str) -> Result<()> {
        debug!("Adding VPN host route: {ip} dev {iface}");
        self.run_cmd("ip", &["route", "add", ip, "dev", iface])
    }

    pub fn add_host_route_direct(&self, ip: &str) -> Result<()> {
        let gw = self.get_default_gateway()?;
        let dev = self.get_default_interface()?;
        debug!("Adding direct host route: {ip} via {gw} dev {dev}");
        self.run_cmd("ip", &["route", "add", ip, "via", &gw, "dev", &dev])
    }

    pub fn del_host_route(&self, ip: &str) -> Result<()> {
        debug!("Deleting host route: {ip}");
        let _ = self.run_cmd("ip", &["route", "del", ip]);
        Ok(())
    }

    pub fn add_fwmark_rule(&self) -> Result<()> {
        let fwmark_hex = format!("0x{:x}", self.fwmark);
        let table = self.table.to_string();
        self.run_cmd("ip", &[
            "-4", "rule", "add", "fwmark", &fwmark_hex, "table", &table,
        ])
    }

    pub fn del_fwmark_rule(&self) -> Result<()> {
        let fwmark_hex = format!("0x{:x}", self.fwmark);
        let table = self.table.to_string();
        let _ = self.run_cmd("ip", &[
            "-4", "rule", "del", "fwmark", &fwmark_hex, "table", &table,
        ]);
        Ok(())
    }

    pub fn get_default_gateway(&self) -> Result<String> {
        let output = self.run_cmd_capture("ip", &["route", "show", "default"])?;
        let parts: Vec<&str> = output.split_whitespace().collect();
        let via_idx = parts.iter().position(|&p| p == "via")
            .ok_or_else(|| WgsplitError::Routing("No default gateway found".into()))?;
        Ok(parts.get(via_idx + 1).unwrap_or(&"").to_string())
    }

    pub fn get_default_interface(&self) -> Result<String> {
        let output = self.run_cmd_capture("ip", &["route", "show", "default"])?;
        let parts: Vec<&str> = output.split_whitespace().collect();
        let dev_idx = parts.iter().position(|&p| p == "dev")
            .ok_or_else(|| WgsplitError::Routing("No default interface found".into()))?;
        Ok(parts.get(dev_idx + 1).unwrap_or(&"").to_string())
    }

    fn run_cmd(&self, cmd: &str, args: &[&str]) -> Result<()> {
        let status = Command::new(cmd)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(WgsplitError::Routing(format!(
                "{cmd} {} failed", args.join(" "),
            )));
        }
        Ok(())
    }

    fn run_cmd_capture(&self, cmd: &str, args: &[&str]) -> Result<String> {
        let output = Command::new(cmd).args(args).output()?;
        if !output.status.success() {
            return Err(WgsplitError::Routing(format!(
                "{cmd} {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim(),
            )));
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
