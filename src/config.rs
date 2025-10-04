use crate::models::{SavedRange, CONFIG_FILE};
use std::fs;

pub fn load_config() -> Vec<SavedRange> {
    if let Ok(contents) = fs::read_to_string(CONFIG_FILE) {
        serde_json::from_str(&contents).unwrap_or_default()
    } else {
        Vec::new()
    }
}

pub fn save_config(ranges: &[SavedRange]) {
    if let Ok(json) = serde_json::to_string_pretty(ranges) {
        let _ = fs::write(CONFIG_FILE, json);
    }
}
