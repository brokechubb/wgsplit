use std::process::Command;
use log::{info, debug};
use wgsplit_common::error::{Result, WgsplitError};

const CGROUP_NAME: &str = "wgsplit-vpn";

pub struct NftablesManager {
    fwmark: u32,
    wg_fwmark: u32,
}

impl NftablesManager {
    pub fn new(fwmark: u32) -> Self {
        Self { fwmark, wg_fwmark: fwmark + 1 }
    }

    pub fn wg_fwmark(&self) -> u32 {
        self.wg_fwmark
    }

    pub fn setup_full_vpn(&self, vpn_iface: &str) -> Result<()> {
        let fwmark_hex = format!("0x{:x}", self.fwmark);
        let wg_fwmark_hex = format!("0x{:x}", self.wg_fwmark);
        let table_name = "wgsplit";

        self.teardown()?;

        let ruleset = format!(
            r#"
table inet {table_name} {{
    chain mangle {{
        type route hook output priority mangle; policy accept;
        meta mark != {wg_fwmark_hex} counter meta mark set {fwmark_hex}
    }}

    chain postrouting {{
        type nat hook postrouting priority srcnat; policy accept;
        oifname "{vpn_iface}" tcp flags syn tcp option maxseg size set 1200
        oifname "{vpn_iface}" masquerade
    }}
}}
"#
        );

        debug!("Applying nftables full-VPN ruleset:\n{ruleset}");
        let mut child = Command::new("nft")
            .arg("-f")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(ruleset.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(WgsplitError::Nftables("Failed to apply full-VPN nftables ruleset".into()));
        }

        info!("nftables full-VPN rules applied (table={table_name}, fwmark={fwmark_hex})");
        Ok(())
    }

    pub fn setup_base(&self, vpn_iface: &str) -> Result<()> {
        let fwmark_hex = format!("0x{:x}", self.fwmark);
        let table_name = "wgsplit";

        self.teardown()?;

        let ruleset = format!(
            r#"
table inet {table_name} {{
    chain mangle {{
        type route hook output priority mangle; policy accept;
        socket cgroupv2 level 1 "{CGROUP_NAME}" counter meta mark set {fwmark_hex}
    }}

    chain postrouting {{
        type nat hook postrouting priority srcnat; policy accept;
        oifname "{vpn_iface}" tcp flags syn tcp option maxseg size set 1200
        meta mark {fwmark_hex} oifname "{vpn_iface}" masquerade
    }}
}}
"#
        );

        debug!("Applying nftables ruleset:\n{ruleset}");
        let mut child = Command::new("nft")
            .arg("-f")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(ruleset.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(WgsplitError::Nftables("Failed to apply nftables ruleset".into()));
        }

        info!("nftables app routing rules applied (table={table_name}, fwmark={fwmark_hex})");
        Ok(())
    }

    pub fn enable_killswitch(&self, vpn_iface: &str, dns_ips: &[String]) -> Result<()> {
        self.teardown()?;

        let fwmark_hex = format!("0x{:x}", self.fwmark);
        let mut dns_rules = String::new();
        for dns in dns_ips {
            dns_rules.push_str(&format!(
                r#"        ip daddr {dns} udp dport 53 accept
        ip daddr {dns} tcp dport 53 accept
"#
            ));
        }

        let ruleset = format!(
            r#"
table inet wgsplit {{
    chain mangle {{
        type route hook output priority mangle; policy accept;
        socket cgroupv2 level 1 "{CGROUP_NAME}" counter meta mark set {fwmark_hex}
    }}

    chain postrouting {{
        type nat hook postrouting priority srcnat; policy accept;
        meta mark {fwmark_hex} oifname "{vpn_iface}" masquerade
    }}

    chain output {{
        type filter hook output priority filter; policy drop;
        oifname "{vpn_iface}" accept
        oifname "lo" accept
{dns_rules}        ct state established,related accept
        ip daddr 224.0.0.0/3 accept
        ip6 daddr ff00::/8 accept
        udp sport 68 udp dport 67 accept
        ip6 daddr fe80::/10 udp sport 546 udp dport 547 accept
        icmp type {{ router-solicitation, router-advertisement }} accept
        meta l4proto ipv6-icmp accept
    }}
}}
"#
        );

        let mut child = Command::new("nft")
            .arg("-f")
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(ruleset.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(WgsplitError::Nftables("Failed to apply kill switch ruleset".into()));
        }
        info!("Kill switch enabled");
        Ok(())
    }

pub fn teardown(&self) -> Result<()> {
        let _ = Command::new("nft")
            .args(["delete", "table", "inet", "wgsplit"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        Ok(())
    }
}
