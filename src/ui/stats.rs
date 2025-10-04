use crate::models::MinerInfo;
use eframe::egui;
use egui::Color32;

pub fn draw_stats_card(ui: &mut egui::Ui, miners: &[MinerInfo]) {
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
