use crate::models::MinerInfo;
use eframe::egui;
use egui::Color32;

pub fn draw_stats_card(ui: &mut egui::Ui, miners: &[MinerInfo]) {
    let target_inner_width = (ui.available_width() - 30.0).max(0.0);
    let miner_count = miners.len();

    let hashrates: Vec<f64> = miners.iter().filter_map(|m| m.hashrate_th).collect();

    let total_hashrate: f64 = hashrates.iter().sum();

    let avg_hashrate = if !hashrates.is_empty() {
        total_hashrate / hashrates.len() as f64
    } else {
        0.0
    };

    let wattages: Vec<f64> = miners.iter().filter_map(|m| m.wattage_w).collect();

    let total_wattage: f64 = wattages.iter().sum();

    let temps: Vec<f64> = miners.iter().filter_map(|m| m.temperature_c).collect();

    let avg_temp = if !temps.is_empty() {
        temps.iter().sum::<f64>() / temps.len() as f64
    } else {
        0.0
    };

    let efficiencies: Vec<f64> = miners
        .iter()
        .filter_map(|m| m.efficiency_w_th)
        .filter(|v| v.is_finite())
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
            ui.set_width(target_inner_width);
            ui.set_max_width(target_inner_width);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("⚡ FLEET OVERVIEW")
                        .size(13.0)
                        .color(Color32::WHITE)
                        .strong()
                        .monospace(),
                );

                ui.add_space(10.0);

                // Miner count
                ui.label(
                    egui::RichText::new(format!("{miner_count}"))
                        .size(16.0)
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

                ui.add_space(10.0);

                // Total hashrate
                let (hashrate_display, hashrate_unit) = if total_hashrate >= 1000.0 {
                    (total_hashrate / 1000.0, "PH/s")
                } else {
                    (total_hashrate, "TH/s")
                };
                // Total wattage
                let (wattage_format, wattage_unit) = if total_wattage >= 10000.0 {
                    (format!("{:.2}", total_wattage / 1000.0), "kW")
                } else {
                    (format!("{:.0}", total_wattage), "W")
                };
                // Fleet efficiency
                let fleet_efficiency = if total_hashrate > 0.0 {
                    total_wattage / total_hashrate
                } else {
                    0.0
                };

                ui.label(
                    egui::RichText::new(format!("{hashrate_display:.2} {hashrate_unit}  •   {wattage_format} {wattage_unit}  •   {fleet_efficiency:.2} W/TH"))
                        .size(16.0)
                        .color(Color32::WHITE)
                        .strong()
                        .monospace(),
                );

                ui.add_space(10.0);

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
