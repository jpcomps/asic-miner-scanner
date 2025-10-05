use crate::models::MinerInfo;
use eframe::egui;
use egui::Color32;
use egui_plot::{Line, Plot, PlotPoints};

// Linear interpolation between data points for smoother visualization
fn interpolate_points(points: &[(f64, f64)], points_per_interval: usize) -> Vec<[f64; 2]> {
    if points.len() < 2 {
        return points.iter().map(|(x, y)| [*x, *y]).collect();
    }

    let mut interpolated = Vec::new();

    for i in 0..points.len() - 1 {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];

        // Add the starting point
        interpolated.push([x1, y1]);

        // Add interpolated points between this and next
        for j in 1..points_per_interval {
            let t = j as f64 / points_per_interval as f64;
            let x = x1 + t * (x2 - x1);
            let y = y1 + t * (y2 - y1);
            interpolated.push([x, y]);
        }
    }

    // Add the final point
    if let Some(&(x, y)) = points.last() {
        interpolated.push([x, y]);
    }

    interpolated
}

pub fn draw_stats_card(
    ui: &mut egui::Ui,
    miners: &[MinerInfo],
    fleet_hashrate_history: &[(f64, f64)],
) {
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

    // Parse wattage - extract numbers from strings like "3400 W" or "3400.5"
    let wattages: Vec<f64> = miners
        .iter()
        .filter_map(|m| m.wattage.split_whitespace().next()?.parse::<f64>().ok())
        .collect();

    let total_wattage: f64 = wattages.iter().sum();

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
                    egui::RichText::new(format!("{hashrate_display:.2} {hashrate_unit}  •   {wattage_format:.2} {wattage_unit}  •   {fleet_efficiency:.2} W/TH"))
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

    ui.add_space(15.0);

    // Fleet Hashrate History Plot
    if !fleet_hashrate_history.is_empty() {
        // Request faster repaints for smoother plot updates
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));

        egui::Frame::new()
            .fill(Color32::from_rgb(28, 28, 28))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
            .corner_radius(4.0)
            .inner_margin(15.0)
            .show(ui, |ui| {
                // Determine if we should use PH or TH based on max value
                let max_value_th = fleet_hashrate_history
                    .iter()
                    .map(|(_, hr)| hr)
                    .fold(0.0f64, |a, &b| a.max(b));

                let use_ph = max_value_th >= 1000.0;
                let unit_label = if use_ph { "PH/s" } else { "TH/s" };
                let divisor = if use_ph { 1000.0 } else { 1.0 };

                ui.label(
                    egui::RichText::new(format!("FLEET HASHRATE HISTORY ({})", unit_label))
                        .size(11.0)
                        .color(Color32::from_rgb(240, 240, 240))
                        .strong()
                        .monospace(),
                );

                ui.add_space(5.0);

                // Convert to (timestamp, hashrate) tuples and apply interpolation
                let raw_points: Vec<(f64, f64)> = fleet_hashrate_history
                    .iter()
                    .map(|(ts, hr)| (*ts, hr / divisor))
                    .collect();

                // Interpolate with 4 points between each actual data point for smoothness
                let points = interpolate_points(&raw_points, 4);

                // Calculate y-axis range with some padding for better visibility
                let min_hashrate = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                let max_hashrate = points.iter().map(|p| p[1]).fold(0.0f64, f64::max);

                // Add 10% padding on top and bottom
                let range = max_hashrate - min_hashrate;
                let y_min = (min_hashrate - range * 0.05).max(0.0);
                let y_max = max_hashrate + range * 0.05;

                Plot::new("fleet_hashrate_plot")
                    .height(150.0)
                    .allow_zoom([true, false])
                    .allow_scroll(false)
                    .include_y(y_min)
                    .include_y(y_max)
                    .show_axes([false, true])
                    .x_axis_formatter(|val, _range| {
                        use chrono::{Local, TimeZone};
                        if let Some(dt) = Local.timestamp_opt(val.value as i64, 0).single() {
                            dt.format("%H:%M:%S").to_string()
                        } else {
                            String::new()
                        }
                    })
                    .show(ui, |plot_ui| {
                        plot_ui.line(
                            Line::new("Fleet Hashrate", PlotPoints::from(points))
                                .color(Color32::from_rgb(255, 87, 51))
                                .width(2.0),
                        );
                    });
            });
    }
}
