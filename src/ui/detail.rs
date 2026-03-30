use crate::models::{
    FanModeSelection, MetricsHistory, MinerInfo, MinerOptionSettings, MiningModeSelection,
    PoolInput, RecordingState, TuningTargetSelection, EPIC_TUNING_ALGO_OPTIONS,
    HASHRATE_ALGO_OPTIONS,
};
use crate::options;
use asic_rs::MinerFactory;
use asic_rs_core::data::hashrate::{HashRate, HashRateUnit};
use asic_rs_core::data::miner::MinerData;
use eframe::egui;
use egui::Color32;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[allow(clippy::too_many_arguments)]
pub fn draw_miner_detail_modal(
    ctx: &egui::Context,
    detail_view_miners: &mut Vec<MinerInfo>,
    miners_arc: Arc<Mutex<Vec<MinerInfo>>>,
    detail_refresh_times: &mut HashMap<String, Instant>,
    detail_graph_update_times: &mut HashMap<String, Instant>,
    detail_metrics_history: &mut HashMap<String, MetricsHistory>,
    recording_states: &mut HashMap<String, RecordingState>,
    detail_refresh_interval_secs: &mut u64,
    global_options: &MinerOptionSettings,
    miner_option_overrides: Arc<Mutex<HashMap<String, MinerOptionSettings>>>,
    miner_options_prefill_pending: Arc<Mutex<HashSet<String>>>,
) {
    let mut miners_to_close = Vec::new();

    for (idx, detail_miner) in detail_view_miners.iter().enumerate() {
        let mut is_open = true;

        // Get the latest data from the main miners list
        let miners_list = miners_arc.lock().unwrap();
        let current_miner = miners_list.iter().find(|m| m.ip == detail_miner.ip);

        if let Some(miner) = current_miner {
            egui::Window::new(
                egui::RichText::new(format!("🔍 Miner Details - {} - {}", miner.ip, miner.model))
                    .size(12.0)
                    .monospace(),
            )
            .id(egui::Id::new(format!("detail_modal_{}", miner.ip)))
            .default_width(900.0)
            .default_height(600.0)
            .min_width(800.0)
            .min_height(600.0)
            .resizable(true)
            .collapsible(true)
            .open(&mut is_open)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    // Left column - Basic Information (1/3 width)
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width() * 0.33);
                        egui::ScrollArea::vertical()
                            .id_salt(format!("left_scroll_{}", miner.ip))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if let Some(data) = &miner.full_data {
                                    draw_basic_info(ui, data);
                                    ui.add_space(15.0);
                                    draw_performance_info(ui, data);
                                    ui.add_space(15.0);
                                    draw_fans_info(ui, data);
                                    ui.add_space(15.0);
                                    draw_pools_info(ui, data);
                                    ui.add_space(15.0);
                                    draw_hashboards_info(ui, data);
                                } else {
                                    ui.label("No detailed data available");
                                }
                            });
                    });

                    // Right column - Controls and Graphs (2/3 width)
                    ui.vertical(|ui| {
                        egui::ScrollArea::vertical()
                            .id_salt(format!("right_scroll_{}", miner.ip))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                draw_controls_and_graphs(
                                    ui,
                                    miner,
                                    miners_arc.clone(),
                                    detail_refresh_times,
                                    detail_graph_update_times,
                                    detail_metrics_history,
                                    recording_states,
                                    detail_refresh_interval_secs,
                                    global_options,
                                    Arc::clone(&miner_option_overrides),
                                    Arc::clone(&miner_options_prefill_pending),
                                );
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
        let miner = detail_view_miners.remove(*idx);
        // Clean up history for this miner
        detail_metrics_history.remove(&miner.ip);
        detail_refresh_times.remove(&miner.ip);
        detail_graph_update_times.remove(&miner.ip);
        miner_option_overrides.lock().unwrap().remove(&miner.ip);
        miner_options_prefill_pending
            .lock()
            .unwrap()
            .remove(&miner.ip);

        // Delete recording file if it exists
        if let Some(recording) = recording_states.remove(&miner.ip) {
            if let Err(e) = crate::recording::delete_recording(&recording) {
                eprintln!("✗ Failed to delete recording for {}: {}", miner.ip, e);
            }
        }
    }
}

fn hashrate_to_terahash(hashrate: Option<&HashRate>) -> Option<f64> {
    hashrate
        .cloned()
        .map(|value| value.as_unit(HashRateUnit::TeraHash).value)
}

fn draw_basic_info(ui: &mut egui::Ui, data: &MinerData) {
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
            ui.label(
                data.mac
                    .map(|m| m.to_string())
                    .unwrap_or_else(|| "N/A".to_string()),
            );
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
            ui.label(
                data.control_board_version
                    .as_ref()
                    .map(|cb| cb.name.clone())
                    .unwrap_or_else(|| "N/A".to_string()),
            );
            ui.end_row();

            ui.label(egui::RichText::new("Is Mining:").strong());
            ui.label(if data.is_mining { "Yes" } else { "No" });
            ui.end_row();

            ui.label(egui::RichText::new("Light Flashing:").strong());
            ui.label(if data.light_flashing.unwrap_or(false) {
                "Yes"
            } else {
                "No"
            });
            ui.end_row();

            ui.label(egui::RichText::new("Uptime:").strong());
            if let Some(uptime) = data.uptime {
                ui.label(format!("{} seconds", uptime.as_secs()));
            } else {
                ui.label("N/A");
            }
            ui.end_row();
        });
}

