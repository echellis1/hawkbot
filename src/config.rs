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

    #[serde(default)]
    pub mqtt_enabled: bool,
    #[serde(default = "default_mqtt_host")]
    pub mqtt_host: String,
    #[serde(default = "default_mqtt_port")]
    pub mqtt_port: u16,
    #[serde(default = "default_mqtt_topic")]
    pub mqtt_topic: String,
    #[serde(default = "default_mqtt_retain")]
    pub mqtt_retain: bool,
    #[serde(default = "default_mqtt_client_id")]
    pub mqtt_client_id: String,
    #[serde(default)]
    pub mqtt_username: Option<String>,
    #[serde(default)]
    pub mqtt_password: Option<String>,
    #[serde(default)]
    pub mqtt_use_tls: bool,
    #[serde(default)]
    pub mqtt_ca_file: Option<String>,
    #[serde(default)]
    pub mqtt_cert_file: Option<String>,
    #[serde(default)]
    pub mqtt_key_file: Option<String>,
    #[serde(default = "default_publish_interval_ms")]
    pub publish_interval_ms: u64,
}

fn default_mqtt_host() -> String {
    "mqtt.scorenode.org".to_string()
}
fn default_mqtt_port() -> u16 {
    1883
}
fn default_mqtt_topic() -> String {
    "scoreboard/status".to_string()
}
fn default_mqtt_retain() -> bool {
    true
}
fn default_mqtt_client_id() -> String {
    "hawkbot".to_string()
}
fn default_publish_interval_ms() -> u64 {
    100
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
            mqtt_enabled: false,
            mqtt_host: default_mqtt_host(),
            mqtt_port: default_mqtt_port(),
            mqtt_topic: default_mqtt_topic(),
            mqtt_retain: default_mqtt_retain(),
            mqtt_client_id: default_mqtt_client_id(),
            mqtt_username: None,
            mqtt_password: None,
            mqtt_use_tls: false,
            mqtt_ca_file: None,
            mqtt_cert_file: None,
            mqtt_key_file: None,
            publish_interval_ms: default_publish_interval_ms(),
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
