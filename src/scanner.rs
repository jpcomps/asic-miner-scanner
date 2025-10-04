use crate::models::{HashratePoint, MinerInfo, ScanProgress};
use asic_rs::MinerFactory;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn parse_ip_range(start: &str, end: &str) -> Result<String, String> {
    let start_parts: Vec<&str> = start.split('.').collect();
    let end_parts: Vec<&str> = end.split('.').collect();

    if start_parts.len() != 4 || end_parts.len() != 4 {
        return Err("Invalid IP address format".to_string());
    }

    // Verify first three octets match
    if start_parts[0] != end_parts[0]
        || start_parts[1] != end_parts[1]
        || start_parts[2] != end_parts[2]
    {
        return Err(
            "IP ranges must be in the same subnet (first 3 octets must match)".to_string(),
        );
    }

    let start_last: u8 = start_parts[3]
        .parse()
        .map_err(|_| "Invalid start IP address".to_string())?;
    let end_last: u8 = end_parts[3]
        .parse()
        .map_err(|_| "Invalid end IP address".to_string())?;

    if start_last > end_last {
        return Err("Start IP must be less than or equal to end IP".to_string());
    }

    // Format: "192.168.1.1-254" for asic-rs
    let range = format!(
        "{}.{}.{}.{}-{}",
        start_parts[0], start_parts[1], start_parts[2], start_last, end_last
    );

    Ok(range)
}

pub fn calculate_total_ips(range: &str) -> usize {
    // Parse "192.168.1.1-254" format
    if let Some(dash_pos) = range.rfind('-') {
        if let Some(last_dot_pos) = range[..dash_pos].rfind('.') {
            if let (Ok(start), Ok(end)) = (
                range[last_dot_pos + 1..dash_pos].parse::<u8>(),
                range[dash_pos + 1..].parse::<u8>(),
            ) {
                if end >= start {
                    return (end as usize) - (start as usize) + 1;
                }
            }
        }
    }
    0
}

pub fn scan_ranges(
    ranges: Vec<String>,
    miners: Arc<Mutex<Vec<MinerInfo>>>,
    scan_progress: Arc<Mutex<ScanProgress>>,
    hashrate_history: Arc<Mutex<HashMap<String, Vec<HashratePoint>>>>,
) {
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let mut new_miners: HashMap<String, MinerInfo> = HashMap::new();

            for range in ranges {
                match MinerFactory::new()
                    .with_adaptive_concurrency()
                    .with_identification_timeout_secs(5)
                    .with_connectivity_retries(2)
                    .with_port_check(true)
                    .scan_by_range(&range)
                    .await
                {
                    Ok(discovered_miners) => {
                        for miner in discovered_miners {
                            let ip = miner.get_ip().to_string();

                            {
                                let mut progress = scan_progress.lock().unwrap();
                                progress.current_ip = ip.clone();
                                progress.scanned_ips += 1;
                            }

                            let data = miner.get_data().await;

                            // Extract hashrate value for efficiency calculation
                            let hashrate_th = data.hashrate.as_ref().map(|hr| {
                                hr.clone()
                                    .as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash)
                            });

                            // Extract wattage (power) and efficiency
                            let wattage_str = if let Some(wattage) = data.wattage {
                                format!("{:.0} W", wattage.as_watts())
                            } else {
                                "N/A".to_string()
                            };

                            let efficiency_str = if let Some(efficiency) = data.efficiency {
                                format!("{efficiency:.1}")
                            } else {
                                "N/A".to_string()
                            };

                            let miner_info = MinerInfo {
                                ip: ip.clone(),
                                hostname: data
                                    .hostname
                                    .clone()
                                    .unwrap_or_else(|| "N/A".to_string()),
                                model: data.device_info.model.to_string(),
                                firmware_version: data
                                    .firmware_version
                                    .clone()
                                    .unwrap_or_else(|| "N/A".to_string()),
                                control_board: data
                                    .control_board_version
                                    .as_ref()
                                    .map(|cb| format!("{cb:?}"))
                                    .unwrap_or_else(|| "N/A".to_string()),
                                hashrate: match hashrate_th {
                                    Some(hr) => format!("{hr:.2}"),
                                    None => "N/A".to_string(),
                                },
                                wattage: wattage_str,
                                efficiency: efficiency_str,
                                temperature: {
                                    if let Some(temp) = data.average_temperature {
                                        format!("{:.1}°C", temp.as_celsius())
                                    } else {
                                        "N/A".to_string()
                                    }
                                },
                                fan_speed: {
                                    if !data.fans.is_empty() {
                                        if let Some(rpm) = data.fans[0].rpm {
                                            let rpm_value = rpm.as_radians_per_second() * 60.0
                                                / (2.0 * std::f64::consts::PI);
                                            format!("{rpm_value:.0} RPM")
                                        } else {
                                            "N/A".to_string()
                                        }
                                    } else {
                                        "N/A".to_string()
                                    }
                                },
                                pool: {
                                    if !data.pools.is_empty() {
                                        if let Some(url) = &data.pools[0].url {
                                            url.to_string()
                                        } else {
                                            "N/A".to_string()
                                        }
                                    } else {
                                        "N/A".to_string()
                                    }
                                },
                                light_flashing: data.light_flashing.unwrap_or(false),
                                full_data: Some(data.clone()),
                            };

                            // Record hashrate history
                            {
                                if let Some(hashrate_val) =
                                    miner_info.hashrate.split_whitespace().next()
                                {
                                    if let Ok(hashrate) = hashrate_val.parse::<f64>() {
                                        let timestamp = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs_f64();

                                        let mut history_map = hashrate_history.lock().unwrap();
                                        let history = history_map
                                            .entry(miner_info.ip.clone())
                                            .or_default();
                                        history.push(HashratePoint {
                                            timestamp,
                                            hashrate,
                                        });

                                        // Keep only last MAX_HISTORY_POINTS
                                        if history.len() > crate::models::MAX_HISTORY_POINTS {
                                            history.drain(
                                                0..history.len()
                                                    - crate::models::MAX_HISTORY_POINTS,
                                            );
                                        }
                                    }
                                }
                            }

                            // Use IP as key to deduplicate
                            new_miners.insert(miner_info.ip.clone(), miner_info);

                            let mut progress = scan_progress.lock().unwrap();
                            progress.found_miners = new_miners.len();
                        }
                    }
                    Err(e) => {
                        eprintln!("Scan error for range {range}: {e:?}");
                    }
                }
            }

            // Atomically replace miners list with new data (convert HashMap to Vec)
            {
                let mut miners_lock = miners.lock().unwrap();
                *miners_lock = new_miners.into_values().collect();
            }

            {
                let mut progress = scan_progress.lock().unwrap();
                progress.scanning = false;
                progress.current_ip.clear();
            }
        });
    });
}
