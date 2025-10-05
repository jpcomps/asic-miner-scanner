use crate::models::{MetricsHistory, MinerInfo, RecordingState};
use asic_rs::MinerFactory;
use eframe::egui;
use egui::Color32;
use std::collections::HashMap;
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

        // Delete recording file if it exists
        if let Some(recording) = recording_states.remove(&miner.ip) {
            if let Err(e) = crate::recording::delete_recording(&recording) {
                eprintln!("✗ Failed to delete recording for {}: {}", miner.ip, e);
            }
        }
    }
}

fn draw_basic_info(ui: &mut egui::Ui, data: &asic_rs::data::miner::MinerData) {
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
                    .map(|cb| format!("{cb:?}"))
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

fn draw_performance_info(ui: &mut egui::Ui, data: &asic_rs::data::miner::MinerData) {
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
                ui.label(format!(
                    "{:.2}",
                    hr.clone()
                        .as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash)
                ));
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

fn draw_fans_info(ui: &mut egui::Ui, data: &asic_rs::data::miner::MinerData) {
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

fn draw_pools_info(ui: &mut egui::Ui, data: &asic_rs::data::miner::MinerData) {
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
                ui.label(format!(
                    "Active: {}",
                    if pool.active.unwrap_or(false) {
                        "Yes"
                    } else {
                        "No"
                    }
                ));
            });
            ui.add_space(5.0);
        }
    } else {
        ui.label("No pool data available");
    }
}

fn draw_hashboards_info(ui: &mut egui::Ui, data: &asic_rs::data::miner::MinerData) {
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
                    ui.label(format!(
                        "Hashrate: {:.2}",
                        hashrate
                            .clone()
                            .as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash)
                    ));
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
                let converted = hr
                    .clone()
                    .as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash);
                let display_str = format!("{converted}");
                display_str
                    .split_whitespace()
                    .next()
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
            let board_hashrates: Vec<f64> = data
                .hashboards
                .iter()
                .filter_map(|board| {
                    board.hashrate.as_ref().and_then(|hr| {
                        let converted = hr
                            .clone()
                            .as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash);
                        let display_str = format!("{converted}");
                        display_str
                            .split_whitespace()
                            .next()
                            .and_then(|s| s.parse::<f64>().ok())
                    })
                })
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

            history.push((
                timestamp,
                total_hashrate,
                power,
                board_hashrates,
                avg_temp,
                board_temps,
            ));

            // Keep only last 9000 points (~5 minutes at 30fps for smooth rolling)
            if history.len() > 9000 {
                history.remove(0);
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
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async move {
                    let factory = MinerFactory::new();
                    if let Ok(Some(miner_obj)) = factory.get_miner(ip.parse().unwrap()).await {
                        let new_state = !current_state;
                        match miner_obj.set_fault_light(new_state).await {
                            Ok(_) => {
                                println!(
                                    "✓ Set fault light to {}: {ip}",
                                    if new_state { "ON" } else { "OFF" }
                                );
                                // Refresh miner data immediately
                                let data = miner_obj.get_data().await;
                                let mut miners_list = miners.lock().unwrap();
                                if let Some(existing) = miners_list.iter_mut().find(|m| m.ip == ip)
                                {
                                    existing.full_data = Some(data);
                                }
                            }
                            Err(e) => eprintln!("✗ Failed to set fault light on {ip}: {e}"),
                        }
                    }
                });
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

        // Request continuous repaints for smooth 60fps animation
        ui.ctx().request_repaint();

        ui.heading("Metrics Over Time");
        ui.separator();
        ui.add_space(5.0);

        if !history_data.is_empty() {
            use chrono::{Local, TimeZone};

            let num_boards = history_data
                .first()
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

                if let Some(latest) = history_data.last() {
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

                    if let Some(latest) = history_data.last() {
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

                if let Some(latest) = history_data.last() {
                    ui.label(format!("Current: {:.0} W", latest.2));
                }
            });

            ui.add_space(5.0);
            ui.label(format!(
                "Data points: {} (last {}s)",
                history_data.len(),
                ((history_data.len() - 1) * 10)
            ));
        } else {
            ui.label("Collecting data...");
        }
    }
}
