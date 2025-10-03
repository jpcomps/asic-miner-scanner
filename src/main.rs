use asic_rs::data::miner::MinerData;
use asic_rs::MinerFactory;
use eframe::egui;
use egui::{Color32, FontId, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("ASIC-RS Miner Scanner"),
        ..Default::default()
    };

    eframe::run_native(
        "ASIC-RS Miner Scanner",
        options,
        Box::new(|_cc| Ok(Box::new(MinerScannerApp::new()))),
    )
}

#[derive(Clone, Debug)]
struct MinerInfo {
    ip: String,
    hostname: String,
    model: String,
    firmware_version: String,
    control_board: String,
    hashrate: String,
    wattage: String,
    efficiency: String, // W/TH
    temperature: String,
    fan_speed: String,
    pool: String,
    light_flashing: bool,         // Fault light status
    full_data: Option<MinerData>, // Store complete MinerData for detail view
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SavedRange {
    name: String,
    range: String,
}

#[derive(Clone, Debug)]
struct HashratePoint {
    timestamp: f64,
    hashrate: f64,
}

const CONFIG_FILE: &str = "scanner_config.json";
const MAX_HISTORY_POINTS: usize = 288; // 24 hours at 5-min intervals

struct ScanProgress {
    scanning: bool,
    current_ip: String,
    total_ips: usize,
    scanned_ips: usize,
    found_miners: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
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
}

#[derive(Clone, Copy, PartialEq)]
enum SortDirection {
    Ascending,
    Descending,
}

struct MinerScannerApp {
    ip_range_start: String,
    ip_range_end: String,
    miners: Arc<Mutex<Vec<MinerInfo>>>,
    scan_progress: Arc<Mutex<ScanProgress>>,
    error_message: String,
    sort_column: Option<SortColumn>,
    sort_direction: SortDirection,
    saved_ranges: Vec<SavedRange>,
    new_range_name: String,
    auto_scan_enabled: bool,
    auto_scan_interval_secs: u64,
    last_scan_time: Option<Instant>,
    hashrate_history: Arc<Mutex<HashMap<String, Vec<HashratePoint>>>>, // IP -> history
    selected_miners: std::collections::HashSet<String>,                // Selected IPs
    detail_view_miners: Vec<MinerInfo>, // Miners being viewed in detail modals
    detail_refresh_times: HashMap<String, Instant>, // IP -> last refresh time
    detail_metrics_history: HashMap<String, Vec<(f64, f64, f64, Vec<f64>, f64, Vec<f64>)>>, // IP -> Vec<(timestamp, total_hashrate, power, board_hashrates, avg_temp, board_temps)>
}

impl MinerScannerApp {
    fn new() -> Self {
        let saved_ranges = Self::load_config();

        Self {
            ip_range_start: "10.0.81.0".to_string(),
            ip_range_end: "10.0.81.255".to_string(),
            miners: Arc::new(Mutex::new(Vec::new())),
            scan_progress: Arc::new(Mutex::new(ScanProgress {
                scanning: false,
                current_ip: String::new(),
                total_ips: 0,
                scanned_ips: 0,
                found_miners: 0,
            })),
            error_message: String::new(),
            sort_column: None,
            sort_direction: SortDirection::Ascending,
            saved_ranges,
            new_range_name: String::new(),
            auto_scan_enabled: true,
            auto_scan_interval_secs: 120, // 2 minutes default
            last_scan_time: None,
            hashrate_history: Arc::new(Mutex::new(HashMap::new())),
            selected_miners: std::collections::HashSet::new(),
            detail_view_miners: Vec::new(),
            detail_refresh_times: HashMap::new(),
            detail_metrics_history: HashMap::new(),
        }
    }

    fn load_config() -> Vec<SavedRange> {
        if let Ok(contents) = fs::read_to_string(CONFIG_FILE) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn save_config(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.saved_ranges) {
            let _ = fs::write(CONFIG_FILE, json);
        }
    }

