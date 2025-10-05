use asic_rs::data::miner::MinerData;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct MinerInfo {
    pub ip: String,
    pub hostname: String,
    pub model: String,
    pub firmware_version: String,
    pub control_board: String,
    pub hashrate: String,
    pub wattage: String,
    pub efficiency: String, // W/TH
    pub temperature: String,
    pub fan_speed: String,
    pub pool: String,
    pub worker: String,
    pub light_flashing: bool,         // Fault light status
    pub full_data: Option<MinerData>, // Store complete MinerData for detail view
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
pub type MetricsHistory = Vec<(f64, f64, f64, Vec<f64>, f64, Vec<f64>)>;

#[derive(Clone, Debug)]
pub struct RecordingState {
    pub file_path: String,
    pub start_time: std::time::Instant,
    pub row_count: usize,
    pub is_recording: bool,
}