fn draw_performance_info(ui: &mut egui::Ui, data: &MinerData) {
    ui.heading("Performance");
    ui.separator();
    ui.add_space(5.0);

    egui::Grid::new("performance_grid")
        .num_columns(2)
        .spacing([40.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Hashrate:").strong());
            if let Some(hashrate) = hashrate_to_terahash(data.hashrate.as_ref()) {
                ui.label(format!("{hashrate:.2} TH/s"));
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
}

fn draw_fans_info(ui: &mut egui::Ui, data: &MinerData) {
    ui.heading("Fans");
    ui.separator();
    ui.add_space(5.0);

    if !data.fans.is_empty() {
        for (i, fan) in data.fans.iter().enumerate() {
            ui.label(egui::RichText::new(format!("Fan {}:", i + 1)).strong());
            ui.indent(format!("fan_{i}"), |ui| {
                if let Some(rpm) = fan.rpm {
                    let rpm_value =
                        rpm.as_radians_per_second() * 60.0 / (2.0 * std::f64::consts::PI);
                    ui.label(format!("Speed: {rpm_value:.0} RPM"));
                }
            });
        }
    } else {
        ui.label("No fan data available");
    }
}

fn draw_pools_info(ui: &mut egui::Ui, data: &MinerData) {
    ui.heading("Pools");
    ui.separator();
    ui.add_space(5.0);

    if !data.pools.is_empty() {
        for (group_idx, group) in data.pools.iter().enumerate() {
            ui.label(
                egui::RichText::new(format!(
                    "Group {}: {} (quota {}%)",
                    group_idx + 1,
                    group.name,
                    group.quota
                ))
                .strong(),
            );

            if group.pools.is_empty() {
                ui.indent(format!("pool_group_{group_idx}"), |ui| {
                    ui.label("No pools configured in this group");
                });
                ui.add_space(5.0);
                continue;
            }

            for (pool_idx, pool) in group.pools.iter().enumerate() {
                ui.indent(format!("pool_group_{group_idx}_pool_{pool_idx}"), |ui| {
                    ui.label(egui::RichText::new(format!("Pool {}", pool_idx + 1)).strong());

                    if let Some(url) = &pool.url {
                        ui.label(format!("URL: {url}"));
                    }

                    if let Some(user) = &pool.user {
                        ui.label(format!("User: {user}"));
                    }

                    ui.label(format!(
                        "Active: {}",
                        if pool.active.unwrap_or(false) {
                            "Yes"
                        } else {
                            "No"
                        }
                    ));

                    if let Some(alive) = pool.alive {
                        ui.label(format!("Alive: {}", if alive { "Yes" } else { "No" }));
                    }
                });
            }

            ui.add_space(5.0);
        }
    } else {
        ui.label("No pool data available");
    }
}

fn draw_hashboards_info(ui: &mut egui::Ui, data: &MinerData) {
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
                    if let Some(value) = hashrate_to_terahash(Some(hashrate)) {
                        ui.label(format!("Hashrate: {value:.2} TH/s"));
                    }
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
}

#[allow(clippy::too_many_arguments)]
fn draw_controls_and_graphs(
    ui: &mut egui::Ui,
    miner: &MinerInfo,
    miners_arc: Arc<Mutex<Vec<MinerInfo>>>,
    detail_refresh_times: &mut HashMap<String, Instant>,
    detail_graph_update_times: &mut HashMap<String, Instant>,
    detail_metrics_history: &mut HashMap<String, MetricsHistory>,
    recording_states: &mut HashMap<String, RecordingState>,
    detail_refresh_interval_secs: &mut u64,
    global_options: &MinerOptionSettings,
    miner_option_overrides: Arc<Mutex<HashMap<String, MinerOptionSettings>>>,
    miner_options_prefill_pending: Arc<Mutex<HashSet<String>>>,
) {
    // Graph rolling update logic (every ~33ms for 30fps smooth rolling)
    let should_update_graph = if let Some(last_update) = detail_graph_update_times.get(&miner.ip) {
        last_update.elapsed().as_millis() >= 33
    } else {
        true
    };

    // Auto-refresh logic (fetches new data from miner - for real data points and CSV recording)
    let last_refresh_time = detail_refresh_times.get(&miner.ip).cloned();
    let should_auto_refresh = if let Some(last_time) = last_refresh_time {
        last_time.elapsed().as_secs() >= *detail_refresh_interval_secs
    } else {
        true // First time, refresh immediately
    };

    if should_auto_refresh {
        let ip = miner.ip.clone();
        let miners = Arc::clone(&miners_arc);
        crate::runtime::spawn(async move {
            let factory = MinerFactory::new();
            if let Ok(Some(miner_obj)) = factory.get_miner(ip.parse().unwrap()).await {
                let data = miner_obj.get_data().await;

                let mut miners_list = miners.lock().unwrap();
                if let Some(existing) = miners_list.iter_mut().find(|m| m.ip == ip) {
                    existing.full_data = Some(data);
                }
            }
        });
        detail_refresh_times.insert(miner.ip.clone(), Instant::now());
    }

    // Add data point to history for smooth 30fps rolling graph
    if should_update_graph {
        if let Some(data) = &miner.full_data {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs_f64();

            let total_hashrate = if let Some(hr) = &data.hashrate {
                hashrate_to_terahash(Some(hr)).unwrap_or(0.0)
            } else {
                0.0
            };

            let power = if let Some(wattage) = data.wattage {
                wattage.as_watts()
            } else {
                0.0
            };

            // Collect per-board hashrates
            let board_hashrates: Vec<f64> = data
                .hashboards
                .iter()
                .filter_map(|board| hashrate_to_terahash(board.hashrate.as_ref()))
                .collect();

            // Collect per-board temperatures and calculate average
            let board_temps: Vec<f64> = data
                .hashboards
                .iter()
                .filter_map(|board| board.board_temperature.map(|temp| temp.as_celsius()))
                .collect();

            let avg_temp = if !board_temps.is_empty() {
                board_temps.iter().sum::<f64>() / board_temps.len() as f64
            } else {
                0.0
            };

            let history = detail_metrics_history.entry(miner.ip.clone()).or_default();

            history.push_back((
                timestamp,
                total_hashrate,
                power,
                board_hashrates,
                avg_temp,
                board_temps,
            ));

            // Keep only last 9000 points (~5 minutes at 30fps for smooth rolling)
            if history.len() > 9000 {
                history.pop_front();
            }
        }

        detail_graph_update_times.insert(miner.ip.clone(), Instant::now());
    }

    // CSV recording - only on actual refresh (real data points)
    if should_auto_refresh {
        if let Some(recording) = recording_states.get_mut(&miner.ip) {
            if recording.is_recording {
                if let Err(e) = crate::recording::append_data_point(recording, miner) {
                    eprintln!("✗ Failed to append recording data: {e}");
                }
            }
        }
    }

    ui.horizontal(|ui| {
        // Display last refresh time
        if let Some(last_time) = last_refresh_time {
            let elapsed = last_time.elapsed().as_secs();
            ui.label(
                egui::RichText::new(format!("Last updated: {elapsed}s ago"))
                    .size(10.0)
                    .color(Color32::from_rgb(120, 120, 120)),
            );
        }

        ui.add_space(10.0);

        // Manual refresh button
        if ui
            .button(egui::RichText::new("🔄 Refresh").color(Color32::WHITE))
            .clicked()
        {
            let ip = miner.ip.clone();
            let miners = Arc::clone(&miners_arc);
            crate::runtime::spawn(async move {
                let factory = MinerFactory::new();
                if let Ok(Some(miner_obj)) = factory.get_miner(ip.parse().unwrap()).await {
                    let data = miner_obj.get_data().await;

                    let mut miners_list = miners.lock().unwrap();
                    if let Some(existing) = miners_list.iter_mut().find(|m| m.ip == ip) {
                        existing.full_data = Some(data);
                    }
                }
            });
            detail_refresh_times.insert(miner.ip.clone(), Instant::now());
        }

        ui.add_space(15.0);

        // Refresh interval slider
        ui.label(
            egui::RichText::new("Auto-refresh interval:")
                .size(10.0)
                .color(Color32::from_rgb(120, 120, 120)),
        );
        ui.add(
            egui::Slider::new(detail_refresh_interval_secs, 5..=60)
                .suffix("s")
                .text(""),
        );
    });

    ui.add_space(10.0);

    draw_miner_options_panel(
        ui,
        miner,
        global_options,
        Arc::clone(&miner_option_overrides),
        Arc::clone(&miner_options_prefill_pending),
    );

    ui.add_space(10.0);

    // Web interface and control buttons
    draw_control_buttons(
        ui,
        miner,
        miners_arc,
        detail_refresh_times,
        recording_states,
    );

    // Draw graphs
    draw_metrics_graphs(ui, miner, detail_metrics_history);
}

fn draw_miner_options_panel(
    ui: &mut egui::Ui,
    miner: &MinerInfo,
    global_options: &MinerOptionSettings,
    miner_option_overrides: Arc<Mutex<HashMap<String, MinerOptionSettings>>>,
    miner_options_prefill_pending: Arc<Mutex<HashSet<String>>>,
) {
    ui.heading("Miner Options");
    ui.add_space(4.0);
    let show_epic_tuning_presets = miner.is_epic_firmware();

    let should_try_prefill = {
        let has_override = miner_option_overrides
            .lock()
            .unwrap()
            .contains_key(&miner.ip);
        let mut pending = miner_options_prefill_pending.lock().unwrap();
        if !has_override && !pending.contains(&miner.ip) {
            pending.insert(miner.ip.clone());
            true
        } else {
            false
        }
    };

    if should_try_prefill {
        let ip = miner.ip.clone();
        let defaults = global_options.clone();
        let overrides = Arc::clone(&miner_option_overrides);
        let pending = Arc::clone(&miner_options_prefill_pending);
        crate::runtime::spawn(async move {
            if let Ok(current) = options::fetch_current_options(ip.clone(), defaults).await {
                overrides.lock().unwrap().insert(ip.clone(), current);
            }
            pending.lock().unwrap().remove(&ip);
        });
    }

    let mut options_state = miner_option_overrides
        .lock()
        .unwrap()
        .get(&miner.ip)
        .cloned()
        .unwrap_or_else(|| global_options.clone());

    ui.horizontal_wrapped(|ui| {
        let capability_chip = |ui: &mut egui::Ui, label: &str, supported: bool| {
            let color = if supported {
                Color32::from_rgb(90, 170, 100)
            } else {
                Color32::from_rgb(140, 90, 90)
            };
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}",
                    label,
                    if supported { "YES" } else { "NO" }
                ))
                .size(10.0)
                .color(color)
                .monospace(),
            );
        };

        capability_chip(ui, "POWER", miner.capabilities.set_power_limit);
        capability_chip(ui, "FAN", miner.capabilities.fan_config);
        capability_chip(ui, "TUNING", miner.capabilities.tuning_config);
        capability_chip(ui, "SCALING", miner.capabilities.scaling_config);
        capability_chip(ui, "POOLS", miner.capabilities.pools_config);
    });

    ui.add_space(6.0);

    ui.horizontal(|ui| {
        ui.checkbox(&mut options_state.apply_power_limit, "");
        ui.label("Power");
        ui.add_enabled(
            miner.capabilities.set_power_limit,
            egui::DragValue::new(&mut options_state.power_limit_watts)
                .range(100.0..=9000.0)
                .speed(25.0)
                .suffix(" W"),
        );
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut options_state.apply_fan_config, "");
        ui.label("Fan");

        ui.add_enabled_ui(miner.capabilities.fan_config, |ui| {
            egui::ComboBox::from_id_salt(format!("fan_mode_{}", miner.ip))
                .selected_text(match options_state.fan_mode {
                    FanModeSelection::Auto => "Auto",
                    FanModeSelection::Manual => "Manual",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut options_state.fan_mode,
                        FanModeSelection::Auto,
                        "Auto",
                    );
                    ui.selectable_value(
                        &mut options_state.fan_mode,
                        FanModeSelection::Manual,
                        "Manual",
                    );
                });

            match options_state.fan_mode {
                FanModeSelection::Auto => {
                    ui.add(
                        egui::DragValue::new(&mut options_state.fan_target_temp_c)
                            .range(30.0..=95.0)
                            .speed(0.5)
                            .suffix(" C"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut options_state.fan_idle_speed_percent)
                            .range(0..=100)
                            .speed(1)
                            .suffix(" % idle"),
                    );
                }
                FanModeSelection::Manual => {
                    ui.add(
                        egui::DragValue::new(&mut options_state.fan_speed_percent)
                            .range(0..=100)
                            .speed(1)
                            .suffix(" %"),
                    );
                }
            }
        });
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut options_state.apply_tuning_config, "");
        ui.label("Tuning");

        ui.add_enabled_ui(miner.capabilities.tuning_config, |ui| {
            egui::ComboBox::from_id_salt(format!("tuning_target_{}", miner.ip))
                .selected_text(match options_state.tuning_target {
                    TuningTargetSelection::MiningMode => "Mode",
                    TuningTargetSelection::Power => "Power",
                    TuningTargetSelection::Hashrate => "Hashrate",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut options_state.tuning_target,
                        TuningTargetSelection::MiningMode,
                        "Mode",
                    );
                    ui.selectable_value(
                        &mut options_state.tuning_target,
                        TuningTargetSelection::Power,
                        "Power",
                    );
                    ui.selectable_value(
                        &mut options_state.tuning_target,
                        TuningTargetSelection::Hashrate,
                        "Hashrate",
                    );
                });
        });
    });

    ui.horizontal_wrapped(|ui| {
        ui.label("Target");
        ui.add_enabled_ui(miner.capabilities.tuning_config, |ui| {
            match options_state.tuning_target {
                TuningTargetSelection::MiningMode => {
                    egui::ComboBox::from_id_salt(format!("tuning_mode_{}", miner.ip))
                        .selected_text(match options_state.mining_mode {
                            MiningModeSelection::Low => "Low",
                            MiningModeSelection::Normal => "Normal",
                            MiningModeSelection::High => "High",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut options_state.mining_mode,
                                MiningModeSelection::Low,
                                "Low",
                            );
                            ui.selectable_value(
                                &mut options_state.mining_mode,
                                MiningModeSelection::Normal,
                                "Normal",
                            );
                            ui.selectable_value(
                                &mut options_state.mining_mode,
                                MiningModeSelection::High,
                                "High",
                            );
                        });
                }
                TuningTargetSelection::Power => {
                    ui.add(
                        egui::DragValue::new(&mut options_state.tuning_power_watts)
                            .range(100.0..=9000.0)
                            .speed(25.0)
                            .suffix(" W"),
                    );
                }
                TuningTargetSelection::Hashrate => {
                    ui.add(
                        egui::DragValue::new(&mut options_state.tuning_hashrate_ths)
                            .range(1.0..=5000.0)
                            .speed(5.0)
                            .suffix(" TH/s"),
                    );
                    egui::ComboBox::from_id_salt(format!("tuning_hashrate_algo_{}", miner.ip))
                        .selected_text(options_state.tuning_hashrate_algo.clone())
                        .show_ui(ui, |ui| {
                            for algo in HASHRATE_ALGO_OPTIONS {
                                ui.selectable_value(
                                    &mut options_state.tuning_hashrate_algo,
                                    algo.to_string(),
                                    algo,
                                );
                            }
                        });
                    ui.add(
                        egui::TextEdit::singleline(&mut options_state.tuning_hashrate_algo)
                            .desired_width(110.0)
                            .hint_text("custom algo"),
                    );
                }
            }

            ui.label("Algo");
            if show_epic_tuning_presets {
                egui::ComboBox::from_id_salt(format!("tuning_algorithm_{}", miner.ip))
                    .selected_text(if options_state.tuning_algorithm.trim().is_empty() {
                        "None".to_string()
                    } else {
                        options_state.tuning_algorithm.clone()
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut options_state.tuning_algorithm,
                            String::new(),
                            "None",
                        );
                        for algo in EPIC_TUNING_ALGO_OPTIONS {
                            ui.selectable_value(
                                &mut options_state.tuning_algorithm,
                                algo.to_string(),
                                algo,
                            );
                        }
                    });
            }
            ui.add(
                egui::TextEdit::singleline(&mut options_state.tuning_algorithm)
                    .desired_width(120.0)
                    .hint_text(if show_epic_tuning_presets {
                        "custom override"
                    } else {
                        "custom tuning algo"
                    }),
            );
        });
    });

    if options_state.apply_tuning_config {
        if let Some(message) = options_state.tuning_validation_message() {
            ui.label(
                egui::RichText::new(format!("⚠ {}", message))
                    .size(10.0)
                    .color(Color32::from_rgb(255, 120, 120)),
            );
        }
    }

    ui.horizontal(|ui| {
        ui.checkbox(&mut options_state.apply_scaling_config, "");
        ui.label("Scaling");

        ui.add_enabled_ui(miner.capabilities.scaling_config, |ui| {
            ui.add(
                egui::DragValue::new(&mut options_state.scaling_step)
                    .range(1..=100)
                    .speed(1)
                    .prefix("step "),
            );
            ui.add(
                egui::DragValue::new(&mut options_state.scaling_minimum)
                    .range(0..=100)
                    .speed(1)
                    .prefix("min "),
            );
            ui.checkbox(&mut options_state.scaling_shutdown, "shutdown");
            ui.add_enabled(
                options_state.scaling_shutdown,
                egui::DragValue::new(&mut options_state.scaling_shutdown_duration)
                    .range(1.0..=300.0)
                    .speed(1.0)
                    .suffix(" s"),
            );
        });
    });

    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.checkbox(&mut options_state.apply_pool_config, "");
        ui.label("Pools");
        ui.add_enabled_ui(miner.capabilities.pools_config, |ui| {
            ui.label("Group");
            ui.add(
                egui::TextEdit::singleline(&mut options_state.pool_group_name)
                    .desired_width(100.0)
                    .hint_text("Primary"),
            );
            ui.add(
                egui::DragValue::new(&mut options_state.pool_group_quota)
                    .range(1..=100)
                    .suffix(" %"),
            );
        });
    });

    ui.add_enabled_ui(miner.capabilities.pools_config, |ui| {
        let mut remove_idx: Option<usize> = None;
        let can_remove_pool = options_state.pool_inputs.len() > 1;
        for (idx, pool) in options_state.pool_inputs.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", idx + 1));
                ui.add(
                    egui::TextEdit::singleline(&mut pool.url)
                        .desired_width(260.0)
                        .hint_text("stratum+tcp://pool.example.com:3333"),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut pool.username)
                        .desired_width(180.0)
                        .hint_text("wallet.worker"),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut pool.password)
                        .desired_width(70.0)
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
            options_state.pool_inputs.remove(idx);
        }

        ui.horizontal(|ui| {
            if ui.button("+ Add Pool").clicked() {
                options_state.pool_inputs.push(PoolInput::default());
            }
            ui.label(
                egui::RichText::new(format!("{} configured", options_state.pool_inputs.len()))
                    .size(10.0)
                    .color(Color32::from_rgb(130, 130, 130)),
            );
        });

        if options_state.apply_pool_config {
            if let Some(message) = options_state.pool_validation_message() {
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
    });

    if let Some(data) = &miner.full_data {
        let configured_pools = data
            .pools
            .iter()
            .flat_map(|g| g.pools.iter().map(|p| (g.name.clone(), p)))
            .collect::<Vec<_>>();

        if !configured_pools.is_empty() {
            ui.add_space(4.0);
            ui.collapsing("Current Pool Preview", |ui| {
                for (idx, (group_name, pool)) in configured_pools.iter().enumerate() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}. [{}] {} ({})",
                            idx + 1,
                            group_name,
                            pool.url
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_else(|| "N/A".to_string()),
                            pool.user.clone().unwrap_or_else(|| "N/A".to_string())
                        ))
                        .size(10.0)
                        .color(Color32::from_rgb(160, 160, 160)),
                    );
                }
            });
        }
    }

    ui.horizontal(|ui| {
        if ui.button("Use Global Defaults").clicked() {
            options_state = global_options.clone();
        }

        if ui.button("Apply to This Miner").clicked() {
            let ip = miner.ip.clone();
            let settings = options_state.clone();
            if !settings.has_any_enabled() {
                eprintln!("✗ No option toggles are enabled for {}", ip);
            } else {
                crate::runtime::spawn(async move {
                    match options::apply_options_to_miner(ip.clone(), settings).await {
                        Ok(applied) => {
                            println!("✓ Applied options to {} ({})", ip, applied.join(", "));
                        }
                        Err(err) => {
                            eprintln!("✗ {}", err);
                        }
                    }
                });
            }
        }
    });

    miner_option_overrides
        .lock()
        .unwrap()
        .insert(miner.ip.clone(), options_state);
}

