mod config;
mod models;
mod recording;
mod scanner;
mod ui;

use eframe::egui;
use egui::Color32;
use models::{MetricsHistory, MinerInfo, SavedRange, ScanProgress, SortColumn, SortDirection};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use ui::ScanControlState;

fn main() -> Result<(), eframe::Error> {
    // Auto-increase ulimit to max on Unix systems (do once at startup)
    #[cfg(unix)]
    {
        use rlimit::Resource;

        if let Ok((soft, hard)) = Resource::NOFILE.get() {
            if soft < hard {
                // Increase soft limit to hard limit (maximum allowed)
                match Resource::NOFILE.set(hard, hard) {
                    Ok(_) => {
                        println!("✓ Increased file descriptor limit from {soft} to {hard}");
                    }
                    Err(e) => {
                        eprintln!("⚠️  WARNING: Could not increase file descriptor limit from {soft} to {hard}: {e}");
                    }
                }
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("asic-rs Miner Scanner"),
        ..Default::default()
    };

    eframe::run_native(
        "asic-rs Miner Scanner",
        options,
        Box::new(|cc| Ok(Box::new(MinerScannerApp::new(cc)))),
    )
}

struct MinerScannerApp {
    miners: Arc<Mutex<Vec<MinerInfo>>>,
    scan_progress: Arc<Mutex<ScanProgress>>,
    error_message: String,
    sort_column: Option<SortColumn>,
    sort_direction: SortDirection,
    saved_ranges: Vec<SavedRange>,
    hashrate_history: Arc<Mutex<HashMap<String, Vec<models::HashratePoint>>>>,
    selected_miners: HashSet<String>,
    detail_view_miners: Vec<MinerInfo>,
    detail_refresh_times: HashMap<String, Instant>,
    detail_graph_update_times: HashMap<String, Instant>, // For 200ms rolling graph updates
    detail_metrics_history: HashMap<String, MetricsHistory>,
    search_query: String,
    scan_control_state: ScanControlState,
    recording_states: HashMap<String, models::RecordingState>, // IP -> RecordingState
    detail_refresh_interval_secs: u64,                         // Refresh interval for detail modal
    prev_detail_refresh_interval_secs: u64,                    // Previous value to detect changes
    prev_auto_scan_interval_secs: u64,                         // Previous value to detect changes
    prev_identification_timeout_secs: u64,                     // Previous value to detect changes
    prev_connectivity_timeout_secs: u64,                       // Previous value to detect changes
    prev_connectivity_retries: u32,                            // Previous value to detect changes
    fleet_hashrate_history: Vec<(f64, f64)>,                   // (timestamp, total_hashrate)
    last_fleet_update: Option<Instant>,
}

impl MinerScannerApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let app_config = config::load_config();

        Self {
            miners: Arc::new(Mutex::new(Vec::new())),
            scan_progress: Arc::new(Mutex::new(ScanProgress {
                scanning: false,
                current_ip: String::new(),
                total_ips: 0,
                scanned_ips: 0,
                found_miners: 0,
                scan_start_time: None,
                total_ranges: 0,
                scanned_ranges: 0,
                scan_duration_secs: 0,
            })),
            error_message: String::new(),
            sort_column: None,
            sort_direction: SortDirection::Ascending,
            saved_ranges: app_config.saved_ranges,
            hashrate_history: Arc::new(Mutex::new(HashMap::new())),
            selected_miners: HashSet::new(),
            detail_view_miners: Vec::new(),
            detail_refresh_times: HashMap::new(),
            detail_graph_update_times: HashMap::new(),
            detail_metrics_history: HashMap::new(),
            search_query: String::new(),
            scan_control_state: ScanControlState {
                ip_range_start: "10.0.81.0".to_string(),
                ip_range_end: "10.0.81.255".to_string(),
                new_range_name: String::new(),
                auto_scan_enabled: true,
                auto_scan_interval_secs: app_config.auto_scan_interval_secs,
                last_scan_time: None,
                identification_timeout_secs: app_config.identification_timeout_secs,
                connectivity_timeout_secs: app_config.connectivity_timeout_secs,
                connectivity_retries: app_config.connectivity_retries,
                show_name_error: false,
            },
            recording_states: HashMap::new(),
            detail_refresh_interval_secs: app_config.detail_refresh_interval_secs,
            prev_detail_refresh_interval_secs: app_config.detail_refresh_interval_secs,
            prev_auto_scan_interval_secs: app_config.auto_scan_interval_secs,
            prev_identification_timeout_secs: app_config.identification_timeout_secs,
            prev_connectivity_timeout_secs: app_config.connectivity_timeout_secs,
            prev_connectivity_retries: app_config.connectivity_retries,
            fleet_hashrate_history: Vec::new(),
            last_fleet_update: None,
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
                SortColumn::Worker => a.worker.cmp(&b.worker),
            };

            match direction {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }

    fn save_config(&self) {
        let app_config = config::AppConfig {
            saved_ranges: self.saved_ranges.clone(),
            detail_refresh_interval_secs: self.detail_refresh_interval_secs,
            auto_scan_interval_secs: self.scan_control_state.auto_scan_interval_secs,
            identification_timeout_secs: self.scan_control_state.identification_timeout_secs,
            connectivity_timeout_secs: self.scan_control_state.connectivity_timeout_secs,
            connectivity_retries: self.scan_control_state.connectivity_retries,
        };
        config::save_config(&app_config);
    }

    fn add_saved_range(&mut self) {
        if !self.scan_control_state.new_range_name.trim().is_empty() {
            let range = match scanner::parse_ip_range(
                &self.scan_control_state.ip_range_start,
                &self.scan_control_state.ip_range_end,
            ) {
                Ok(range) => range,
                Err(e) => {
                    self.error_message = e;
                    return;
                }
            };

            self.saved_ranges.push(SavedRange {
                name: self.scan_control_state.new_range_name.trim().to_string(),
                range,
            });
            self.scan_control_state.new_range_name.clear();
            self.scan_control_state.show_name_error = false;
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
                self.scan_control_state.ip_range_start = start;
                self.scan_control_state.ip_range_end = end;
            }
        }
    }

    fn export_miners_to_csv(&self) {
        use chrono::Local;
        use std::fs;

        let miners = self.miners.lock().unwrap();
        if miners.is_empty() {
            return;
        }

        let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let filename = format!("miner_export_{}.csv", timestamp);

        // Use file picker to save the file
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&filename)
            .add_filter("CSV Files", &["csv"])
            .save_file()
        {
            let mut csv_content = String::new();

            // Header
            csv_content.push_str("IP,Hostname,Model,Firmware,Control Board,Hashrate (TH/s),Wattage (W),Efficiency (W/TH),Temperature (°C),Fan Speed (RPM),Pool,Worker\n");

            // Rows
            for miner in miners.iter() {
                csv_content.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    miner.ip,
                    miner.hostname,
                    miner.model,
                    miner.firmware_version,
                    miner.control_board,
                    miner.hashrate.replace(" TH/s", ""),
                    miner.wattage.replace(" W", ""),
                    miner.efficiency.replace(" W/TH", ""),
                    miner.temperature.replace("°C", ""),
                    miner.fan_speed.replace(" RPM", ""),
                    miner.pool,
                    miner.worker
                ));
            }

            if let Err(e) = fs::write(&path, csv_content) {
                eprintln!("Failed to export CSV: {}", e);
            } else {
                println!("Exported {} miners to {}", miners.len(), path.display());
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

        let total_ips: usize = ranges.iter().map(|r| scanner::calculate_total_ips(r)).sum();

        // Update scan progress
        {
            let mut progress = self.scan_progress.lock().unwrap();
            progress.scanning = true;
            progress.total_ips = total_ips;
            progress.scanned_ips = 0;
            progress.found_miners = 0;
            progress.current_ip.clear();
            progress.scan_start_time = Some(Instant::now());
            progress.total_ranges = ranges.len();
            progress.scanned_ranges = 0;
            progress.scan_duration_secs = 0;
        }

        scanner::scan_ranges(
            ranges,
            Arc::clone(&self.miners),
            Arc::clone(&self.scan_progress),
            Arc::clone(&self.hashrate_history),
            self.scan_control_state.identification_timeout_secs,
            self.scan_control_state.connectivity_timeout_secs,
            self.scan_control_state.connectivity_retries,
        );

        self.scan_control_state.last_scan_time = Some(Instant::now());
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

        // Update fleet hashrate history periodically (every ~33ms for 30fps rolling effect)
        let should_update_fleet = if let Some(last_update) = self.last_fleet_update {
            last_update.elapsed().as_millis() >= 33
        } else {
            true
        };

        if should_update_fleet {
            let miners = self.miners.lock().unwrap();

            // Only record if there are miners
            if !miners.is_empty() {
                let total_hashrate: f64 = miners
                    .iter()
                    .filter_map(|m| m.hashrate.split_whitespace().next()?.parse::<f64>().ok())
                    .sum();
                drop(miners);

                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs_f64();

                self.fleet_hashrate_history
                    .push((timestamp, total_hashrate));

                // Keep only last 9000 points (~5 minutes at 30fps for smooth rolling)
                if self.fleet_hashrate_history.len() > 9000 {
                    self.fleet_hashrate_history.remove(0);
                }
            } else {
                drop(miners);
            }

            self.last_fleet_update = Some(Instant::now());
        }

        // Show miner detail modal if one is selected
        ui::draw_miner_detail_modal(
            ctx,
            &mut self.detail_view_miners,
            Arc::clone(&self.miners),
            &mut self.detail_refresh_times,
            &mut self.detail_graph_update_times,
            &mut self.detail_metrics_history,
            &mut self.recording_states,
            &mut self.detail_refresh_interval_secs,
        );

        // Save config if any interval or scan parameter changed
        if self.detail_refresh_interval_secs != self.prev_detail_refresh_interval_secs
            || self.scan_control_state.auto_scan_interval_secs != self.prev_auto_scan_interval_secs
            || self.scan_control_state.identification_timeout_secs
                != self.prev_identification_timeout_secs
            || self.scan_control_state.connectivity_timeout_secs
                != self.prev_connectivity_timeout_secs
            || self.scan_control_state.connectivity_retries != self.prev_connectivity_retries
        {
            self.prev_detail_refresh_interval_secs = self.detail_refresh_interval_secs;
            self.prev_auto_scan_interval_secs = self.scan_control_state.auto_scan_interval_secs;
            self.prev_identification_timeout_secs =
                self.scan_control_state.identification_timeout_secs;
            self.prev_connectivity_timeout_secs = self.scan_control_state.connectivity_timeout_secs;
            self.prev_connectivity_retries = self.scan_control_state.connectivity_retries;
            self.save_config();
        }

        // Top bar
        egui::TopBottomPanel::top("top_bar")
            .frame(egui::Frame::new().fill(Color32::from_rgb(18, 18, 18)))
            .show(ctx, |ui| {
                ui.add_space(15.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);

                    // Display logo with 3D effect
                    egui::Frame::new()
                        .fill(Color32::from_rgb(255, 87, 51))
                        .inner_margin(egui::vec2(12.0, 6.0))
                        .corner_radius(6.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                // Shadow layers for 3D effect
                                ui.label(
                                    egui::RichText::new("asic-rs")
                                        .size(20.0)
                                        .color(Color32::from_rgb(0, 0, 0))
                                        .strong()
                                        .raised()
                                        .monospace(),
                                );
                            });
                        });

                    ui.add_space(10.0);

                    ui.label(
                        egui::RichText::new("MINER SCANNER")
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
                    let row_height = 400.0;

                    ui.horizontal(|ui| {
                        // Fleet Overview (left column) - centered vertically
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width() / 2.0 - 7.5, row_height),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                let miners = self.miners.lock().unwrap();
                                ui::draw_stats_card(ui, &miners, &self.fleet_hashrate_history);
                            },
                        );

                        ui.add_space(15.0);

                        // Scan Control & Ranges (right column)
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), row_height),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let mut on_scan_clicked = false;
                                let mut on_save_range_clicked = false;
                                let mut range_to_remove: Option<usize> = None;
                                let mut range_to_load: Option<SavedRange> = None;

                                ui::draw_scan_and_ranges_card(
                                    ui,
                                    &mut self.scan_control_state,
                                    &self.saved_ranges,
                                    Arc::clone(&self.scan_progress),
                                    &mut on_scan_clicked,
                                    &mut on_save_range_clicked,
                                    &mut range_to_remove,
                                    &mut range_to_load,
                                );

                                // Handle events after rendering
                                if on_scan_clicked {
                                    self.scan_all_saved_ranges();
                                }
                                if on_save_range_clicked {
                                    self.add_saved_range();
                                }
                                if let Some(idx) = range_to_remove {
                                    self.remove_saved_range(idx);
                                }
                                if let Some(range) = range_to_load {
                                    self.load_saved_range(&range);
                                }
                            },
                        );
                    });

                    ui.add_space(15.0);

                    // Table
                    let miners = self.miners.lock().unwrap().clone();
                    drop(miners);

                    let mut export_clicked = false;
                    let clicked_column = ui::draw_miners_table(
                        ui,
                        &self.miners.lock().unwrap(),
                        &mut self.search_query,
                        &mut self.selected_miners,
                        &mut self.detail_view_miners,
                        self.sort_column,
                        self.sort_direction,
                        Arc::clone(&self.scan_progress),
                        &mut export_clicked,
                    );

                    // Sort if a column header was clicked
                    if let Some(column) = clicked_column {
                        self.sort_miners(column);
                    }

                    // Export to CSV if export button was clicked
                    if export_clicked {
                        self.export_miners_to_csv();
                    }
                });
            });

        // Auto-scan logic
        if self.scan_control_state.auto_scan_enabled && !self.saved_ranges.is_empty() {
            let should_scan = if let Some(last_scan) = self.scan_control_state.last_scan_time {
                last_scan.elapsed().as_secs() >= self.scan_control_state.auto_scan_interval_secs
            } else {
                true
            };

            let is_scanning = self.scan_progress.lock().unwrap().scanning;

            if should_scan && !is_scanning {
                self.scan_all_saved_ranges();
            }
        }

        // Request repaint to keep fleet graph updating
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}
