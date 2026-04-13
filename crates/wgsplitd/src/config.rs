use std::path::PathBuf;
use wgsplit_common::types::AppSettings;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub socket_path: String,
    pub tunnel_dir: String,
    pub config_path: PathBuf,
    pub routing_table: u32,
    pub fwmark: u32,
    pub settings: AppSettings,
}

impl DaemonConfig {
    pub fn load() -> anyhow::Result<Self> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let config_dir = PathBuf::from(format!("{home}/.config/wgsplit"));
        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("settings.toml");
        let settings = if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)?;
            toml::from_str(&contents)?
        } else {
            AppSettings::default()
        };

        let tunnel_dir = config_dir.join("tunnels");
        std::fs::create_dir_all(&tunnel_dir)?;

        Ok(Self {
            socket_path: settings.daemon.socket_path.clone(),
            tunnel_dir: tunnel_dir.to_string_lossy().to_string(),
            config_path,
            routing_table: settings.daemon.routing_table,
            fwmark: settings.daemon.fwmark,
            settings,
        })
    }

    pub fn save_settings(&self, settings: &AppSettings) -> anyhow::Result<()> {
        let contents = toml::to_string_pretty(settings)?;
        std::fs::write(&self.config_path, contents)?;
        Ok(())
    }
}
