use crate::models::SavedRange;
use serde::{Deserialize, Serialize};
use std::fs::{self, create_dir_all};
use std::path::PathBuf;

const CONFIG_DIR: &str = "asic-miner-scanner";
const CONFIG_FILE: &str = "scanner_config.json";

#[derive(Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub saved_ranges: Vec<SavedRange>,
    #[serde(default = "default_refresh_interval")]
    pub detail_refresh_interval_secs: u64,
    #[serde(default = "default_auto_scan_interval")]
    pub auto_scan_interval_secs: u64,
}

fn default_refresh_interval() -> u64 {
    10
}

fn default_auto_scan_interval() -> u64 {
    120
}

fn get_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        let config_dir = home.join(CONFIG_DIR);
        let _ = create_dir_all(&config_dir);
        config_dir.join(CONFIG_FILE)
    })
}

pub fn load_config() -> AppConfig {
    if let Some(config_path) = get_config_path() {
        if let Ok(contents) = fs::read_to_string(config_path) {
            // Try loading as new config format
            if let Ok(config) = serde_json::from_str::<AppConfig>(&contents) {
                return config;
            }
            // Fall back to old format (just Vec<SavedRange>)
            if let Ok(ranges) = serde_json::from_str::<Vec<SavedRange>>(&contents) {
                return AppConfig {
                    saved_ranges: ranges,
                    detail_refresh_interval_secs: default_refresh_interval(),
                    auto_scan_interval_secs: default_auto_scan_interval(),
                };
            }
        }
    }
    AppConfig::default()
}

pub fn save_config(config: &AppConfig) {
    if let Some(config_path) = get_config_path() {
        if let Ok(json) = serde_json::to_string_pretty(config) {
            let _ = fs::write(config_path, json);
        }
    }
}
