use std::sync::Arc;
use wgsplitd::{DaemonConfig, AppState};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    let config = DaemonConfig::load()?;

    let rt = tokio::runtime::Runtime::new()?;
    let state = Arc::new(AppState::new(config.clone())?);

    {
        let settings = &config.settings.split_tunneling;
        if settings.enabled {
            if let Err(e) = state.split_tunnel.apply_settings(settings) {
                log::error!("Failed to apply split tunneling settings: {e}");
            }
        }
    }

    rt.block_on(async {
        wgsplitd::run_with_state(state).await
    })
}
