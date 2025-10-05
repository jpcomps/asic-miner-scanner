use crate::models::{MinerInfo, ScanProgress, SortColumn, SortDirection};
use asic_rs::MinerFactory;
use eframe::egui;
use egui::{Color32, FontId};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub fn draw_miners_table(
    ui: &mut egui::Ui,
    miners: &[MinerInfo],
    search_query: &mut String,
    selected_miners: &mut HashSet<String>,
    detail_view_miners: &mut Vec<MinerInfo>,
    sort_column: Option<SortColumn>,
    sort_direction: SortDirection,
    scan_progress: Arc<Mutex<ScanProgress>>,
) -> Option<SortColumn> {
    let mut clicked_column: Option<SortColumn> = None;

    // Filter miners based on search query
    let filtered_miners: Vec<&MinerInfo> = if search_query.is_empty() {
        miners.iter().collect()
    } else {
        let query = search_query.to_lowercase();
        miners
            .iter()
            .filter(|m| {
                m.ip.to_lowercase().contains(&query)
                    || m.hostname.to_lowercase().contains(&query)
                    || m.model.to_lowercase().contains(&query)
                    || m.firmware_version.to_lowercase().contains(&query)
                    || m.pool.to_lowercase().contains(&query)
            })
            .collect()
    };

    // Show scanning progress if no miners found yet
    let progress = scan_progress.lock().unwrap();
    if miners.is_empty() && progress.scanning {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);

            ui.label(
                egui::RichText::new("🔍 Scanning for miners...")
                    .size(16.0)
                    .color(Color32::from_rgb(255, 87, 51)),
            );

            ui.add_space(10.0);

            let progress_text = if progress.total_ips > 0 {
                format!(
                    "Scanned {} / {} IPs ({} miners found)",
                    progress.scanned_ips, progress.total_ips, progress.found_miners
                )
            } else {
                "Initializing scan...".to_string()
            };

            ui.label(
                egui::RichText::new(progress_text)
                    .size(12.0)
                    .color(Color32::from_rgb(180, 180, 180)),
            );

            ui.add_space(10.0);

            // Progress bar
            if progress.total_ips > 0 {
                let progress_fraction = progress.scanned_ips as f32 / progress.total_ips as f32;
                ui.add(
                    egui::ProgressBar::new(progress_fraction)
                        .text(format!("{:.1}%", progress_fraction * 100.0))
                        .desired_width(400.0)
                        .fill(Color32::from_rgb(255, 87, 51)),
                );
            } else {
                ui.spinner();
            }

            ui.add_space(20.0);

            if !progress.current_ip.is_empty() {
                ui.label(
                    egui::RichText::new(format!("Current IP: {}", progress.current_ip))
                        .size(10.0)
                        .color(Color32::from_rgb(120, 120, 120))
                        .monospace(),
                );
            }
        });
        drop(progress);
        return None;
    }
    drop(progress);

    // Bulk actions bar
    if !filtered_miners.is_empty() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Selected: {}", selected_miners.len()))
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
                .add_enabled(!selected_miners.is_empty(), start_btn)
                .on_hover_text("Start selected miners")
                .clicked()
            {
                let selected_ips = selected_miners.clone();
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
                .add_enabled(!selected_miners.is_empty(), stop_btn)
                .on_hover_text("Stop selected miners")
                .clicked()
            {
                let selected_ips = selected_miners.clone();
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
                .add_enabled(!selected_miners.is_empty(), fault_light_btn)
                .on_hover_text("Toggle fault light on selected miners")
                .clicked()
            {
                let selected_ips = selected_miners.clone();
                // Collect current states of selected miners
                let states: HashMap<String, bool> = filtered_miners
                    .iter()
                    .filter(|m| selected_ips.contains(&m.ip))
                    .map(|m| (m.ip.clone(), m.light_flashing))
                    .collect();

                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    for ip in selected_ips {
                        let ip_clone = ip.clone();
                        let current_state = states.get(&ip).copied().unwrap_or(false);
                        rt.block_on(async move {
                            let factory = MinerFactory::new();
                            if let Ok(Some(miner)) =
                                factory.get_miner(ip_clone.parse().unwrap()).await
                            {
                                let new_state = !current_state;
                                match miner.set_fault_light(new_state).await {
                                    Ok(_) => println!(
                                        "✓ Set fault light to {} on: {ip_clone}",
                                        if new_state { "ON" } else { "OFF" }
                                    ),
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
                *selected_miners = filtered_miners.iter().map(|m| m.ip.clone()).collect();
            }

            if ui.button("Deselect All").clicked() {
                selected_miners.clear();
            }
        });

        ui.add_space(10.0);

        // Search bar
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("🔍 SEARCH:")
                    .size(11.0)
                    .color(Color32::from_rgb(160, 160, 160))
                    .monospace(),
            );
            ui.add_space(5.0);
            ui.add(
                egui::TextEdit::singleline(search_query)
                    .hint_text("Filter by IP, hostname, model...")
                    .desired_width(300.0)
                    .font(FontId::monospace(11.0)),
            );
            if ui.button("✕").clicked() {
                search_query.clear();
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
                    let display_text = if search_query.is_empty() {
                        format!("⛏ DISCOVERED MINERS ({})", miners.len())
                    } else {
                        format!(
                            "⛏ DISCOVERED MINERS ({}/{})",
                            filtered_miners.len(),
                            miners.len()
                        )
                    };
                    ui.label(
                        egui::RichText::new(display_text)
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
                                for miner in filtered_miners.iter() {
                                    let is_selected = selected_miners.contains(&miner.ip);
                                    body.row(35.0, |mut row| {
                                        // Checkbox column
                                        row.col(|ui| {
                                            let mut selected = is_selected;
                                            if ui.checkbox(&mut selected, "").changed() {
                                                if selected {
                                                    selected_miners.insert(miner.ip.clone());
                                                } else {
                                                    selected_miners.remove(&miner.ip);
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
                                                    if !detail_view_miners
                                                        .iter()
                                                        .any(|m| m.ip == miner.ip)
                                                    {
                                                        detail_view_miners.push((*miner).clone());
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

    clicked_column
}