fn draw_control_buttons(
    ui: &mut egui::Ui,
    miner: &MinerInfo,
    miners_arc: Arc<Mutex<Vec<MinerInfo>>>,
    detail_refresh_times: &mut HashMap<String, Instant>,
    recording_states: &mut HashMap<String, RecordingState>,
) {
    let url = format!("http://{}", miner.ip);

    // Quick Actions heading
    ui.heading("Quick Actions");
    ui.add_space(5.0);

    ui.horizontal(|ui| {
        // Web interface button
        if ui
            .add_sized(
                [150.0, 32.0],
                egui::Button::new(
                    egui::RichText::new("🌐 Web Interface")
                        .size(13.0)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(100, 150, 255))
                .corner_radius(6.0),
            )
            .clicked()
        {
            let _ = webbrowser::open(&url);
        }

        ui.add_space(5.0);

        // Miner control actions
        if ui
            .add_sized(
                [120.0, 32.0],
                egui::Button::new(
                    egui::RichText::new("▶ START")
                        .size(13.0)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(100, 200, 100)),
            )
            .clicked()
        {
            let ip = miner.ip.clone();
            crate::runtime::spawn(async move {
                let factory = MinerFactory::new();
                if let Ok(Some(miner)) = factory.get_miner(ip.parse().unwrap()).await {
                    match miner.resume(None).await {
                        Ok(_) => println!("✓ Started miner: {ip}"),
                        Err(e) => eprintln!("✗ Failed to start {ip}: {e}"),
                    }
                }
            });
        }

        if ui
            .add_sized(
                [120.0, 32.0],
                egui::Button::new(
                    egui::RichText::new("■ STOP")
                        .size(13.0)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(255, 100, 100)),
            )
            .clicked()
        {
            let ip = miner.ip.clone();
            crate::runtime::spawn(async move {
                let factory = MinerFactory::new();
                if let Ok(Some(miner)) = factory.get_miner(ip.parse().unwrap()).await {
                    match miner.pause(None).await {
                        Ok(_) => println!("✓ Stopped miner: {ip}"),
                        Err(e) => eprintln!("✗ Failed to stop {ip}: {e}"),
                    }
                }
            });
        }

        if ui
            .add_sized(
                [130.0, 32.0],
                egui::Button::new(
                    egui::RichText::new("💡 FAULT LIGHT")
                        .size(13.0)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(255, 165, 0)),
            )
            .clicked()
        {
            let ip = miner.ip.clone();
            let current_state = miner.light_flashing;
            let miners = Arc::clone(&miners_arc);
            crate::runtime::spawn(async move {
                let factory = MinerFactory::new();
                if let Ok(Some(miner_obj)) = factory.get_miner(ip.parse().unwrap()).await {
                    let new_state = !current_state;
                    match miner_obj.set_fault_light(new_state).await {
                        Ok(_) => {
                            println!(
                                "✓ Set fault light to {}: {ip}",
                                if new_state { "ON" } else { "OFF" }
                            );
                            let data = miner_obj.get_data().await;
                            let mut miners_list = miners.lock().unwrap();
                            if let Some(existing) = miners_list.iter_mut().find(|m| m.ip == ip) {
                                existing.full_data = Some(data);
                            }
                        }
                        Err(e) => eprintln!("✗ Failed to set fault light on {ip}: {e}"),
                    }
                }
            });
            // Force immediate refresh in UI
            detail_refresh_times.insert(miner.ip.clone(), Instant::now());
        }
    });

    // Recording controls - new row
    ui.add_space(15.0);
    ui.heading("Metrics Recording");
    ui.add_space(5.0);

    ui.horizontal(|ui| {
        // Check recording state without holding reference
        let is_recording = recording_states
            .get(&miner.ip)
            .map(|r| r.is_recording)
            .unwrap_or(false);
        let recording_info = recording_states
            .get(&miner.ip)
            .map(|r| (r.start_time.elapsed().as_secs(), r.row_count));

        if is_recording {
            // Show stop button and recording status
            if let Some((elapsed, row_count)) = recording_info {
                let mins = elapsed / 60;
                let secs = elapsed % 60;

                if ui
                    .add_sized(
                        [150.0, 32.0],
                        egui::Button::new(
                            egui::RichText::new("⏹ STOP RECORDING")
                                .size(13.0)
                                .color(Color32::WHITE),
                        )
                        .fill(Color32::from_rgb(200, 50, 50)),
                    )
                    .clicked()
                {
                    if let Some(recording) = recording_states.get_mut(&miner.ip) {
                        crate::recording::stop_recording(recording);
                    }
                }

                ui.label(
                    egui::RichText::new(format!("📊 {mins}:{secs:02} ({row_count} rows)"))
                        .color(Color32::from_rgb(255, 100, 100))
                        .size(14.0),
                );
            }
        } else {
            // Show start button
            if ui
                .add_sized(
                    [150.0, 32.0],
                    egui::Button::new(
                        egui::RichText::new("🔴 START RECORDING")
                            .size(13.0)
                            .color(Color32::WHITE),
                    )
                    .fill(Color32::from_rgb(255, 87, 51)),
                )
                .clicked()
            {
                match crate::recording::start_recording(miner) {
                    Ok(recording_state) => {
                        recording_states.insert(miner.ip.clone(), recording_state);
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to start recording: {e}");
                    }
                }
            }

            // Show export button if there's a stopped recording with data
            if let Some((_, row_count)) = recording_info {
                if row_count > 0 {
                    if ui
                        .add_sized(
                            [140.0, 32.0],
                            egui::Button::new(
                                egui::RichText::new("💾 EXPORT TO CSV")
                                    .size(13.0)
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(100, 150, 255)),
                        )
                        .clicked()
                    {
                        // Open file dialog
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("CSV", &["csv"])
                            .set_file_name(format!("miner_{}.csv", miner.ip.replace(".", "_")))
                            .save_file()
                        {
                            if let Some(recording) = recording_states.get(&miner.ip) {
                                match crate::recording::export_recording(
                                    recording,
                                    path.to_str().unwrap(),
                                ) {
                                    Ok(_) => println!("✓ Exported recording to: {path:?}"),
                                    Err(e) => eprintln!("✗ Failed to export: {e}"),
                                }
                            }
                        }
                    }

                    ui.label(
                        egui::RichText::new(format!("📁 {row_count} rows ready"))
                            .color(Color32::from_rgb(150, 150, 150))
                            .size(14.0),
                    );
                }
            }
        }
    });
}

