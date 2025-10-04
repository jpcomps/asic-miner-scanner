use crate::models::{SavedRange, ScanProgress};
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
}

pub fn draw_scan_and_ranges_card(
    ui: &mut egui::Ui,
    state: &mut ScanControlState,
    saved_ranges: &mut Vec<SavedRange>,
    scan_progress: Arc<Mutex<ScanProgress>>,
    on_scan_clicked: &mut bool,
    on_save_range_clicked: &mut bool,
    range_to_remove: &mut Option<usize>,
    range_to_load: &mut Option<SavedRange>,
) {
    // Get progress info early, then drop the lock
    let (is_scanning, scanned_ips, total_ips) = {
        let progress = scan_progress.lock().unwrap();
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

                    if ui
                        .add_enabled(!saved_ranges.is_empty(), scan_btn)
                        .clicked()
                    {
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

                    let text_edit = egui::TextEdit::singleline(&mut state.ip_range_start)
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

                    let text_edit = egui::TextEdit::singleline(&mut state.ip_range_end)
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

                    let text_edit = egui::TextEdit::singleline(&mut state.new_range_name)
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
                        *on_save_range_clicked = true;
                    }
                });

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
}