    fn sort_miners(&mut self, column: SortColumn) {
        // Toggle direction if clicking same column
        if self.sort_column == Some(column) {
            self.sort_direction = match self.sort_direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.sort_column = Some(column);
            self.sort_direction = SortDirection::Ascending;
        }

        let mut miners = self.miners.lock().unwrap();
        let direction = self.sort_direction;

        miners.sort_by(|a, b| {
            let cmp = match column {
                SortColumn::Ip => a.ip.cmp(&b.ip),
                SortColumn::Hostname => a.hostname.cmp(&b.hostname),
                SortColumn::Model => a.model.cmp(&b.model),
                SortColumn::Firmware => a.firmware_version.cmp(&b.firmware_version),
                SortColumn::ControlBoard => a.control_board.cmp(&b.control_board),
                SortColumn::Hashrate => a.hashrate.cmp(&b.hashrate),
                SortColumn::Wattage => a.wattage.cmp(&b.wattage),
                SortColumn::Efficiency => a.efficiency.cmp(&b.efficiency),
                SortColumn::Temperature => a.temperature.cmp(&b.temperature),
                SortColumn::FanSpeed => a.fan_speed.cmp(&b.fan_speed),
                SortColumn::Pool => a.pool.cmp(&b.pool),
            };

            match direction {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }

    fn parse_ip_range(&self) -> Result<String, String> {
        let start_parts: Vec<&str> = self.ip_range_start.split('.').collect();
        let end_parts: Vec<&str> = self.ip_range_end.split('.').collect();

        if start_parts.len() != 4 || end_parts.len() != 4 {
            return Err("Invalid IP address format".to_string());
        }

        // Verify first three octets match
        if start_parts[0] != end_parts[0]
            || start_parts[1] != end_parts[1]
            || start_parts[2] != end_parts[2]
        {
            return Err(
                "IP ranges must be in the same subnet (first 3 octets must match)".to_string(),
            );
        }

        let start_last: u8 = start_parts[3]
            .parse()
            .map_err(|_| "Invalid start IP address".to_string())?;
        let end_last: u8 = end_parts[3]
            .parse()
            .map_err(|_| "Invalid end IP address".to_string())?;

        if start_last > end_last {
            return Err("Start IP must be less than or equal to end IP".to_string());
        }

        // Format: "192.168.1.1-254" for asic-rs
        let range = format!(
            "{}.{}.{}.{}-{}",
            start_parts[0], start_parts[1], start_parts[2], start_last, end_last
        );

        Ok(range)
    }

    fn calculate_total_ips(&self, range: &str) -> usize {
        // Parse "192.168.1.1-254" format
        if let Some(dash_pos) = range.rfind('-') {
            if let Some(last_dot_pos) = range[..dash_pos].rfind('.') {
                if let (Ok(start), Ok(end)) = (
                    range[last_dot_pos + 1..dash_pos].parse::<u8>(),
                    range[dash_pos + 1..].parse::<u8>(),
                ) {
                    return (end - start + 1) as usize;
                }
            }
        }
        0
    }

    fn add_saved_range(&mut self) {
        if !self.new_range_name.trim().is_empty() {
            let range = match self.parse_ip_range() {
                Ok(range) => range,
                Err(e) => {
                    self.error_message = e;
                    return;
                }
            };

            self.saved_ranges.push(SavedRange {
                name: self.new_range_name.trim().to_string(),
                range,
            });
            self.new_range_name.clear();
            self.save_config();
        }
    }

    fn remove_saved_range(&mut self, index: usize) {
        if index < self.saved_ranges.len() {
            self.saved_ranges.remove(index);
            self.save_config();
        }
    }

    fn load_saved_range(&mut self, range: &SavedRange) {
        // Parse the range back into start and end IPs
        if let Some(dash_pos) = range.range.rfind('-') {
            let start = range.range[..dash_pos].to_string();
            let end_octet = &range.range[dash_pos + 1..];
            if let Some(last_dot_pos) = start.rfind('.') {
                let end = format!("{}.{}", &start[..last_dot_pos], end_octet);
                self.ip_range_start = start;
                self.ip_range_end = end;
            }
        }
    }

    fn scan_all_saved_ranges(&mut self) {
        if self.saved_ranges.is_empty() {
            self.error_message = "No saved ranges to scan".to_string();
            return;
        }

        // Build list of all ranges to scan
        let ranges: Vec<String> = self.saved_ranges.iter().map(|r| r.range.clone()).collect();

        let total_ips: usize = ranges.iter().map(|r| self.calculate_total_ips(r)).sum();

        // Don't clear previous results - update will happen when new data arrives

        // Update scan progress
        {
            let mut progress = self.scan_progress.lock().unwrap();
            progress.scanning = true;
            progress.total_ips = total_ips;
            progress.scanned_ips = 0;
            progress.found_miners = 0;
            progress.current_ip.clear();
        }

        let miners = Arc::clone(&self.miners);
        let scan_progress = Arc::clone(&self.scan_progress);
        let hashrate_history = Arc::clone(&self.hashrate_history);

        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();

            rt.block_on(async {
                let mut new_miners: HashMap<String, MinerInfo> = HashMap::new();

                for range in ranges {
                    match MinerFactory::new()
                        .with_adaptive_concurrency()
                        .with_port_check(true)
                        .scan_by_range(&range)
                        .await
                    {
                        Ok(discovered_miners) => {
                            for miner in discovered_miners {
                                let ip = miner.get_ip().to_string();

                                {
                                    let mut progress = scan_progress.lock().unwrap();
                                    progress.current_ip = ip.clone();
                                    progress.scanned_ips += 1;
                                }

                                let data = miner.get_data().await;

                                // Extract hashrate value for efficiency calculation
                                let hashrate_th = data.hashrate.as_ref().map(|hr| {
                                    hr.clone()
                                        .as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash)
                                });

                                // Extract wattage (power) and efficiency
                                let wattage_str = if let Some(wattage) = data.wattage {
                                    format!("{:.0} W", wattage.as_watts())
                                } else {
                                    "N/A".to_string()
                                };

                                let efficiency_str = if let Some(efficiency) = data.efficiency {
                                    format!("{efficiency:.1}")
                                } else {
                                    "N/A".to_string()
                                };

                                let miner_info = MinerInfo {
                                    ip: ip.clone(),
                                    hostname: data
                                        .hostname
                                        .clone()
                                        .unwrap_or_else(|| "N/A".to_string()),
                                    model: data.device_info.model.to_string(),
                                    firmware_version: data
                                        .firmware_version
                                        .clone()
                                        .unwrap_or_else(|| "N/A".to_string()),
                                    control_board: data
                                        .control_board_version
                                        .as_ref()
                                        .map(|cb| format!("{cb:?}"))
                                        .unwrap_or_else(|| "N/A".to_string()),
                                    hashrate: match hashrate_th {
                                        Some(hr) => format!("{hr:.2}"),
                                        None => "N/A".to_string(),
                                    },
                                    wattage: wattage_str,
                                    efficiency: efficiency_str,
                                    temperature: {
                                        if let Some(temp) = data.average_temperature {
                                            format!("{:.1}°C", temp.as_celsius())
                                        } else {
                                            "N/A".to_string()
                                        }
                                    },
                                    fan_speed: {
                                        if !data.fans.is_empty() {
                                            if let Some(rpm) = data.fans[0].rpm {
                                                let rpm_value = rpm.as_radians_per_second() * 60.0
                                                    / (2.0 * std::f64::consts::PI);
                                                format!("{rpm_value:.0} RPM")
                                            } else {
                                                "N/A".to_string()
                                            }
                                        } else {
                                            "N/A".to_string()
                                        }
                                    },
                                    pool: {
                                        if !data.pools.is_empty() {
                                            if let Some(url) = &data.pools[0].url {
                                                url.to_string()
                                            } else {
                                                "N/A".to_string()
                                            }
                                        } else {
                                            "N/A".to_string()
                                        }
                                    },
                                    light_flashing: data.light_flashing.unwrap_or(false),
                                    full_data: Some(data.clone()),
                                };

                                // Record hashrate history
                                {
                                    if let Some(hashrate_val) =
                                        miner_info.hashrate.split_whitespace().next()
                                    {
                                        if let Ok(hashrate) = hashrate_val.parse::<f64>() {
                                            let timestamp = SystemTime::now()
                                                .duration_since(UNIX_EPOCH)
                                                .unwrap()
                                                .as_secs_f64();

                                            let mut history_map = hashrate_history.lock().unwrap();
                                            let history = history_map
                                                .entry(miner_info.ip.clone())
                                                .or_default();
                                            history.push(HashratePoint {
                                                timestamp,
                                                hashrate,
                                            });

                                            // Keep only last MAX_HISTORY_POINTS
                                            if history.len() > MAX_HISTORY_POINTS {
                                                history
                                                    .drain(0..history.len() - MAX_HISTORY_POINTS);
                                            }
                                        }
                                    }
                                }

                                // Use IP as key to deduplicate
                                new_miners.insert(miner_info.ip.clone(), miner_info);

                                let mut progress = scan_progress.lock().unwrap();
                                progress.found_miners = new_miners.len();
                            }
                        }
                        Err(e) => {
                            eprintln!("Scan error for range {range}: {e:?}");
                        }
                    }
                }

                // Atomically replace miners list with new data (convert HashMap to Vec)
                {
                    let mut miners_lock = miners.lock().unwrap();
                    *miners_lock = new_miners.into_values().collect();
                }

                {
                    let mut progress = scan_progress.lock().unwrap();
                    progress.scanning = false;
                    progress.current_ip.clear();
                }
            });
        });

        self.last_scan_time = Some(Instant::now());
    }

    fn draw_scan_and_ranges_card(&mut self, ui: &mut egui::Ui) {
        // Get progress info early, then drop the lock
        let (is_scanning, scanned_ips, total_ips) = {
            let progress = self.scan_progress.lock().unwrap();
            (progress.scanning, progress.scanned_ips, progress.total_ips)
        };

        egui::Frame::new()
            .fill(Color32::from_rgb(28, 28, 28))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
            .corner_radius(4.0)
            .inner_margin(15.0)
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("🔍 SCAN CONTROL")
                            .size(13.0)
                            .color(Color32::from_rgb(240, 240, 240))
                            .strong()
                            .monospace(),
                    );

                    ui.add_space(10.0);

                    // Auto-scan checkbox and interval
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.auto_scan_enabled, "")
                            .on_hover_text("Enable automatic scanning");
                        ui.label(
                            egui::RichText::new("AUTO-SCAN")
                                .size(11.0)
                                .color(Color32::from_rgb(160, 160, 160))
                                .monospace(),
                        );

                        ui.add_space(20.0);

                        ui.label(
                            egui::RichText::new("INTERVAL:")
                                .size(11.0)
                                .color(Color32::from_rgb(160, 160, 160))
                                .monospace(),
                        );

