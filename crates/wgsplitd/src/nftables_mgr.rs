use std::process::Command;
use log::{info, debug};
use wgsplit_common::error::{Result, WgsplitError};

pub struct NftablesManager {
    fwmark: u32,
}

impl NftablesManager {
    pub fn new(fwmark: u32) -> Self {
        Self { fwmark }
    }

    pub fn setup_base(&self, vpn_iface: &str, default_iface: &str) -> Result<()> {
        let fwmark_hex = format!("0x{:x}", self.fwmark);
        let table_name = "wgsplit";

        let ruleset = format!(
            r#"
table inet {table_name} {{
    chain mangle {{
        type filter hook output priority mangle; policy accept;
        meta cgroup "wgsplit-vpn" mark set {fwmark_hex}
    }}

    chain output {{
        type filter hook output priority filter; policy accept;
        oifname "{vpn_iface}" accept
        oifname "lo" accept
        ct state established,related accept
    }}

    chain killswitch {{
        type filter hook output priority filter + 1; policy drop;
        oifname "{vpn_iface}" accept
        oifname "lo" accept
        oifname "{default_iface}" udp dport 53 accept
        ct state established,related accept
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
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(ruleset.as_bytes())?;
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(WgsplitError::Nftables("Failed to apply nftables ruleset".into()));
        }

        info!("nftables base rules applied (table={table_name}, fwmark={fwmark_hex})");
        Ok(())
    }

    pub fn add_domain_ips(&self, ips: &[String]) -> Result<()> {
        for ip in ips {
            debug!("Adding nftables allow rule for {ip}");
            let spec = format!("ip daddr {ip} accept");
            let _ = Command::new("nft")
                .args(["add", "rule", "inet", "wgsplit", "output", &spec])
                .status();
        }
        Ok(())
    }

    pub fn remove_domain_ips(&self, _ips: &[String]) -> Result<()> {
        let _ = self.flush_ruleset();
        Ok(())
    }

    pub fn enable_killswitch(&self, vpn_iface: &str, _default_iface: &str, dns_ips: &[String]) -> Result<()> {
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
        type filter hook output priority mangle; policy accept;
        meta cgroup "wgsplit-vpn" mark set {fwmark_hex}
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
        if let Some(ref mut stdin) = child.stdin {
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

    fn flush_ruleset(&self) -> Result<()> {
        let _ = Command::new("nft")
            .args(["delete", "table", "inet", "wgsplit"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        Ok(())
    }
}
