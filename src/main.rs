mod config;
mod models;
mod options;
mod recording;
mod runtime;
mod scanner;
mod ui;

use eframe::egui;
use egui::Color32;
use models::{
    MetricsHistory, MinerInfo, MinerOptionSettings, SavedRange, ScanProgress, SortColumn,
    SortDirection,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ui::ScanControlState;

fn main() -> Result<(), eframe::Error> {
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
    global_options: MinerOptionSettings,
    miner_option_overrides: Arc<Mutex<HashMap<String, MinerOptionSettings>>>,
    miner_options_prefill_pending: Arc<Mutex<HashSet<String>>>,
    prev_global_options: MinerOptionSettings,
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
            global_options: app_config.global_options.clone(),
            miner_option_overrides: Arc::new(Mutex::new(HashMap::new())),
            miner_options_prefill_pending: Arc::new(Mutex::new(HashSet::new())),
            prev_global_options: app_config.global_options,
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

        // Helper function to extract numeric value from a string
        fn extract_numeric(s: &str) -> f64 {
            // First try splitting on whitespace (for "10.5 TH/s")
            if let Some(num_str) = s.split_whitespace().next() {
                if let Ok(val) = num_str.parse::<f64>() {
                    return val;
                }
            }

            // If that fails, try parsing just the leading numeric part (for "25.3°C")
            let numeric_part: String = s
                .chars()
                .take_while(|c| c.is_numeric() || *c == '.' || *c == '-')
                .collect();

            numeric_part.parse::<f64>().unwrap_or(0.0)
        }

        miners.sort_by(|a, b| {
            let cmp = match column {
                SortColumn::Ip => a.ip.cmp(&b.ip),
                SortColumn::Hostname => a.hostname.cmp(&b.hostname),
                SortColumn::Model => a.model.cmp(&b.model),
                SortColumn::Firmware => a.firmware_version.cmp(&b.firmware_version),
                SortColumn::ControlBoard => a.control_board.cmp(&b.control_board),
                SortColumn::ActiveBoards => a
                    .active_boards_count
                    .unwrap_or(0)
                    .cmp(&b.active_boards_count.unwrap_or(0))
                    .then_with(|| {
                        a.total_boards_count
                            .unwrap_or(0)
                            .cmp(&b.total_boards_count.unwrap_or(0))
                    }),
                SortColumn::Hashrate => a
                    .hashrate_th
                    .unwrap_or(extract_numeric(&a.hashrate))
                    .partial_cmp(&b.hashrate_th.unwrap_or(extract_numeric(&b.hashrate)))
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Wattage => a
                    .wattage_w
                    .unwrap_or(extract_numeric(&a.wattage))
                    .partial_cmp(&b.wattage_w.unwrap_or(extract_numeric(&b.wattage)))
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Efficiency => a
                    .efficiency_w_th
                    .unwrap_or(extract_numeric(&a.efficiency))
                    .partial_cmp(&b.efficiency_w_th.unwrap_or(extract_numeric(&b.efficiency)))
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Temperature => a
                    .temperature_c
                    .unwrap_or(extract_numeric(&a.temperature))
                    .partial_cmp(&b.temperature_c.unwrap_or(extract_numeric(&b.temperature)))
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::FanSpeed => a
                    .fan_rpm
                    .unwrap_or(extract_numeric(&a.fan_speed))
                    .partial_cmp(&b.fan_rpm.unwrap_or(extract_numeric(&b.fan_speed)))
                    .unwrap_or(std::cmp::Ordering::Equal),
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
            global_options: self.global_options.clone(),
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
            csv_content.push_str("IP,Hostname,Model,Firmware,Control Board,Active Boards,Hashrate (TH/s),Wattage (W),Efficiency (W/TH),Temperature (°C),Fan Speed (RPM),Pool,Worker\n");

            // Rows
            for miner in miners.iter() {
                csv_content.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    miner.ip,
                    miner.hostname,
                    miner.model,
                    miner.firmware_version,
                    miner.control_board,
                    miner.active_boards,
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
            &self.global_options,
            Arc::clone(&self.miner_option_overrides),
            Arc::clone(&self.miner_options_prefill_pending),
        );

        // Save config if any interval or scan parameter changed
        if self.detail_refresh_interval_secs != self.prev_detail_refresh_interval_secs
            || self.scan_control_state.auto_scan_interval_secs != self.prev_auto_scan_interval_secs
            || self.scan_control_state.identification_timeout_secs
                != self.prev_identification_timeout_secs
            || self.scan_control_state.connectivity_timeout_secs
                != self.prev_connectivity_timeout_secs
            || self.scan_control_state.connectivity_retries != self.prev_connectivity_retries
            || self.global_options != self.prev_global_options
        {
            self.prev_detail_refresh_interval_secs = self.detail_refresh_interval_secs;
            self.prev_auto_scan_interval_secs = self.scan_control_state.auto_scan_interval_secs;
            self.prev_identification_timeout_secs =
                self.scan_control_state.identification_timeout_secs;
            self.prev_connectivity_timeout_secs = self.scan_control_state.connectivity_timeout_secs;
            self.prev_connectivity_retries = self.scan_control_state.connectivity_retries;
            self.prev_global_options = self.global_options.clone();
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

                if !self.error_message.is_empty() {
                    ui.label(
                        egui::RichText::new(&self.error_message)
                            .size(11.0)
                            .color(Color32::from_rgb(255, 120, 120))
                            .monospace(),
                    );
                    ui.add_space(6.0);
                }

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
                    // Top row: two equal columns with explicit gutter
                    let column_gap = 20.0;
                    let total_top_width = ui.available_width();
                    let column_width = (total_top_width - column_gap).max(0.0) / 2.0;

                    let mut on_scan_clicked = false;
                    let mut on_save_range_clicked = false;
                    let mut range_to_remove: Option<usize> = None;
                    let mut range_to_load: Option<SavedRange> = None;
                    let mut apply_global_selected_clicked = false;
                    let mut apply_global_all_clicked = false;

                    let miners_count = self.miners.lock().unwrap().len();
                    let selected_count = self.selected_miners.len();
                    let show_epic_tuning_presets = {
                        let miners = self.miners.lock().unwrap();

                        if selected_count > 0 {
                            let selected_matches: Vec<_> = miners
                                .iter()
                                .filter(|miner| self.selected_miners.contains(&miner.ip))
                                .collect();

                            !selected_matches.is_empty()
                                && selected_matches
                                    .iter()
                                    .all(|miner| miner.is_epic_firmware())
                        } else {
                            !miners.is_empty()
                                && miners.iter().all(|miner| miner.is_epic_firmware())
                        }
                    };

                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(column_width, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                let miners = self.miners.lock().unwrap();
                                ui::draw_stats_card(ui, &miners);
                                ui.add_space(12.0);
                                ui::draw_global_options_card(
                                    ui,
                                    &mut self.global_options,
                                    selected_count,
                                    miners_count,
                                    show_epic_tuning_presets,
                                    &mut apply_global_selected_clicked,
                                    &mut apply_global_all_clicked,
                                );
                            },
                        );

                        ui.add_space(column_gap);

                        ui.allocate_ui_with_layout(
                            egui::vec2(column_width, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
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
                            },
                        );
                    });

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
                    if apply_global_selected_clicked {
                        let selected_ips: Vec<String> =
                            self.selected_miners.iter().cloned().collect();
                        if !self.global_options.has_any_enabled() {
                            self.error_message =
                                "Enable at least one global option before applying".to_string();
                        } else if let Some(message) =
                            self.global_options.tuning_validation_message()
                        {
                            self.error_message = message;
                        } else if let Some(message) = self.global_options.pool_validation_message()
                        {
                            self.error_message = message;
                        } else if selected_ips.is_empty() {
                            self.error_message = "No selected miners to apply options".to_string();
                        } else {
                            self.error_message.clear();
                            let settings = self.global_options.clone();
                            runtime::spawn(async move {
                                options::apply_options_to_many(selected_ips, settings).await;
                            });
                        }
                    }
                    if apply_global_all_clicked {
                        let all_ips: Vec<String> = self
                            .miners
                            .lock()
                            .unwrap()
                            .iter()
                            .map(|m| m.ip.clone())
                            .collect();
                        if !self.global_options.has_any_enabled() {
                            self.error_message =
                                "Enable at least one global option before applying".to_string();
                        } else if let Some(message) =
                            self.global_options.tuning_validation_message()
                        {
                            self.error_message = message;
                        } else if let Some(message) = self.global_options.pool_validation_message()
                        {
                            self.error_message = message;
                        } else if all_ips.is_empty() {
                            self.error_message =
                                "No discovered miners to apply options".to_string();
                        } else {
                            self.error_message.clear();
                            let settings = self.global_options.clone();
                            runtime::spawn(async move {
                                options::apply_options_to_many(all_ips, settings).await;
                            });
                        }
                    }

                    ui.add_space(15.0);

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