                        ui.add_space(5.0);

                        let interval_mins = (self.auto_scan_interval_secs / 60) as i32;
                        let mut temp_interval = interval_mins;
                        if ui
                            .add(
                                egui::DragValue::new(&mut temp_interval)
                                    .suffix(" min")
                                    .speed(1),
                            )
                            .changed()
                        {
                            self.auto_scan_interval_secs = (temp_interval.max(1) * 60) as u64;
                        }

                        ui.add_space(20.0);

                        // Scan button
                        let scan_btn = egui::Button::new(
                            egui::RichText::new("⟳ SCAN ALL")
                                .size(12.0)
                                .color(Color32::WHITE)
                                .monospace(),
                        )
                        .fill(Color32::from_rgb(255, 87, 51))
                        .corner_radius(4.0)
                        .min_size(Vec2::new(120.0, 28.0));

                        if ui
                            .add_enabled(!self.saved_ranges.is_empty(), scan_btn)
                            .clicked()
                        {
                            self.scan_all_saved_ranges();
                        }

                        // Show last scan time
                        if let Some(last_scan) = self.last_scan_time {
                            let elapsed = last_scan.elapsed().as_secs();
                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new(format!("Last scan: {elapsed}s ago"))
                                    .size(10.0)
                                    .color(Color32::from_rgb(120, 120, 120))
                                    .monospace(),
                            );
                        }
                    });

                    ui.add_space(10.0);

                    // Always show scan progress bar
                    let _progress_fraction = if is_scanning && total_ips > 0 {
                        scanned_ips as f32 / total_ips as f32
                    } else {
                        0.0
                    };

                    let status_text = if is_scanning {
                        format!("⏳ Scanning: {scanned_ips}/{total_ips}")
                    } else {
                        "Ready to scan".to_string()
                    };

                    ui.label(
                        egui::RichText::new(status_text)
                            .size(10.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                    );

                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(15.0);

                    // IP Range Configuration
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("START IP:")
                                .size(11.0)
                                .color(Color32::from_rgb(160, 160, 160))
                                .monospace(),
                        );

                        ui.add_space(10.0);

                        let text_edit = egui::TextEdit::singleline(&mut self.ip_range_start)
                            .font(FontId::monospace(12.0))
                            .desired_width(150.0);
                        ui.add(text_edit);

                        ui.add_space(20.0);

                        ui.label(
                            egui::RichText::new("END IP:")
                                .size(11.0)
                                .color(Color32::from_rgb(160, 160, 160))
                                .monospace(),
                        );

                        ui.add_space(10.0);

                        let text_edit = egui::TextEdit::singleline(&mut self.ip_range_end)
                            .font(FontId::monospace(12.0))
                            .desired_width(150.0);
                        ui.add(text_edit);
                    });

                    ui.add_space(10.0);

                    // Add new range
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("NAME:")
                                .size(11.0)
                                .color(Color32::from_rgb(160, 160, 160))
                                .monospace(),
                        );

                        ui.add_space(10.0);

                        let text_edit = egui::TextEdit::singleline(&mut self.new_range_name)
                            .font(FontId::monospace(12.0))
                            .desired_width(150.0)
                            .hint_text("e.g. Main Site");
                        ui.add(text_edit);

                        ui.add_space(10.0);

                        if ui
                            .button(
                                egui::RichText::new("💾 Save Range")
                                    .size(11.0)
                                    .color(Color32::WHITE)
                                    .monospace(),
                            )
                            .clicked()
                        {
                            self.add_saved_range();
                        }
                    });

                    ui.add_space(15.0);
                    ui.separator();
                    ui.add_space(15.0);

                    // Show saved ranges
                    let mut range_to_remove: Option<usize> = None;
                    let mut range_to_load: Option<SavedRange> = None;

                    if !self.saved_ranges.is_empty() {
                        ui.label(
                            egui::RichText::new("SAVED RANGES:")
                                .size(11.0)
                                .color(Color32::from_rgb(180, 180, 180))
                                .monospace(),
                        );
                        ui.add_space(5.0);
                    }

                    for (idx, range) in self.saved_ranges.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&range.name)
                                    .size(11.0)
                                    .color(Color32::from_rgb(200, 200, 200))
                                    .monospace(),
                            );
                            ui.label(
                                egui::RichText::new(format!("({})", &range.range))
                                    .size(10.0)
                                    .color(Color32::from_rgb(150, 150, 150))
                                    .monospace(),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(
                                            egui::RichText::new("✕")
                                                .color(Color32::from_rgb(255, 100, 100)),
                                        )
                                        .clicked()
                                    {
                                        range_to_remove = Some(idx);
                                    }

                                    if ui
                                        .button(
                                            egui::RichText::new("Load")
                                                .color(Color32::from_rgb(100, 200, 255)),
                                        )
                                        .clicked()
                                    {
                                        range_to_load = Some(range.clone());
                                    }
                                },
                            );
                        });
                    }

                    // Handle removals and loads
                    if let Some(idx) = range_to_remove {
                        self.remove_saved_range(idx);
                    }
                    if let Some(range) = range_to_load {
                        self.load_saved_range(&range);
                    }
                });
            });
    }

    fn draw_stats_card(&self, ui: &mut egui::Ui) {
        let miners = self.miners.lock().unwrap();

        let miner_count = miners.len();

        // Parse hashrate - it's stored as just the number without units
        let hashrates: Vec<f64> = miners
            .iter()
            .filter_map(|m| {
                // Try to parse as-is first, or split and take first part
                m.hashrate.split_whitespace().next()?.parse::<f64>().ok()
            })
            .collect();

        let total_hashrate: f64 = hashrates.iter().sum();

        let avg_hashrate = if !hashrates.is_empty() {
            total_hashrate / hashrates.len() as f64
        } else {
            0.0
        };

        let temps: Vec<f64> = miners
            .iter()
            .filter_map(|m| m.temperature.trim_end_matches("°C").parse::<f64>().ok())
            .collect();

        let avg_temp = if !temps.is_empty() {
            temps.iter().sum::<f64>() / temps.len() as f64
        } else {
            0.0
        };

        // Parse efficiency - remove any units and parse, filter out NaN values
        let efficiencies: Vec<f64> = miners
            .iter()
            .filter_map(|m| {
                let val = m
                    .efficiency
                    .split_whitespace()
                    .next()?
                    .parse::<f64>()
                    .ok()?;
                if val.is_finite() {
                    Some(val)
                } else {
                    None
                }
            })
            .collect();

        let avg_efficiency = if !efficiencies.is_empty() {
            efficiencies.iter().sum::<f64>() / efficiencies.len() as f64
        } else {
            0.0
        };

        egui::Frame::new()
            .fill(Color32::from_rgb(255, 87, 51))
            .corner_radius(4.0)
            .inner_margin(15.0)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("⚡ FLEET OVERVIEW")
                            .size(13.0)
                            .color(Color32::WHITE)
                            .strong()
                            .monospace(),
                    );

                    ui.add_space(20.0);

                    // Miner count
                    ui.label(
                        egui::RichText::new(format!("{miner_count}"))
                            .size(32.0)
                            .color(Color32::WHITE)
                            .strong()
                            .monospace(),
                    );
                    ui.label(
                        egui::RichText::new("MINERS")
                            .size(10.0)
                            .color(Color32::from_rgb(255, 200, 180))
                            .monospace(),
                    );

                    ui.add_space(20.0);

                    // Total hashrate
                    ui.label(
                        egui::RichText::new(format!("{total_hashrate:.2}"))
                            .size(32.0)
                            .color(Color32::WHITE)
                            .strong()
                            .monospace(),
                    );
                    ui.label(
                        egui::RichText::new("TOTAL TH/s")
                            .size(10.0)
                            .color(Color32::from_rgb(255, 200, 180))
                            .monospace(),
                    );

                    ui.add_space(15.0);

                    // Averages - compact display
                    ui.label(
                        egui::RichText::new(format!(
                            "AVG: {avg_hashrate:.2} TH/s  •  {avg_efficiency:.1} W/TH  •  {avg_temp:.1}°C"
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(255, 200, 180))
                        .monospace(),
                    );
                });
            });
    }

    fn draw_miner_detail_modal(&mut self, ctx: &egui::Context) {
        let mut miners_to_close = Vec::new();

        for (idx, detail_miner) in self.detail_view_miners.iter().enumerate() {
            let mut is_open = true;

            // Get the latest data from the main miners list
            let miners_list = self.miners.lock().unwrap();
            let current_miner = miners_list.iter().find(|m| m.ip == detail_miner.ip);

            if let Some(miner) = current_miner {
                egui::Window::new(format!("🔍 Miner Details - {} - {}", miner.ip, miner.model))
                    .id(egui::Id::new(format!("detail_modal_{}", miner.ip)))
                    .default_width(1200.0)
                    .default_height(700.0)
                    .resizable(true)
                    .collapsible(true)
                    .open(&mut is_open)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                        // Left column - Basic Information (1/3 width)
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width() * 0.33, 700.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    if let Some(data) = &miner.full_data {
                            ui.heading("Basic Information");
                            ui.separator();
                            ui.add_space(5.0);

                            egui::Grid::new("basic_info_grid")
                                .num_columns(2)
                                .spacing([40.0, 8.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("IP Address:").strong());
                                    ui.label(data.ip.to_string());
                                    ui.end_row();

                                    ui.label(egui::RichText::new("MAC Address:").strong());
                                    ui.label(data.mac.map(|m| m.to_string()).unwrap_or_else(|| "N/A".to_string()));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Hostname:").strong());
                                    ui.label(data.hostname.as_ref().unwrap_or(&"N/A".to_string()));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Model:").strong());
                                    ui.label(format!("{:?}", data.device_info.model));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Firmware:").strong());
                                    ui.label(data.firmware_version.as_ref().unwrap_or(&"N/A".to_string()));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Serial Number:").strong());
                                    ui.label(data.serial_number.as_ref().unwrap_or(&"N/A".to_string()));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Control Board:").strong());
                                    ui.label(data.control_board_version.as_ref().map(|cb| format!("{cb:?}")).unwrap_or_else(|| "N/A".to_string()));
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Is Mining:").strong());
                                    ui.label(if data.is_mining { "Yes" } else { "No" });
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Light Flashing:").strong());
                                    ui.label(if data.light_flashing.unwrap_or(false) { "Yes" } else { "No" });
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Uptime:").strong());
                                    if let Some(uptime) = data.uptime {
                                        ui.label(format!("{} seconds", uptime.as_secs()));
                                    } else {
                                        ui.label("N/A");
                                    }
                                    ui.end_row();
                                });

                            ui.add_space(15.0);
                            ui.heading("Performance");
                            ui.separator();
                            ui.add_space(5.0);

                            egui::Grid::new("performance_grid")
                                .num_columns(2)
                                .spacing([40.0, 8.0])
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Hashrate:").strong());
                                    if let Some(hr) = &data.hashrate {
                                        ui.label(format!("{:.2}", hr.clone().as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash)));
                                    } else {
                                        ui.label("N/A");
                                    }
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Wattage:").strong());
                                    if let Some(wattage) = data.wattage {
                                        ui.label(format!("{:.0} W", wattage.as_watts()));
                                    } else {
                                        ui.label("N/A");
                                    }
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Efficiency:").strong());
                                    if let Some(eff) = data.efficiency {
                                        ui.label(format!("{eff:.1} W/TH"));
                                    } else {
                                        ui.label("N/A");
                                    }
                                    ui.end_row();

                                    ui.label(egui::RichText::new("Avg Temperature:").strong());
                                    if let Some(temp) = data.average_temperature {
                                        ui.label(format!("{:.1}°C", temp.as_celsius()));
                                    } else {
                                        ui.label("N/A");
                                    }
                                    ui.end_row();
                                });

                            ui.add_space(15.0);
                            ui.heading("Fans");
                            ui.separator();
                            ui.add_space(5.0);

                            if !data.fans.is_empty() {
                                for (i, fan) in data.fans.iter().enumerate() {
                                    ui.label(egui::RichText::new(format!("Fan {}:", i + 1)).strong());
                                    ui.indent(format!("fan_{i}"), |ui| {
                                        if let Some(rpm) = fan.rpm {
                                            let rpm_value = rpm.as_radians_per_second() * 60.0 / (2.0 * std::f64::consts::PI);
                                            ui.label(format!("Speed: {rpm_value:.0} RPM"));
                                        }
                                    });
                                }
                            } else {
                                ui.label("No fan data available");
                            }

                            ui.add_space(15.0);
                            ui.heading("Pools");
                            ui.separator();
                            ui.add_space(5.0);

                            if !data.pools.is_empty() {
                                for (i, pool) in data.pools.iter().enumerate() {
                                    ui.label(egui::RichText::new(format!("Pool {}:", i + 1)).strong());
                                    ui.indent(format!("pool_{i}"), |ui| {
                                        if let Some(url) = &pool.url {
                                            ui.label(format!("URL: {url}"));
                                        }
                                        if let Some(user) = &pool.user {
                                            ui.label(format!("User: {user}"));
                                        }
                                        ui.label(format!("Active: {}", if pool.active.unwrap_or(false) { "Yes" } else { "No" }));
                                    });
                                    ui.add_space(5.0);
                                }
                            } else {
                                ui.label("No pool data available");
                            }

                            ui.add_space(15.0);
                            ui.heading("Hashboards");
                            ui.separator();
                            ui.add_space(5.0);

                            if !data.hashboards.is_empty() {
                                for (i, board) in data.hashboards.iter().enumerate() {
                                    ui.label(egui::RichText::new(format!("Hashboard {}:", i + 1)).strong());
                                    ui.indent(format!("board_{i}"), |ui| {
                                        if let Some(temp) = board.board_temperature {
                                            ui.label(format!("Board Temp: {:.1}°C", temp.as_celsius()));
                                        }
                                        if let Some(intake_temp) = board.intake_temperature {
                                            ui.label(format!("Intake Temp: {:.1}°C", intake_temp.as_celsius()));
                                        }
                                        if let Some(hashrate) = &board.hashrate {
                                            ui.label(format!("Hashrate: {:.2}", hashrate.clone().as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash)));
                                        }
                                        if let Some(chips) = board.expected_chips {
                                            ui.label(format!("Expected Chips: {chips}"));
                                        }
                                    });
                                    ui.add_space(5.0);
                                }
                            } else {
                                ui.label("No hashboard data available");
                            }
                                } else {
                                    ui.label("No detailed data available");
                                }
                            });
                        });

                        ui.add_space(10.0);

                        // Right column - Controls and Graphs (2/3 width)
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                // Auto-refresh indicator and manual refresh button
                                let last_refresh_time = self.detail_refresh_times.get(&miner.ip).cloned();
                                let should_auto_refresh = if let Some(last_time) = last_refresh_time {
                                    last_time.elapsed().as_secs() >= 10 // Auto-refresh every 10 seconds
                                } else {
                                    true // First time, refresh immediately
                                };

                                if should_auto_refresh {
                                    let ip = miner.ip.clone();
                                    let miners = Arc::clone(&self.miners);
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async move {
                                            let factory = MinerFactory::new();
                                            if let Ok(Some(miner_obj)) = factory.get_miner(ip.parse().unwrap()).await {
                                                let data = miner_obj.get_data().await;

                                                // Update the miner in the main list
                                                let mut miners_list = miners.lock().unwrap();
                                                if let Some(existing) = miners_list.iter_mut().find(|m| m.ip == ip) {
                                                    existing.full_data = Some(data);
                                                }
                                            }
                                        });
                                    });
                                    self.detail_refresh_times.insert(miner.ip.clone(), Instant::now());

                                    // Add data point to history
                                    if let Some(data) = &miner.full_data {
                                        let timestamp = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs_f64();

                                        let total_hashrate = if let Some(hr) = &data.hashrate {
                                            let converted = hr.clone().as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash);
                                            let display_str = format!("{converted}");
                                            display_str.split_whitespace().next()
                                                .and_then(|s| s.parse::<f64>().ok())
                                                .unwrap_or(0.0)
                                        } else {
                                            0.0
                                        };

                                        let power = if let Some(wattage) = data.wattage {
                                            wattage.as_watts()
                                        } else {
                                            0.0
                                        };

                                        // Collect per-board hashrates
                                        let board_hashrates: Vec<f64> = data.hashboards
                                            .iter()
                                            .filter_map(|board| {
                                                board.hashrate.as_ref().and_then(|hr| {
                                                    let converted = hr.clone().as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash);
                                                    let display_str = format!("{converted}");
                                                    display_str.split_whitespace().next()
                                                        .and_then(|s| s.parse::<f64>().ok())
                                                })
                                            })
                                            .collect();

                                        // Collect per-board temperatures and calculate average
                                        let board_temps: Vec<f64> = data.hashboards
                                            .iter()
                                            .filter_map(|board| {
                                                board.board_temperature.map(|temp| temp.as_celsius())
                                            })
                                            .collect();

                                        let avg_temp = if !board_temps.is_empty() {
                                            board_temps.iter().sum::<f64>() / board_temps.len() as f64
                                        } else {
                                            0.0
                                        };

                                        self.detail_metrics_history
                                            .entry(miner.ip.clone())
                                            .or_default()
                                            .push((timestamp, total_hashrate, power, board_hashrates, avg_temp, board_temps));
                                    }
                                }

                                ui.horizontal(|ui| {
                                    // Display last refresh time
                                    if let Some(last_time) = last_refresh_time {
                                        let elapsed = last_time.elapsed().as_secs();
                                        ui.label(
                                            egui::RichText::new(format!("Last updated: {elapsed}s ago"))
                                                .size(10.0)
                                                .color(Color32::from_rgb(120, 120, 120))
                                        );
                                    }

                                    ui.add_space(10.0);

                                    // Manual refresh button
                                    if ui.button(
                                        egui::RichText::new("🔄 Refresh")
                                            .color(Color32::WHITE)
                                    ).clicked() {
                                        let ip = miner.ip.clone();
                                        let miners = Arc::clone(&self.miners);
                                        std::thread::spawn(move || {
                                            let rt = tokio::runtime::Runtime::new().unwrap();
                                            rt.block_on(async move {
                                                let factory = MinerFactory::new();
                                                if let Ok(Some(miner_obj)) = factory.get_miner(ip.parse().unwrap()).await {
                                                    let data = miner_obj.get_data().await;

                                                    // Update the miner in the main list
                                                    let mut miners_list = miners.lock().unwrap();
                                                    if let Some(existing) = miners_list.iter_mut().find(|m| m.ip == ip) {
                                                        existing.full_data = Some(data);
                                                    }
                                                }
                                            });
                                        });
                                        self.detail_refresh_times.insert(miner.ip.clone(), Instant::now());
                                    }
                                });

                                // Web interface section
                                let url = format!("http://{}", miner.ip);
                                ui.horizontal(|ui| {
                                    // Web interface button
                                    if ui.add_sized(
                                        [200.0, 40.0],
                                    egui::Button::new(
                                        egui::RichText::new("🌐 Open Web Interface")
                                            .size(16.0)
                                            .color(Color32::WHITE)
                                    )
                                    .fill(Color32::from_rgb(100, 150, 255))
                                    .corner_radius(8.0)
                                ).clicked() {
                                        let _ = webbrowser::open(&url);
                                    }
                                // Quick actions section
                                ui.separator();
                                // Miner control actions
                                if ui.add_sized(
                                    [180.0, 40.0],
                                    egui::Button::new(
                                        egui::RichText::new("▶ START")
                                            .color(Color32::WHITE)
                                    )
                                    .fill(Color32::from_rgb(100, 200, 100))
                                ).clicked() {
                                    let ip = miner.ip.clone();
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async move {
                                            let factory = MinerFactory::new();
                                            if let Ok(Some(miner)) = factory.get_miner(ip.parse().unwrap()).await {
                                                match miner.resume(None).await {
                                                    Ok(_) => println!("✓ Started miner: {ip}"),
                                                    Err(e) => eprintln!("✗ Failed to start {ip}: {e}"),
                                                }
                                            }
                                        });
                                    });
                                }

                                if ui.add_sized(
                                    [180.0, 40.0],
                                    egui::Button::new(
                                        egui::RichText::new("■ STOP")
                                            .color(Color32::WHITE)
                                    )
                                    .fill(Color32::from_rgb(255, 100, 100))
                                ).clicked() {
                                    let ip = miner.ip.clone();
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async move {
                                            let factory = MinerFactory::new();
                                            if let Ok(Some(miner)) = factory.get_miner(ip.parse().unwrap()).await {
                                                match miner.pause(None).await {
                                                    Ok(_) => println!("✓ Stopped miner: {ip}"),
                                                    Err(e) => eprintln!("✗ Failed to stop {ip}: {e}"),
                                                }
                                            }
                                        });
                                    });
                                }

                                if ui.add_sized(
                                    [180.0, 40.0],
                                    egui::Button::new(
                                        egui::RichText::new("💡 FAULT LIGHT")
                                            .color(Color32::WHITE)
                                    )
                                    .fill(Color32::from_rgb(255, 165, 0))
                                ).clicked() {
                                    let ip = miner.ip.clone();
                                    std::thread::spawn(move || {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async move {
                                            let factory = MinerFactory::new();
                                            if let Ok(Some(miner)) = factory.get_miner(ip.parse().unwrap()).await {
                                                match miner.set_fault_light(true).await {
                                                    Ok(_) => println!("✓ Set fault light on: {ip}"),
                                                    Err(e) => eprintln!("✗ Failed to set fault light on {ip}: {e}"),
                                                }
                                            }
                                        });
                                    });
                                }
                                });
                                // Get historical data for this miner
                                let history = self.detail_metrics_history.get(&miner.ip);

                                if let Some(history_data) = history {
                                    use egui_plot::{Line, Plot, PlotPoints, Legend};

                                    ui.heading("Metrics Over Time");
                                    ui.separator();
                                    ui.add_space(5.0);

                                    if !history_data.is_empty() {
                                        // Find the earliest timestamp to normalize
                                        let start_time = history_data.first().map(|p| p.0).unwrap_or(0.0);
                                        let num_boards = history_data.first()
                                            .map(|(_, _, _, boards, _, _)| boards.len())
                                            .unwrap_or(0);

                                        // Row 1: Hashrate (total + per-board)
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new("Hashrate").strong());

                                            let total_hashrate_points: Vec<[f64; 2]> = history_data
                                                .iter()
                                                .map(|(ts, hr, _, _, _, _)| [(ts - start_time), *hr])
                                                .collect();

                                            let max_hashrate = total_hashrate_points.iter()
                                                .map(|p| p[1])
                                                .fold(0.0f64, f64::max);

                                            Plot::new(format!("total_hashrate_{}", miner.ip))
                                                .height(200.0)
                                                .allow_zoom([true, false])
                                                .allow_scroll(false)
                                                .include_y(0.0)
                                                .include_y(max_hashrate * 1.1)
                                                .legend(Legend::default())
                                                .show(ui, |plot_ui| {
                                                    // Total hashrate line
                                                    plot_ui.line(
                                                        Line::new("total_hashrate",PlotPoints::from(total_hashrate_points.clone()))
                                                            .color(Color32::from_rgb(100, 200, 255))
                                                            .width(2.5)
                                                            .name("Total")
                                                    );

                                                    // Per-board hashrate lines
                                                    let board_colors = [
                                                        Color32::from_rgb(100, 255, 100),
                                                        Color32::from_rgb(255, 255, 100),
                                                        Color32::from_rgb(255, 150, 100),
                                                        Color32::from_rgb(200, 100, 255),
                                                    ];

                                                    for board_idx in 0..num_boards {
                                                        let board_points: Vec<[f64; 2]> = history_data
                                                            .iter()
                                                            .filter_map(|(ts, _, _, boards, _, _)| {
                                                                boards.get(board_idx).map(|hr| [(ts - start_time), *hr])
                                                            })
                                                            .collect();

                                                        if !board_points.is_empty() {
                                                            plot_ui.line(
                                                                Line::new("Points",PlotPoints::from(board_points))
                                                                    .color(board_colors[board_idx % board_colors.len()])
                                                                    .width(1.5)
                                                                    .name(format!("Board {board_idx}"))
                                                            );
                                                        }
                                                    }
                                                });

                                            if let Some(latest) = history_data.last() {
                                                ui.label(format!("Total: {:.2} TH/s", latest.1));
                                            }
                                        });

                                        ui.add_space(15.0);

                                        // Row 2: Temperature (average + per-board)
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new("Temperature").strong());

                                            let avg_temp_points: Vec<[f64; 2]> = history_data
                                                .iter()
                                                .map(|(ts, _, _, _, avg_t, _)| [(ts - start_time), *avg_t])
                                                .collect();

                                            let max_temp = avg_temp_points.iter()
                                                .map(|p| p[1])
                                                .fold(0.0f64, f64::max);

                                            Plot::new(format!("temperature_{}", miner.ip))
                                                .height(200.0)
                                                .allow_zoom([true, false])
                                                .allow_scroll(false)
                                                .include_y(0.0)
                                                .include_y(max_temp * 1.1)
                                                .legend(Legend::default())
                                                .show(ui, |plot_ui| {
                                                    // Average temperature line
                                                    plot_ui.line(
                                                        Line::new("temp",PlotPoints::from(avg_temp_points.clone()))
                                                            .color(Color32::from_rgb(255, 100, 100))
                                                            .width(2.5)
                                                            .name("Average")
                                                    );

                                                    // Per-board temperature lines
                                                    let board_colors = [
                                                        Color32::from_rgb(255, 150, 150),
                                                        Color32::from_rgb(255, 200, 100),
                                                        Color32::from_rgb(200, 150, 255),
                                                        Color32::from_rgb(150, 255, 200),
                                                    ];

                                                    for board_idx in 0..num_boards {
                                                        let board_temp_points: Vec<[f64; 2]> = history_data
                                                            .iter()
                                                            .filter_map(|(ts, _, _, _, _, temps)| {
                                                                temps.get(board_idx).map(|t| [(ts - start_time), *t])
                                                            })
                                                            .collect();

                                                        if !board_temp_points.is_empty() {
                                                            plot_ui.line(
                                                                Line::new("board_temp",PlotPoints::from(board_temp_points))
                                                                    .color(board_colors[board_idx % board_colors.len()])
                                                                    .width(1.5)
                                                                    .name(format!("Board {board_idx}"))
                                                            );
                                                        }
                                                    }
                                                });

                                            if let Some(latest) = history_data.last() {
                                                ui.label(format!("Average: {:.1} °C", latest.4));
                                            }
                                        });

                                        ui.add_space(15.0);

                                        // Row 3: Power
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new("Power").strong());

                                            let power_points: Vec<[f64; 2]> = history_data
                                                .iter()
                                                .map(|(ts, _, pw, _, _, _)| [(ts - start_time), *pw])
                                                .collect();

                                            let max_power = power_points.iter()
                                                .map(|p| p[1])
                                                .fold(0.0f64, f64::max);

                                            Plot::new(format!("total_power_{}", miner.ip))
                                                .height(200.0)
                                                .allow_zoom([true, false])
                                                .allow_scroll(false)
                                                .include_y(0.0)
                                                .include_y(max_power * 1.1)
                                                .show(ui, |plot_ui| {
                                                    plot_ui.line(
                                                        Line::new("power",PlotPoints::from(power_points.clone()))
                                                            .color(Color32::from_rgb(255, 165, 0))
                                                            .width(2.0)
                                                    );
                                                });

                                            if let Some(latest) = history_data.last() {
                                                ui.label(format!("Current: {:.0} W", latest.2));
                                            }
                                        });

                                        ui.add_space(5.0);
                                        ui.label(format!("Data points: {} (last {}s)", history_data.len(), ((history_data.len() - 1) * 10)));
                                    } else {
                                        ui.label("Collecting data...");
                                    }
                                    ui.add_space(900.0);

                                }

                            });
                        });
                    });
                    });
            }

            if !is_open {
                miners_to_close.push(idx);
            }

            drop(miners_list);

            // Request repaint to update the refresh timer
            ctx.request_repaint();
        }

        // Remove closed miners (in reverse order to maintain indices)
        for idx in miners_to_close.iter().rev() {
            let miner = self.detail_view_miners.remove(*idx);
            // Clean up history for this miner
            self.detail_metrics_history.remove(&miner.ip);
            self.detail_refresh_times.remove(&miner.ip);
        }
    }

    fn draw_hashrate_plot(&self, ui: &mut egui::Ui) {
        let history = self.hashrate_history.lock().unwrap();

        if history.is_empty() {
            return;
        }

        egui::Frame::new()
            .fill(Color32::from_rgb(28, 28, 28))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
            .corner_radius(4.0)
            .inner_margin(20.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("📊 HASHRATE HISTORY")
                            .size(13.0)
                            .color(Color32::from_rgb(240, 240, 240))
                            .monospace(),
                    );
                });

                ui.add_space(15.0);

                use egui_plot::{Line, Plot, PlotPoints};

                Plot::new("hashrate_plot")
                    .height(250.0)
                    .show_axes([true, true])
                    .show_grid([true, true])
                    .x_axis_label("Time")
                    .y_axis_label("Hashrate (TH/s)")
                    .label_formatter(|name, value| {
                        if !name.is_empty() {
                            format!("{}\n{:.2} TH/s", name, value.y)
                        } else {
                            format!("{:.2} TH/s", value.y)
                        }
                    })
                    .legend(egui_plot::Legend::default())
                    .show(ui, |plot_ui| {
                        // Generate different colors for each miner
                        let colors = [
                            Color32::from_rgb(255, 87, 51),
                            Color32::from_rgb(100, 200, 255),
                            Color32::from_rgb(100, 255, 100),
                            Color32::from_rgb(255, 200, 100),
                            Color32::from_rgb(255, 100, 255),
                            Color32::from_rgb(100, 255, 255),
                        ];

                        for (idx, (ip, points)) in history.iter().enumerate() {
                            if !points.is_empty() {
                                let plot_points: PlotPoints =
                                    points.iter().map(|p| [p.timestamp, p.hashrate]).collect();

                                let color = colors[idx % colors.len()];
                                plot_ui.line(
                                    Line::new("plot", plot_points)
                                        .color(color)
                                        .name(ip)
                                        .width(2.0),
                                );
                            }
                        }
                    });
            });
    }

    fn draw_miners_table(&mut self, ui: &mut egui::Ui) {
        let miners = self.miners.lock().unwrap();
        let sort_column = self.sort_column;
        let sort_direction = self.sort_direction;
        let mut clicked_column: Option<SortColumn> = None;

        // Bulk actions bar
        if !miners.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Selected: {}", self.selected_miners.len()))
                        .size(11.0)
                        .color(Color32::from_rgb(160, 160, 160))
                        .monospace(),
                );

                ui.add_space(10.0);

                let start_btn = egui::Button::new(
                    egui::RichText::new("▶ START")
                        .size(11.0)
                        .color(Color32::WHITE)
                        .monospace(),
                )
                .fill(Color32::from_rgb(100, 200, 100))
                .corner_radius(4.0);

                if ui
                    .add_enabled(!self.selected_miners.is_empty(), start_btn)
                    .on_hover_text("Start selected miners")
                    .clicked()
                {
                    let selected_ips = self.selected_miners.clone();
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        for ip in selected_ips {
                            let ip_clone = ip.clone();
                            rt.block_on(async move {
                                let factory = MinerFactory::new();
                                if let Ok(Some(miner)) =
                                    factory.get_miner(ip_clone.parse().unwrap()).await
                                {
                                    match miner.resume(None).await {
                                        Ok(_) => println!("✓ Started miner: {ip_clone}"),
                                        Err(e) => {
                                            eprintln!("✗ Failed to start {ip_clone}: {e}")
                                        }
                                    }
                                }
                            });
                        }
                    });
                }

                ui.add_space(5.0);

                let stop_btn = egui::Button::new(
                    egui::RichText::new("■ STOP")
                        .size(11.0)
                        .color(Color32::WHITE)
                        .monospace(),
                )
                .fill(Color32::from_rgb(255, 100, 100))
                .corner_radius(4.0);

                if ui
                    .add_enabled(!self.selected_miners.is_empty(), stop_btn)
                    .on_hover_text("Stop selected miners")
                    .clicked()
                {
                    let selected_ips = self.selected_miners.clone();
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        for ip in selected_ips {
                            let ip_clone = ip.clone();
                            rt.block_on(async move {
                                let factory = MinerFactory::new();
                                if let Ok(Some(miner)) =
                                    factory.get_miner(ip_clone.parse().unwrap()).await
                                {
                                    match miner.pause(None).await {
                                        Ok(_) => println!("✓ Stopped miner: {ip_clone}"),
                                        Err(e) => eprintln!("✗ Failed to stop {ip_clone}: {e}"),
                                    }
                                }
                            });
                        }
                    });
                }

                ui.add_space(5.0);

                let fault_light_btn = egui::Button::new(
                    egui::RichText::new("💡 FAULT LIGHT")
                        .size(11.0)
                        .color(Color32::WHITE)
                        .monospace(),
                )
                .fill(Color32::from_rgb(255, 165, 0))
                .corner_radius(4.0);

                if ui
                    .add_enabled(!self.selected_miners.is_empty(), fault_light_btn)
                    .on_hover_text("Set fault light on selected miners")
                    .clicked()
                {
                    let selected_ips = self.selected_miners.clone();
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        for ip in selected_ips {
                            let ip_clone = ip.clone();
                            rt.block_on(async move {
                                let factory = MinerFactory::new();
                                if let Ok(Some(miner)) =
                                    factory.get_miner(ip_clone.parse().unwrap()).await
                                {
                                    match miner.set_fault_light(true).await {
                                        Ok(_) => println!("✓ Set fault light on: {ip_clone}"),
                                        Err(e) => eprintln!(
                                            "✗ Failed to set fault light on {ip_clone}: {e}"
                                        ),
                                    }
                                }
                            });
                        }
                    });
                }

                ui.add_space(10.0);

                if ui.button("Select All").clicked() {
                    self.selected_miners = miners.iter().map(|m| m.ip.clone()).collect();
                }

                if ui.button("Deselect All").clicked() {
                    self.selected_miners.clear();
                }
            });

            ui.add_space(10.0);
        }

        if miners.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("⚡ NO MINERS FOUND")
                        .size(14.0)
                        .color(Color32::from_rgb(120, 120, 120))
                        .monospace(),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Start a scan to discover ASIC miners on your network")
                        .size(11.0)
                        .color(Color32::from_rgb(100, 100, 100))
                        .monospace(),
                );
            });
        } else {
            ui.add_space(15.0);

            egui::Frame::new()
                .fill(Color32::from_rgb(28, 28, 28))
                .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
                .corner_radius(4.0)
                .inner_margin(20.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("⛏ DISCOVERED MINERS ({})", miners.len()))
                                .size(13.0)
                                .color(Color32::from_rgb(240, 240, 240))
                                .monospace(),
                        );
                    });

                    ui.add_space(15.0);

                    egui::ScrollArea::horizontal()
                        .min_scrolled_width(0.0)
                        .show(ui, |ui| {
                            use egui_extras::{Column, TableBuilder};

                            TableBuilder::new(ui)
                                .striped(true)
                                .resizable(true)
                                .drag_to_scroll(false)
                                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                .column(Column::initial(40.0)) // Checkbox
                                .column(Column::initial(30.0)) // Fault Light
                                .column(Column::initial(140.0).resizable(true)) // IP
                                .column(Column::initial(150.0).resizable(true)) // Hostname
                                .column(Column::initial(150.0).resizable(true)) // Model
                                .column(Column::initial(150.0).resizable(true)) // Firmware
                                .column(Column::initial(150.0).resizable(true)) // Control Board
                                .column(Column::initial(120.0).resizable(true)) // Hashrate
                                .column(Column::initial(100.0).resizable(true)) // Wattage
                                .column(Column::initial(100.0).resizable(true)) // Efficiency
                                .column(Column::initial(120.0).resizable(true)) // Temperature
                                .column(Column::initial(120.0).resizable(true)) // Fan Speed
                                .column(Column::remainder().at_least(200.0).resizable(true)) // Pool
                                .header(30.0, |mut header| {
                                    let get_indicator = |col: SortColumn| {
                                        if sort_column == Some(col) {
                                            match sort_direction {
                                                SortDirection::Ascending => " ▲",
                                                SortDirection::Descending => " ▼",
                                            }
                                        } else {
                                            ""
                                        }
                                    };

                                    // Checkbox column header
                                    header.col(|ui| {
                                        ui.label(
                                            egui::RichText::new("☐")
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                        );
                                    });
                                    // Fault light column header
                                    header.col(|ui| {
                                        ui.label(
                                            egui::RichText::new("💡")
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51)),
                                        );
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "IP ADDRESS{}",
                                                    get_indicator(SortColumn::Ip)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Ip);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "HOSTNAME{}",
                                                    get_indicator(SortColumn::Hostname)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Hostname);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "MODEL{}",
                                                    get_indicator(SortColumn::Model)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Model);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "FIRMWARE{}",
                                                    get_indicator(SortColumn::Firmware)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Firmware);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "CONTROL BOARD{}",
                                                    get_indicator(SortColumn::ControlBoard)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::ControlBoard);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "HASHRATE{}",
                                                    get_indicator(SortColumn::Hashrate)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Hashrate);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "WATTAGE{}",
                                                    get_indicator(SortColumn::Wattage)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Wattage);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "W/TH{}",
                                                    get_indicator(SortColumn::Efficiency)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Efficiency);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "TEMPERATURE{}",
                                                    get_indicator(SortColumn::Temperature)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Temperature);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "FAN SPEED{}",
                                                    get_indicator(SortColumn::FanSpeed)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::FanSpeed);
                                        }
                                    });
                                    header.col(|ui| {
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "POOL{}",
                                                    get_indicator(SortColumn::Pool)
                                                ))
                                                .size(11.0)
                                                .color(Color32::from_rgb(255, 87, 51))
                                                .monospace(),
                                            )
                                            .clicked()
                                        {
                                            clicked_column = Some(SortColumn::Pool);
                                        }
                                    });
                                })
                                .body(|mut body| {
                                    for miner in miners.iter() {
                                        let is_selected = self.selected_miners.contains(&miner.ip);
                                        body.row(35.0, |mut row| {
                                            // Checkbox column
                                            row.col(|ui| {
                                                let mut selected = is_selected;
                                                if ui.checkbox(&mut selected, "").changed() {
                                                    if selected {
                                                        self.selected_miners
                                                            .insert(miner.ip.clone());
                                                    } else {
                                                        self.selected_miners.remove(&miner.ip);
                                                    }
                                                }
                                            });
                                            // Fault light indicator column
                                            row.col(|ui| {
                                                if miner.light_flashing {
                                                    // Flashing effect using time-based animation
                                                    let time = ui.input(|i| i.time);
                                                    let flash = (time * 3.0).sin() > 0.0;
                                                    let color = if flash {
                                                        Color32::from_rgb(255, 165, 0)
                                                    // Orange
                                                    } else {
                                                        Color32::from_rgb(150, 100, 0)
                                                        // Dim orange
                                                    };
                                                    ui.label(
                                                        egui::RichText::new("💡")
                                                            .size(14.0)
                                                            .color(color),
                                                    );
                                                    ui.ctx().request_repaint();
                                                }
                                            });
                                            row.col(|ui| {
                                                ui.horizontal(|ui| {
                                                    // Clickable IP to open detail modal
                                                    if ui
                                                        .button(
                                                            egui::RichText::new(&miner.ip)
                                                                .size(11.0)
                                                                .color(Color32::from_rgb(
                                                                    100, 200, 255,
                                                                ))
                                                                .monospace(),
                                                        )
                                                        .on_hover_text("Click to view details")
                                                        .clicked()
                                                    {
                                                        // Only add if not already open
                                                        if !self
                                                            .detail_view_miners
                                                            .iter()
                                                            .any(|m| m.ip == miner.ip)
                                                        {
                                                            self.detail_view_miners
                                                                .push(miner.clone());
                                                        }
                                                    }
                                                });
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.hostname)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(200, 200, 200))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.model)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(200, 200, 200))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.firmware_version)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(200, 200, 200))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.control_board)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(200, 200, 200))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.hashrate)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(100, 200, 255))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.wattage)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(255, 200, 100))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.efficiency)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(200, 150, 255))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.temperature)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(255, 150, 100))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.fan_speed)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(150, 200, 150))
                                                        .monospace(),
                                                );
                                            });
                                            row.col(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&miner.pool)
                                                        .size(11.0)
                                                        .color(Color32::from_rgb(200, 200, 200))
                                                        .monospace(),
                                                );
                                            });
                                        });
                                    }
                                });
                        });
                });
        }

        // Drop the lock before sorting
        drop(miners);

        // Sort if a column header was clicked
        if let Some(column) = clicked_column {
            self.sort_miners(column);
        }
    }
}

