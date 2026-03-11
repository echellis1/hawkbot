use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

use crate::controllers::{ActiveSport, ControllerType};

pub const DEFAULT_CONFIG_PATH: &str = "hawkbot-web-config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub web_enabled: bool,
    pub web_bind: String,
    pub controller_type: ControllerType,
    pub active_sport: ActiveSport,
    pub public_status_uuid: String,
    pub admin_username: String,
    pub admin_password_hash: String,
    pub serial_device: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            web_enabled: true,
            web_bind: "0.0.0.0:8080".to_string(),
            controller_type: ControllerType::Daktronics,
            active_sport: ActiveSport::Basketball,
            public_status_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
            admin_username: "admin".to_string(),
            admin_password_hash: "admin".to_string(),
            serial_device: default_serial_device(),
        }
    }
}

#[cfg(unix)]
fn default_serial_device() -> String {
    "/dev/ttyUSB0".to_string()
}
#[cfg(windows)]
fn default_serial_device() -> String {
    "COM1".to_string()
}

impl AppConfig {
    pub fn load_or_create(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            let raw = fs::read_to_string(path)?;
            serde_json::from_str(&raw).map_err(io::Error::other)
        } else {
            let cfg = Self::default();
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        fs::write(
            path,
            serde_json::to_string_pretty(self).map_err(io::Error::other)?,
        )
    }
}
