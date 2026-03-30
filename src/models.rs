use asic_rs_core::data::miner::MinerData;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default)]
pub struct MinerCapabilities {
    pub set_power_limit: bool,
    pub fan_config: bool,
    pub tuning_config: bool,
    pub scaling_config: bool,
    pub pools_config: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FanModeSelection {
    Auto,
    Manual,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MiningModeSelection {
    Low,
    Normal,
    High,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TuningTargetSelection {
    MiningMode,
    Power,
    Hashrate,
}

pub const HASHRATE_ALGO_OPTIONS: [&str; 5] = ["SHA256", "Scrypt", "X11", "Blake2S256", "Kadena"];
pub const EPIC_TUNING_ALGO_OPTIONS: [&str; 4] =
    ["VoltageOptimizer", "BoardTune", "PowerTune", "ChipTune"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PoolInput {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Default for PoolInput {
    fn default() -> Self {
        Self {
            url: String::new(),
            username: String::new(),
            password: "x".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MinerOptionSettings {
    pub apply_power_limit: bool,
    pub power_limit_watts: f64,
    pub apply_fan_config: bool,
    pub fan_mode: FanModeSelection,
    pub fan_speed_percent: u64,
    pub fan_target_temp_c: f64,
    pub fan_idle_speed_percent: u64,
    pub apply_tuning_config: bool,
    pub tuning_target: TuningTargetSelection,
    pub mining_mode: MiningModeSelection,
    pub tuning_power_watts: f64,
    pub tuning_hashrate_ths: f64,
    pub tuning_hashrate_algo: String,
    pub tuning_algorithm: String,
    pub apply_scaling_config: bool,
    pub scaling_step: u32,
    pub scaling_minimum: u32,
    pub scaling_shutdown: bool,
    pub scaling_shutdown_duration: f32,
    pub apply_pool_config: bool,
    pub pool_group_name: String,
    pub pool_group_quota: u32,
    pub pool_inputs: Vec<PoolInput>,
}

impl Default for MinerOptionSettings {
    fn default() -> Self {
        Self {
            apply_power_limit: false,
            power_limit_watts: 3200.0,
            apply_fan_config: false,
            fan_mode: FanModeSelection::Auto,
            fan_speed_percent: 70,
            fan_target_temp_c: 60.0,
            fan_idle_speed_percent: 35,
            apply_tuning_config: false,
            tuning_target: TuningTargetSelection::MiningMode,
            mining_mode: MiningModeSelection::Normal,
            tuning_power_watts: 3200.0,
            tuning_hashrate_ths: 100.0,
            tuning_hashrate_algo: "SHA256".to_string(),
            tuning_algorithm: String::new(),
            apply_scaling_config: false,
            scaling_step: 10,
            scaling_minimum: 50,
            scaling_shutdown: false,
            scaling_shutdown_duration: 15.0,
            apply_pool_config: false,
            pool_group_name: "Primary".to_string(),
            pool_group_quota: 100,
            pool_inputs: vec![PoolInput::default()],
        }
    }
}

impl MinerOptionSettings {
    pub fn has_any_enabled(&self) -> bool {
        self.apply_power_limit
            || self.apply_fan_config
            || self.apply_tuning_config
            || self.apply_scaling_config
            || self.apply_pool_config
    }

    pub fn pool_validation_message(&self) -> Option<String> {
        if !self.apply_pool_config {
            return None;
        }

        if self.pool_inputs.is_empty() {
            return Some("At least one pool entry is required".to_string());
        }

        for (idx, pool) in self.pool_inputs.iter().enumerate() {
            if pool.url.trim().is_empty() {
                return Some(format!("Pool {} URL is required", idx + 1));
            }
            if pool.username.trim().is_empty() {
                return Some(format!("Pool {} username is required", idx + 1));
            }
        }

        None
    }

    pub fn tuning_validation_message(&self) -> Option<String> {
        if !self.apply_tuning_config {
            return None;
        }

        match self.tuning_target {
            TuningTargetSelection::MiningMode => None,
            TuningTargetSelection::Power => {
                if self.tuning_power_watts <= 0.0 {
                    Some("Tuning power target must be greater than 0 W".to_string())
                } else {
                    None
                }
            }
            TuningTargetSelection::Hashrate => {
                if self.tuning_hashrate_ths <= 0.0 {
                    return Some("Tuning hashrate target must be greater than 0 TH/s".to_string());
                }

                if self.tuning_hashrate_algo.trim().is_empty() {
                    return Some("Tuning hashrate algorithm is required".to_string());
                }

                None
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MinerInfo {
    pub ip: String,
    pub hostname: String,
    pub model: String,
    pub firmware_version: String,
    pub control_board: String,
    pub active_boards: String,
    pub hashrate: String,
    pub wattage: String,
    pub efficiency: String, // W/TH
    pub temperature: String,
    pub fan_speed: String,
    pub pool: String,
    pub worker: String,
    pub light_flashing: bool,         // Fault light status
    pub full_data: Option<MinerData>, // Store complete MinerData for detail view
    pub hashrate_th: Option<f64>,
    pub wattage_w: Option<f64>,
    pub efficiency_w_th: Option<f64>,
    pub temperature_c: Option<f64>,
    pub fan_rpm: Option<f64>,
    pub active_boards_count: Option<usize>,
    pub total_boards_count: Option<usize>,
    pub capabilities: MinerCapabilities,
}

impl MinerInfo {
    pub fn is_epic_firmware(&self) -> bool {
        let from_version = {
            let fw = self.firmware_version.to_ascii_lowercase();
            fw.contains("epic") || fw.contains("powerplay")
        };

        let from_data = self
            .full_data
            .as_ref()
            .map(|data| data.device_info.firmware.to_ascii_lowercase())
            .map(|fw| fw.contains("epic") || fw.contains("powerplay"))
            .unwrap_or(false);

        from_version || from_data
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedRange {
    pub name: String,
    pub range: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct HashratePoint {
    pub timestamp: f64,
    pub hashrate: f64,
}

pub struct ScanProgress {
    pub scanning: bool,
    pub current_ip: String,
    pub total_ips: usize,
    pub scanned_ips: usize,
    pub found_miners: usize,
    pub scan_start_time: Option<std::time::Instant>,
    pub total_ranges: usize,
    pub scanned_ranges: usize,
    pub scan_duration_secs: u64,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortColumn {
    Ip,
    Hostname,
    Model,
    Firmware,
    ControlBoard,
    ActiveBoards,
    Hashrate,
    Wattage,
    Efficiency,
    Temperature,
    FanSpeed,
    Pool,
    Worker,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

pub const MAX_HISTORY_POINTS: usize = 288; // 24 hours at 5-min intervals

// Type alias for metrics history: (timestamp, total_hashrate, power, board_hashrates, avg_temp, board_temps)
pub type MetricsHistory = VecDeque<(f64, f64, f64, Vec<f64>, f64, Vec<f64>)>;

#[derive(Clone, Debug)]
pub struct RecordingState {
    pub file_path: String,
    pub start_time: std::time::Instant,
    pub row_count: usize,
    pub is_recording: bool,
}