impl eframe::App for MinerScannerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set dark theme
        ctx.set_visuals(egui::Visuals {
            dark_mode: true,
            window_fill: Color32::from_rgb(18, 18, 18),
            panel_fill: Color32::from_rgb(18, 18, 18),
            override_text_color: Some(Color32::from_rgb(200, 200, 200)),
            ..Default::default()
        });

        // Request repaint while scanning
        {
            let progress = self.scan_progress.lock().unwrap();
            if progress.scanning {
                ctx.request_repaint();
            }
        }

        // Show miner detail modal if one is selected
        self.draw_miner_detail_modal(ctx);

        // Top bar
        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::new().fill(Color32::from_rgb(18, 18, 18)))
            .show(ctx, |ui| {
                ui.add_space(15.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new("⛏ ASIC-RS MINER SCANNER")
                            .size(16.0)
                            .color(Color32::from_rgb(255, 87, 51))
                            .monospace(),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(20.0);
                        let miners_count = self.miners.lock().unwrap().len();
                        ui.label(
                            egui::RichText::new(format!("MINERS FOUND: {miners_count}"))
                                .size(11.0)
                                .color(Color32::from_rgb(160, 160, 160))
                                .monospace(),
                        );
                    });
                });
                ui.add_space(15.0);

                ui.separator();
            });

        // Main content
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(Color32::from_rgb(18, 18, 18))
                    .inner_margin(20.0),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // Top row: Fleet Overview + Scan Control
                    let row_height = 400.0; // Fixed height for both cards

                    ui.horizontal(|ui| {
                        // Fleet Overview (left column) - centered vertically
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width() / 2.0 - 7.5, row_height),
                            egui::Layout::centered_and_justified(egui::Direction::TopDown),
                            |ui| {
                                self.draw_stats_card(ui);
                            },
                        );

                        ui.add_space(15.0);

                        // Scan Control & Ranges (right column)
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), row_height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                self.draw_scan_and_ranges_card(ui);
                            },
                        );
                    });

                    ui.add_space(15.0);

                    // Table
                    self.draw_miners_table(ui);
                });
            });

        // Auto-scan logic
        if self.auto_scan_enabled && !self.saved_ranges.is_empty() {
            let should_scan = if let Some(last_scan) = self.last_scan_time {
                last_scan.elapsed().as_secs() >= self.auto_scan_interval_secs
            } else {
                true
            };

            let is_scanning = self.scan_progress.lock().unwrap().scanning;

            if should_scan && !is_scanning {
                self.scan_all_saved_ranges();
            }

            // Request repaint to keep checking
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}
