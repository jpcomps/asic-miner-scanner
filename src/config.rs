use crate::models::SavedRange;
use std::fs::{self, create_dir_all};
use std::path::PathBuf;

const CONFIG_DIR: &str = "asic-miner-scanner";
const CONFIG_FILE: &str = "scanner_config.json";

fn get_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        let config_dir = home.join(CONFIG_DIR);
        let _ = create_dir_all(&config_dir);
        config_dir.join(CONFIG_FILE)
    })
}

pub fn load_config() -> Vec<SavedRange> {
    if let Some(config_path) = get_config_path() {
        if let Ok(contents) = fs::read_to_string(config_path) {
            return serde_json::from_str(&contents).unwrap_or_default();
        }
    }
    Vec::new()
}

pub fn save_config(ranges: &[SavedRange]) {
    if let Some(config_path) = get_config_path() {
        if let Ok(json) = serde_json::to_string_pretty(ranges) {
            let _ = fs::write(config_path, json);
        }
    }
}
