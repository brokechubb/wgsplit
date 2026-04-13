use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use log::{info, debug, warn};
use tokio::sync::RwLock;
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

pub struct DnsResolveLoop {
    resolver: DnsResolver,
    vpn_domains: Vec<String>,
    direct_domains: Vec<String>,
    current_ips: Arc<RwLock<HashMap<String, Vec<String>>>>,
    interval_secs: u64,
}

impl DnsResolveLoop {
    pub fn new(
        vpn_domains: Vec<String>,
        direct_domains: Vec<String>,
        interval_secs: u64,
    ) -> Self {
        Self {
            resolver: DnsResolver::new().unwrap_or_else(|_| {
                panic!("Failed to create DNS resolver");
            }),
            vpn_domains,
            direct_domains,
            current_ips: Arc::new(RwLock::new(HashMap::new())),
            interval_secs,
        }
    }

    pub fn current_ips(&self) -> Arc<RwLock<HashMap<String, Vec<String>>>> {
        self.current_ips.clone()
    }

    pub async fn run<F>(&self, on_ips_changed: F)
    where
        F: Fn(HashMap<String, Vec<String>>) + Send + Sync + 'static,
    {
        let on_changed = Arc::new(on_ips_changed);
        let mut interval = tokio::time::interval(Duration::from_secs(self.interval_secs));

        loop {
            interval.tick().await;
            let mut all_resolved = HashMap::new();

            let vpn_results = self.resolver.resolve_all(&self.vpn_domains).await;
            for host in vpn_results {
                if !host.ips.is_empty() {
                    all_resolved.insert(format!("vpn:{}", host.domain), host.ips.clone());
                }
            }

            let direct_results = self.resolver.resolve_all(&self.direct_domains).await;
            for host in direct_results {
                if !host.ips.is_empty() {
                    all_resolved.insert(format!("direct:{}", host.domain), host.ips.clone());
                }
            }

            let mut current = self.current_ips.write().await;
            if *current != all_resolved {
                info!("DNS resolved IPs changed, updating routes");
                let changed = all_resolved.clone();
                *current = all_resolved;
                drop(current);
                on_changed(changed);
            }
        }
    }
}
