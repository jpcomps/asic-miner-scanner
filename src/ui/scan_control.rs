use crate::models::{
    FanModeSelection, MinerOptionSettings, MiningModeSelection, PoolInput, SavedRange,
    ScanProgress, TuningTargetSelection, EPIC_TUNING_ALGO_OPTIONS, HASHRATE_ALGO_OPTIONS,
};
use eframe::egui;
use egui::{Color32, FontId, Vec2};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct ScanControlState {
    pub ip_range_start: String,
    pub ip_range_end: String,
    pub new_range_name: String,
    pub auto_scan_enabled: bool,
    pub auto_scan_interval_secs: u64,
    pub last_scan_time: Option<Instant>,
    pub identification_timeout_secs: u64,
    pub connectivity_timeout_secs: u64,
    pub connectivity_retries: u32,
    pub show_name_error: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn draw_global_options_card(
    ui: &mut egui::Ui,
    global_options: &mut MinerOptionSettings,
    selected_count: usize,
    miner_count: usize,
    show_epic_tuning_presets: bool,
    on_apply_global_selected_clicked: &mut bool,
    on_apply_global_all_clicked: &mut bool,
) {
    let target_inner_width = (ui.available_width() - 30.0).max(0.0);
    egui::Frame::new()
        .fill(Color32::from_rgb(28, 28, 28))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
        .corner_radius(4.0)
        .inner_margin(15.0)
        .show(ui, |ui| {
            ui.set_width(target_inner_width);
            ui.set_max_width(target_inner_width);
            ui.label(
                egui::RichText::new("GLOBAL OPTIONS")
                    .size(13.0)
                    .color(Color32::from_rgb(240, 240, 240))
                    .strong()
                    .monospace(),
            );
            ui.label(
                egui::RichText::new("Apply shared defaults to selected miners or the full fleet")
                    .size(10.0)
                    .color(Color32::from_rgb(130, 130, 130)),
            );

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut global_options.apply_power_limit, "");
                ui.label("Power");
                ui.add(
                    egui::DragValue::new(&mut global_options.power_limit_watts)
                        .range(100.0..=9000.0)
                        .speed(25.0)
                        .suffix(" W"),
                );

                ui.add_space(8.0);

                ui.checkbox(&mut global_options.apply_tuning_config, "");
                ui.label("Tuning");
                egui::ComboBox::from_id_salt("global_tuning_target")
                    .selected_text(match global_options.tuning_target {
                        TuningTargetSelection::MiningMode => "Mode",
                        TuningTargetSelection::Power => "Power",
                        TuningTargetSelection::Hashrate => "Hashrate",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut global_options.tuning_target,
                            TuningTargetSelection::MiningMode,
                            "Mode",
                        );
                        ui.selectable_value(
                            &mut global_options.tuning_target,
                            TuningTargetSelection::Power,
                            "Power",
                        );
                        ui.selectable_value(
                            &mut global_options.tuning_target,
                            TuningTargetSelection::Hashrate,
                            "Hashrate",
                        );
                    });
            });

            ui.add_space(6.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("Tuning Target");

                match global_options.tuning_target {
                    TuningTargetSelection::MiningMode => {
                        egui::ComboBox::from_id_salt("global_mining_mode")
                            .selected_text(match global_options.mining_mode {
                                MiningModeSelection::Low => "Low",
                                MiningModeSelection::Normal => "Normal",
                                MiningModeSelection::High => "High",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut global_options.mining_mode,
                                    MiningModeSelection::Low,
                                    "Low",
                                );
                                ui.selectable_value(
                                    &mut global_options.mining_mode,
                                    MiningModeSelection::Normal,
                                    "Normal",
                                );
                                ui.selectable_value(
                                    &mut global_options.mining_mode,
                                    MiningModeSelection::High,
                                    "High",
                                );
                            });
                    }
                    TuningTargetSelection::Power => {
                        ui.add(
                            egui::DragValue::new(&mut global_options.tuning_power_watts)
                                .range(100.0..=9000.0)
                                .speed(25.0)
                                .suffix(" W"),
                        );
                    }
                    TuningTargetSelection::Hashrate => {
                        ui.add(
                            egui::DragValue::new(&mut global_options.tuning_hashrate_ths)
                                .range(1.0..=5000.0)
                                .speed(5.0)
                                .suffix(" TH/s"),
                        );
                        egui::ComboBox::from_id_salt("global_tuning_hashrate_algo")
                            .selected_text(global_options.tuning_hashrate_algo.clone())
                            .show_ui(ui, |ui| {
                                for algo in HASHRATE_ALGO_OPTIONS {
                                    ui.selectable_value(
                                        &mut global_options.tuning_hashrate_algo,
                                        algo.to_string(),
                                        algo,
                                    );
                                }
                            });
                        ui.add(
                            egui::TextEdit::singleline(&mut global_options.tuning_hashrate_algo)
                                .desired_width(110.0)
                                .hint_text("custom algo"),
                        );
                    }
                }

                ui.label("Algo");
                if show_epic_tuning_presets {
                    egui::ComboBox::from_id_salt("global_tuning_algorithm")
                        .selected_text(if global_options.tuning_algorithm.trim().is_empty() {
                            "None".to_string()
                        } else {
                            global_options.tuning_algorithm.clone()
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut global_options.tuning_algorithm,
                                String::new(),
                                "None",
                            );
                            for algo in EPIC_TUNING_ALGO_OPTIONS {
                                ui.selectable_value(
                                    &mut global_options.tuning_algorithm,
                                    algo.to_string(),
                                    algo,
                                );
                            }
                        });
                }
                ui.add(
                    egui::TextEdit::singleline(&mut global_options.tuning_algorithm)
                        .desired_width(120.0)
                        .hint_text(if show_epic_tuning_presets {
                            "custom override"
                        } else {
                            "custom tuning algo"
                        }),
                );
            });

            if global_options.apply_tuning_config {
                if let Some(message) = global_options.tuning_validation_message() {
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", message))
                            .size(10.0)
                            .color(Color32::from_rgb(255, 120, 120)),
                    );
                }
            }

            ui.add_space(6.0);

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut global_options.apply_fan_config, "");
                ui.label("Fan");
                egui::ComboBox::from_id_salt("global_fan_mode")
                    .selected_text(match global_options.fan_mode {
                        FanModeSelection::Auto => "Auto",
                        FanModeSelection::Manual => "Manual",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut global_options.fan_mode,
                            FanModeSelection::Auto,
                            "Auto",
                        );
                        ui.selectable_value(
                            &mut global_options.fan_mode,
                            FanModeSelection::Manual,
                            "Manual",
                        );
                    });

                match global_options.fan_mode {
                    FanModeSelection::Auto => {
                        ui.add(
                            egui::DragValue::new(&mut global_options.fan_target_temp_c)
                                .range(30.0..=95.0)
                                .speed(0.5)
                                .suffix(" C"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut global_options.fan_idle_speed_percent)
                                .range(0..=100)
                                .speed(1)
                                .suffix(" % idle"),
                        );
                    }
                    FanModeSelection::Manual => {
                        ui.add(
                            egui::DragValue::new(&mut global_options.fan_speed_percent)
                                .range(0..=100)
                                .speed(1)
                                .suffix(" %"),
                        );
                    }
                }
            });

            ui.add_space(6.0);

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut global_options.apply_scaling_config, "");
                ui.label("Scaling");
                ui.add(
                    egui::DragValue::new(&mut global_options.scaling_step)
                        .range(1..=100)
                        .speed(1)
                        .prefix("step "),
                );
                ui.add(
                    egui::DragValue::new(&mut global_options.scaling_minimum)
                        .range(0..=100)
                        .speed(1)
                        .prefix("min "),
                );
                ui.checkbox(&mut global_options.scaling_shutdown, "shutdown");
                ui.add_enabled(
                    global_options.scaling_shutdown,
                    egui::DragValue::new(&mut global_options.scaling_shutdown_duration)
                        .range(1.0..=300.0)
                        .speed(1.0)
                        .suffix(" s"),
                );
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut global_options.apply_pool_config, "");
                ui.label("Pools");
                ui.label("Group");
                ui.add(
                    egui::TextEdit::singleline(&mut global_options.pool_group_name)
                        .desired_width(110.0)
                        .hint_text("Primary"),
                );
                ui.add(
                    egui::DragValue::new(&mut global_options.pool_group_quota)
                        .range(1..=100)
                        .suffix(" %"),
                );
            });

            let mut remove_idx: Option<usize> = None;
            let can_remove_pool = global_options.pool_inputs.len() > 1;
            let pool_url_width = (ui.available_width() * 0.47).clamp(140.0, 260.0);
            let pool_user_width = (ui.available_width() * 0.28).clamp(100.0, 160.0);
            let pool_pass_width = 58.0;

            for (idx, pool) in global_options.pool_inputs.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{}.", idx + 1));
                    ui.add(
                        egui::TextEdit::singleline(&mut pool.url)
                            .desired_width(pool_url_width)
                            .hint_text("stratum+tcp://pool:3333"),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut pool.username)
                            .desired_width(pool_user_width)
                            .hint_text("wallet.worker"),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut pool.password)
                            .desired_width(pool_pass_width)
                            .hint_text("x"),
                    );

                    if ui
                        .add_enabled(can_remove_pool, egui::Button::new("-"))
                        .clicked()
                    {
                        remove_idx = Some(idx);
                    }
                });
            }

            if let Some(idx) = remove_idx {
                global_options.pool_inputs.remove(idx);
            }

            ui.horizontal_wrapped(|ui| {
                if ui.button("+ Add Pool").clicked() {
                    global_options.pool_inputs.push(PoolInput::default());
                }
                ui.label(
                    egui::RichText::new(format!("{} configured", global_options.pool_inputs.len()))
                        .size(10.0)
                        .color(Color32::from_rgb(130, 130, 130)),
                );
            });

            if global_options.apply_pool_config {
                if let Some(message) = global_options.pool_validation_message() {
                    ui.label(
                        egui::RichText::new(format!("⚠ {}", message))
                            .size(10.0)
                            .color(Color32::from_rgb(255, 120, 120)),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("✓ Pool config looks valid")
                            .size(10.0)
                            .color(Color32::from_rgb(120, 200, 120)),
                    );
                }
            }

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        selected_count > 0,
                        egui::Button::new(
                            egui::RichText::new(format!("Apply to Selected ({selected_count})"))
                                .size(11.0)
                                .color(Color32::WHITE)
                                .monospace(),
                        )
                        .fill(Color32::from_rgb(70, 130, 180))
                        .corner_radius(5.0),
                    )
                    .clicked()
                {
                    *on_apply_global_selected_clicked = true;
                }

                if ui
                    .add_enabled(
                        miner_count > 0,
                        egui::Button::new(
                            egui::RichText::new(format!("Apply to All ({miner_count})"))
                                .size(11.0)
                                .color(Color32::WHITE)
                                .monospace(),
                        )
                        .fill(Color32::from_rgb(100, 160, 100))
                        .corner_radius(5.0),
                    )
                    .clicked()
                {
                    *on_apply_global_all_clicked = true;
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
pub fn draw_scan_and_ranges_card(
    ui: &mut egui::Ui,
    state: &mut ScanControlState,
    saved_ranges: &[SavedRange],
    scan_progress: Arc<Mutex<ScanProgress>>,
    on_scan_clicked: &mut bool,
    on_save_range_clicked: &mut bool,
    range_to_remove: &mut Option<usize>,
    range_to_load: &mut Option<SavedRange>,
) {
    let target_inner_width = (ui.available_width() - 30.0).max(0.0);
    // Get progress info early, then drop the lock
    let (is_scanning, scanned_ranges, total_ranges, found_miners, scan_elapsed) = {
        let progress = scan_progress.lock().unwrap();
        let elapsed = if progress.scanning {
            progress
                .scan_start_time
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0)
        } else {
            progress.scan_duration_secs
        };
        (
            progress.scanning,
            progress.scanned_ranges,
            progress.total_ranges,
            progress.found_miners,
            elapsed,
        )
    };

    egui::Frame::new()
        .fill(Color32::from_rgb(28, 28, 28))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
        .corner_radius(4.0)
        .inner_margin(15.0)
        .show(ui, |ui| {
            ui.set_width(target_inner_width);
            ui.set_max_width(target_inner_width);
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
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut state.auto_scan_enabled, "")
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

                    let interval_mins = (state.auto_scan_interval_secs / 60) as i32;
                    let mut temp_interval = interval_mins;
                    if ui
                        .add(
                            egui::DragValue::new(&mut temp_interval)
                                .suffix(" min")
                                .speed(1),
                        )
                        .changed()
                    {
                        state.auto_scan_interval_secs = (temp_interval.max(1) * 60) as u64;
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

                    if ui.add_enabled(!saved_ranges.is_empty(), scan_btn).clicked() {
                        *on_scan_clicked = true;
                    }

                    // Show last scan time
                    if let Some(last_scan) = state.last_scan_time {
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

                // Show progress bar and stats only when there are saved ranges
                if !saved_ranges.is_empty() {
                    // Progress bar
                    let progress_fraction = if total_ranges > 0 {
                        scanned_ranges as f32 / total_ranges as f32
                    } else {
                        0.0
                    };

                    let progress_bar = egui::ProgressBar::new(progress_fraction)
                        .fill(Color32::from_rgb(255, 87, 51))
                        .show_percentage();
                    ui.add(progress_bar);

                    ui.add_space(5.0);

                    // Scan statistics
                    if is_scanning {
                        ui.label(
                            egui::RichText::new(format!(
                                "⏳ Scanning range {}/{} | Found: {} miners | Time: {}s",
                                scanned_ranges, total_ranges, found_miners, scan_elapsed
                            ))
                            .size(10.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                        );
                    } else if total_ranges > 0 {
                        ui.label(
                            egui::RichText::new(format!(
                                "✓ Last scan: {} miners found in {}s",
                                found_miners, scan_elapsed
                            ))
                            .size(10.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new("Ready to scan")
                                .size(10.0)
                                .color(Color32::from_rgb(160, 160, 160))
                                .monospace(),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new(
                            "No ranges configured - add a range below to begin scanning",
                        )
                        .size(10.0)
                        .color(Color32::from_rgb(160, 160, 160))
                        .monospace(),
                    );
                }

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(15.0);

                // Scan Parameters
                ui.label(
                    egui::RichText::new("SCAN PARAMETERS:")
                        .size(11.0)
                        .color(Color32::from_rgb(180, 180, 180))
                        .monospace(),
                );
                ui.add_space(5.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("ID TIMEOUT:")
                            .size(11.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                    );
                    ui.add_space(5.0);
                    let mut id_timeout = state.identification_timeout_secs as i32;
                    if ui
                        .add(egui::DragValue::new(&mut id_timeout).suffix(" s").speed(1))
                        .changed()
                    {
                        state.identification_timeout_secs = id_timeout.max(1) as u64;
                    }

                    ui.add_space(15.0);

                    ui.label(
                        egui::RichText::new("CONN TIMEOUT:")
                            .size(11.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                    );
                    ui.add_space(5.0);
                    let mut conn_timeout = state.connectivity_timeout_secs as i32;
                    if ui
                        .add(
                            egui::DragValue::new(&mut conn_timeout)
                                .suffix(" s")
                                .speed(1),
                        )
                        .changed()
                    {
                        state.connectivity_timeout_secs = conn_timeout.max(1) as u64;
                    }

                    ui.add_space(15.0);

                    ui.label(
                        egui::RichText::new("RETRIES:")
                            .size(11.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                    );
                    ui.add_space(5.0);
                    let mut retries = state.connectivity_retries as i32;
                    if ui
                        .add(egui::DragValue::new(&mut retries).speed(1))
                        .changed()
                    {
                        state.connectivity_retries = retries.max(0) as u32;
                    }
                });

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

                    let ip_width = (ui.available_width() * 0.36).clamp(120.0, 180.0);

                    let text_edit = egui::TextEdit::singleline(&mut state.ip_range_start)
                        .font(FontId::monospace(12.0))
                        .desired_width(ip_width);
                    ui.add(text_edit);

                    ui.add_space(20.0);

                    ui.label(
                        egui::RichText::new("END IP:")
                            .size(11.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                    );

                    ui.add_space(10.0);

                    let text_edit = egui::TextEdit::singleline(&mut state.ip_range_end)
                        .font(FontId::monospace(12.0))
                        .desired_width(ip_width);
                    ui.add(text_edit);
                });

                ui.add_space(10.0);

                // Add new range
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("NAME:")
                            .size(11.0)
                            .color(Color32::from_rgb(160, 160, 160))
                            .monospace(),
                    );

                    ui.add_space(10.0);

                    let mut text_edit = egui::TextEdit::singleline(&mut state.new_range_name)
                        .font(FontId::monospace(12.0))
                        .desired_width((ui.available_width() * 0.42).clamp(120.0, 200.0))
                        .hint_text("e.g. Main Site");

                    // Show error styling if validation failed
                    if state.show_name_error {
                        text_edit = text_edit.text_color(Color32::from_rgb(255, 100, 100));
                    }

                    let response = ui.add(text_edit);

                    // Clear error when user starts typing
                    if response.changed() && !state.new_range_name.is_empty() {
                        state.show_name_error = false;
                    }

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
                        if state.new_range_name.trim().is_empty() {
                            state.show_name_error = true;
                        } else {
                            state.show_name_error = false;
                            *on_save_range_clicked = true;
                        }
                    }
                });

                // Show error message if name is empty
                if state.show_name_error {
                    ui.label(
                        egui::RichText::new("⚠ Range name is required")
                            .size(10.0)
                            .color(Color32::from_rgb(255, 100, 100))
                            .monospace(),
                    );
                }

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(15.0);

                // Show saved ranges
                if !saved_ranges.is_empty() {
                    ui.label(
                        egui::RichText::new("SAVED RANGES:")
                            .size(11.0)
                            .color(Color32::from_rgb(180, 180, 180))
                            .monospace(),
                    );
                    ui.add_space(5.0);
                }

                egui::ScrollArea::vertical()
                    .id_salt("saved_ranges_scroll")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for (idx, range) in saved_ranges.iter().enumerate() {
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
                                            *range_to_remove = Some(idx);
                                        }

                                        if ui
                                            .button(
                                                egui::RichText::new("Load")
                                                    .color(Color32::from_rgb(100, 200, 255)),
                                            )
                                            .clicked()
                                        {
                                            *range_to_load = Some(range.clone());
                                        }
                                    },
                                );
                            });
                        }
                    });
            });
        });
}
