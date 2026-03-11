use std::{io, path::PathBuf};

use daktronics_allsport_5000::config::{AppConfig, DEFAULT_CONFIG_PATH};

#[tokio::main]
async fn main() -> io::Result<()> {
    let config_path = std::env::var("HAWKBOT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_CONFIG_PATH));
    let config = AppConfig::load_or_create(&config_path)?;

    if !config.web_enabled {
        println!("web interface is disabled in config");
        return Ok(());
    }

    daktronics_allsport_5000::web::run(config, config_path).await
}
