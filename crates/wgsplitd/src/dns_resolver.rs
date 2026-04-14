use std::time::Duration;
use log::{debug, warn};
use hickory_resolver::TokioAsyncResolver;
use hickory_resolver::config::*;
use wgsplit_common::error::{Result, WgsplitError};

#[derive(Debug, Clone)]
pub struct ResolvedHost {
    pub domain: String,
    pub ips: Vec<String>,
}

pub struct DnsResolver {
    resolver: TokioAsyncResolver,
}

impl DnsResolver {
    pub fn new() -> Result<Self> {
        let config = ResolverConfig::default();
        let opts = ResolverOpts::default();
        let resolver = TokioAsyncResolver::tokio(config, opts);
        Ok(Self { resolver })
    }

    pub async fn resolve(&self, domain: &str) -> Result<Vec<String>> {
        if let Ok(addr) = domain.parse::<std::net::IpAddr>() {
            debug!("{domain} is already an IP, skipping DNS");
            return Ok(vec![addr.to_string()]);
        }

        let lookup = self.resolver.lookup_ip(domain).await
            .map_err(|e| WgsplitError::Dns(format!("Failed to resolve {domain}: {e}")))?;

        let ips: Vec<String> = lookup.iter()
            .map(|ip| ip.to_string())
            .collect();

        debug!("Resolved {domain} → {:?}", ips);
        Ok(ips)
    }

    pub async fn resolve_all(&self, domains: &[String]) -> Vec<ResolvedHost> {
        let mut results = Vec::new();
        for domain in domains {
            match self.resolve(domain).await {
                Ok(ips) => {
                    if ips.is_empty() {
                        warn!("No IPs found for {domain}");
                    }
                    results.push(ResolvedHost {
                        domain: domain.clone(),
                        ips,
                    });
                }
                Err(e) => {
                    warn!("Failed to resolve {domain}: {e}, retrying...");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    match self.resolve(domain).await {
                        Ok(ips) => {
                            results.push(ResolvedHost {
                                domain: domain.clone(),
                                ips,
                            });
                        }
                        Err(e) => {
                            warn!("Retry failed for {domain}: {e}");
                            results.push(ResolvedHost {
                                domain: domain.clone(),
                                ips: Vec::new(),
                            });
                        }
                    }
                }
            }
        }
        results
    }
}