fn draw_metrics_graphs(
    ui: &mut egui::Ui,
    miner: &MinerInfo,
    detail_metrics_history: &HashMap<String, MetricsHistory>,
) {
    let history = detail_metrics_history.get(&miner.ip);

    if let Some(history_data) = history {
        use egui_plot::{Legend, Line, Plot, PlotPoints};

        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(200));

        ui.heading("Metrics Over Time");
        ui.separator();
        ui.add_space(5.0);

        if !history_data.is_empty() {
            use chrono::{Local, TimeZone};

            let num_boards = history_data
                .front()
                .map(|(_, _, _, boards, _, _)| boards.len())
                .unwrap_or(0);

            // Row 1: Hashrate (total + per-board)
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Hashrate").strong());

                let total_hashrate_points: Vec<[f64; 2]> = history_data
                    .iter()
                    .map(|(ts, hr, _, _, _, _)| [*ts, *hr])
                    .collect();

                let max_hashrate = total_hashrate_points
                    .iter()
                    .map(|p| p[1])
                    .fold(0.0f64, f64::max);

                Plot::new(format!("total_hashrate_{}", miner.ip))
                    .height(200.0)
                    .allow_zoom([true, false])
                    .allow_scroll(false)
                    .include_y(0.0)
                    .include_y(max_hashrate * 1.1)
                    .legend(Legend::default())
                    .x_axis_formatter(|val, _range| {
                        if let Some(dt) = Local.timestamp_opt(val.value as i64, 0).single() {
                            dt.format("%H:%M:%S").to_string()
                        } else {
                            String::new()
                        }
                    })
                    .show(ui, |plot_ui| {
                        // Total hashrate line
                        plot_ui.line(
                            Line::new("Total", PlotPoints::from(total_hashrate_points.clone()))
                                .color(Color32::from_rgb(100, 200, 255))
                                .width(2.5),
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
                                    boards.get(board_idx).map(|hr| [*ts, *hr])
                                })
                                .collect();

                            if !board_points.is_empty() {
                                plot_ui.line(
                                    Line::new(
                                        format!("Board {board_idx}"),
                                        PlotPoints::from(board_points),
                                    )
                                    .color(board_colors[board_idx % board_colors.len()])
                                    .width(1.5),
                                );
                            }
                        }
                    });

                if let Some(latest) = history_data.back() {
                    ui.label(format!("Total: {:.2} TH/s", latest.1));
                }
            });

            ui.add_space(15.0);

            // Row 2: Temperature (average + per-board)
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Temperature").strong());

                let avg_temp_points: Vec<[f64; 2]> = history_data
                    .iter()
                    .map(|(ts, _, _, _, avg_t, _)| [*ts, *avg_t])
                    .collect();

                let max_temp = avg_temp_points.iter().map(|p| p[1]).fold(0.0f64, f64::max);

                Plot::new(format!("temperature_{}", miner.ip))
                    .height(200.0)
                    .allow_zoom([true, false])
                    .allow_scroll(false)
                    .include_y(0.0)
                    .include_y(max_temp * 1.1)
                    .legend(Legend::default())
                    .x_axis_formatter(|val, _range| {
                        if let Some(dt) = Local.timestamp_opt(val.value as i64, 0).single() {
                            dt.format("%H:%M:%S").to_string()
                        } else {
                            String::new()
                        }
                    })
                    .show(ui, |plot_ui| {
                        // Average temperature line
                        plot_ui.line(
                            Line::new("Average", PlotPoints::from(avg_temp_points.clone()))
                                .color(Color32::from_rgb(255, 100, 100))
                                .width(2.5),
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
                                    temps.get(board_idx).map(|t| [*ts, *t])
                                })
                                .collect();

                            if !board_temp_points.is_empty() {
                                plot_ui.line(
                                    Line::new(
                                        format!("Board {board_idx}"),
                                        PlotPoints::from(board_temp_points),
                                    )
                                    .color(board_colors[board_idx % board_colors.len()])
                                    .width(1.5),
                                );
                            }
                        }
                    });

                if let Some(latest) = history_data.back() {
                    ui.label(format!("Average: {:.1} °C", latest.4));
                }
            });

            ui.add_space(15.0);

            // Row 3: Efficiency (W/TH)
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Efficiency (W/TH)").strong());

                let efficiency_points: Vec<[f64; 2]> = history_data
                    .iter()
                    .filter_map(|(ts, hr, pw, _, _, _)| {
                        if *hr > 0.0 {
                            Some([*ts, pw / hr])
                        } else {
                            None
                        }
                    })
                    .collect();

                if !efficiency_points.is_empty() {
                    let max_efficiency = efficiency_points
                        .iter()
                        .map(|p| p[1])
                        .fold(0.0f64, f64::max);

                    Plot::new(format!("efficiency_{}", miner.ip))
                        .height(200.0)
                        .allow_zoom([true, false])
                        .allow_scroll(false)
                        .include_y(0.0)
                        .include_y(max_efficiency * 1.1)
                        .x_axis_formatter(|val, _range| {
                            if let Some(dt) = Local.timestamp_opt(val.value as i64, 0).single() {
                                dt.format("%H:%M:%S").to_string()
                            } else {
                                String::new()
                            }
                        })
                        .show(ui, |plot_ui| {
                            plot_ui.line(
                                Line::new(
                                    "Efficiency",
                                    PlotPoints::from(efficiency_points.clone()),
                                )
                                .color(Color32::from_rgb(150, 255, 150))
                                .width(2.0),
                            );
                        });

                    if let Some(latest) = history_data.back() {
                        let current_eff = if latest.1 > 0.0 {
                            latest.2 / latest.1
                        } else {
                            0.0
                        };
                        ui.label(format!("Current: {current_eff:.2} W/TH"));
                    }
                }
            });

            ui.add_space(15.0);

            // Row 4: Power
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("Power").strong());

                let power_points: Vec<[f64; 2]> = history_data
                    .iter()
                    .map(|(ts, _, pw, _, _, _)| [*ts, *pw])
                    .collect();

                let max_power = power_points.iter().map(|p| p[1]).fold(0.0f64, f64::max);

                Plot::new(format!("total_power_{}", miner.ip))
                    .height(200.0)
                    .allow_zoom([true, false])
                    .allow_scroll(false)
                    .include_y(0.0)
                    .include_y(max_power * 1.1)
                    .x_axis_formatter(|val, _range| {
                        if let Some(dt) = Local.timestamp_opt(val.value as i64, 0).single() {
                            dt.format("%H:%M:%S").to_string()
                        } else {
                            String::new()
                        }
                    })
                    .show(ui, |plot_ui| {
                        plot_ui.line(
                            Line::new("Power", PlotPoints::from(power_points.clone()))
                                .color(Color32::from_rgb(255, 165, 0))
                                .width(2.0),
                        );
                    });

                if let Some(latest) = history_data.back() {
                    ui.label(format!("Current: {:.0} W", latest.2));
                }
            });

            ui.add_space(5.0);
            let span_secs = match (history_data.front(), history_data.back()) {
                (Some(start), Some(end)) => (end.0 - start.0).max(0.0),
                _ => 0.0,
            };
            ui.label(format!(
                "Data points: {} (last {:.0}s)",
                history_data.len(),
                span_secs
            ));
        } else {
            ui.label("Collecting data...");
        }
    }
}
