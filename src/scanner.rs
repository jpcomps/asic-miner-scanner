use crate::models::{HashratePoint, MinerCapabilities, MinerInfo, ScanProgress};
use asic_rs::MinerFactory;
use asic_rs_core::data::hashrate::{HashRate, HashRateUnit};
use asic_rs_core::data::miner::MinerData;
use asic_rs_core::data::pool::PoolData;
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn parse_ip_range(start: &str, end: &str) -> Result<String, String> {
    let start_addr = start
        .parse::<Ipv4Addr>()
        .map_err(|_| "Invalid start IP address".to_string())?;
    let end_addr = end
        .parse::<Ipv4Addr>()
        .map_err(|_| "Invalid end IP address".to_string())?;

    let start_octets = start_addr.octets();
    let end_octets = end_addr.octets();

    // Verify first three octets match
    if start_octets[..3] != end_octets[..3] {
        return Err("IP ranges must be in the same subnet (first 3 octets must match)".to_string());
    }

    let start_last = start_octets[3];
    let end_last = end_octets[3];

    if start_last > end_last {
        return Err("Start IP must be less than or equal to end IP".to_string());
    }

    // Format: "192.168.1.1-254" for asic-rs, or "192.168.1.1" if single IP
    let range = if start_last == end_last {
        // Single IP, no range needed
        start_addr.to_string()
    } else {
        // Range format
        format!(
            "{}.{}.{}.{}-{}",
            start_octets[0], start_octets[1], start_octets[2], start_last, end_last
        )
    };

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

        return 0;
    }

    // Single IP format
    if range.parse::<Ipv4Addr>().is_ok() {
        return 1;
    }

    0
}

fn hashrate_to_terahash(hashrate: Option<&HashRate>) -> Option<f64> {
    hashrate
        .cloned()
        .map(|value| value.as_unit(HashRateUnit::TeraHash).value)
}

fn average_fan_rpm(data: &MinerData) -> Option<f64> {
    let rpms: Vec<f64> = data
        .fans
        .iter()
        .filter_map(|fan| fan.rpm)
        .map(|rpm| rpm.as_radians_per_second() * 60.0 / (2.0 * std::f64::consts::PI))
        .collect();

    if rpms.is_empty() {
        None
    } else {
        Some(rpms.iter().sum::<f64>() / rpms.len() as f64)
    }
}

fn primary_pool(data: &MinerData) -> Option<&PoolData> {
    data.pools
        .iter()
        .flat_map(|group| group.pools.iter())
        .find(|pool| pool.active.unwrap_or(false))
        .or_else(|| {
            data.pools
                .iter()
                .flat_map(|group| group.pools.iter())
                .next()
        })
}

fn count_active_boards(data: &MinerData) -> (usize, usize) {
    let total = data.hashboards.len();
    if total == 0 {
        return (0, 0);
    }

    let active = data
        .hashboards
        .iter()
        .filter(|board| {
            board.active.unwrap_or(false)
                || board
                    .hashrate
                    .as_ref()
                    .map(|hr| hr.clone().as_unit(HashRateUnit::TeraHash).value > 0.01)
                    .unwrap_or(false)
                || board.working_chips.unwrap_or(0) > 0
        })
        .count();

    (active, total)
}

fn build_miner_info(ip: String, data: MinerData, capabilities: MinerCapabilities) -> MinerInfo {
    let hashrate = hashrate_to_terahash(data.hashrate.as_ref());
    let wattage = data.wattage.map(|value| value.as_watts());
    let efficiency = data.efficiency;
    let temperature = data.average_temperature.map(|temp| temp.as_celsius());
    let fan_rpm = average_fan_rpm(&data);
    let selected_pool = primary_pool(&data);
    let (active_boards, total_boards) = count_active_boards(&data);

    MinerInfo {
        ip,
        hostname: data.hostname.clone().unwrap_or_else(|| "N/A".to_string()),
        model: data.device_info.model.to_string(),
        firmware_version: data
            .firmware_version
            .clone()
            .unwrap_or_else(|| "N/A".to_string()),
        control_board: data
            .control_board_version
            .as_ref()
            .map(|cb| cb.name.clone())
            .unwrap_or_else(|| "N/A".to_string()),
        active_boards: if total_boards > 0 {
            format!("{active_boards}/{total_boards}")
        } else {
            "N/A".to_string()
        },
        hashrate: hashrate
            .map(|value| format!("{value:.2}"))
            .unwrap_or_else(|| "N/A".to_string()),
        wattage: wattage
            .map(|value| format!("{value:.0} W"))
            .unwrap_or_else(|| "N/A".to_string()),
        efficiency: efficiency
            .map(|efficiency| format!("{efficiency:.1}"))
            .unwrap_or_else(|| "N/A".to_string()),
        temperature: temperature
            .map(|temp| format!("{temp:.1}°C"))
            .unwrap_or_else(|| "N/A".to_string()),
        fan_speed: fan_rpm
            .map(|rpm| format!("{rpm:.0} RPM"))
            .unwrap_or_else(|| "N/A".to_string()),
        pool: selected_pool
            .and_then(|pool| pool.url.as_ref())
            .map(ToString::to_string)
            .unwrap_or_else(|| "N/A".to_string()),
        worker: selected_pool
            .and_then(|pool| pool.user.as_ref())
            .cloned()
            .unwrap_or_else(|| "N/A".to_string()),
        light_flashing: data.light_flashing.unwrap_or(false),
        full_data: Some(data),
        hashrate_th: hashrate,
        wattage_w: wattage,
        efficiency_w_th: efficiency,
        temperature_c: temperature,
        fan_rpm,
        active_boards_count: (total_boards > 0).then_some(active_boards),
        total_boards_count: (total_boards > 0).then_some(total_boards),
        capabilities,
    }
}

pub fn scan_ranges(
    ranges: Vec<String>,
    miners: Arc<Mutex<Vec<MinerInfo>>>,
    scan_progress: Arc<Mutex<ScanProgress>>,
    hashrate_history: Arc<Mutex<HashMap<String, Vec<HashratePoint>>>>,
    identification_timeout_secs: u64,
    connectivity_timeout_secs: u64,
    connectivity_retries: u32,
) {
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            // Thread-safe shared HashMap for collecting results from parallel scans
            let new_miners = Arc::new(Mutex::new(HashMap::<String, MinerInfo>::new()));

            // Build a single MinerFactory and add all ranges
            let mut factory = MinerFactory::new()
                .with_identification_timeout_secs(identification_timeout_secs)
                .with_connectivity_timeout_secs(connectivity_timeout_secs)
                .with_connectivity_retries(connectivity_retries)
                .with_port_check(true);

            // Add all ranges to the factory
            for range in &ranges {
                match factory.with_range(range) {
                    Ok(f) => {
                        factory = f;
                    }
                    Err(e) => {
                        eprintln!("Failed to add range {}: {e:?}", range);
                        // On error, factory is consumed so we must stop
                        let mut progress = scan_progress.lock().unwrap();
                        progress.scanning = false;
                        return;
                    }
                }
            }

            factory.update_adaptive_concurrency();

            let mut discovered_miners = Vec::new();
            let mut scan_stream = factory.scan_stream_with_ip();

            while let Some((ip, miner_opt)) = scan_stream.next().await {
                let mut progress = scan_progress.lock().unwrap();
                progress.current_ip = ip.to_string();
                progress.scanned_ips = (progress.scanned_ips + 1).min(progress.total_ips);

                if let Some(miner) = miner_opt {
                    discovered_miners.push(miner);
                    progress.found_miners = discovered_miners.len();
                }
            }

            let data_fetch_concurrency = discovered_miners.len().clamp(1, 64);
            let miner_infos: Vec<MinerInfo> = stream::iter(discovered_miners.into_iter())
                .map(|miner| async move {
                    let capabilities = MinerCapabilities {
                        set_power_limit: miner.supports_set_power_limit(),
                        fan_config: miner.supports_fan_config(),
                        tuning_config: miner.supports_tuning_config(),
                        scaling_config: miner.supports_scaling_config(),
                        pools_config: miner.supports_pools_config(),
                    };
                    let ip = miner.get_ip().to_string();
                    let data = miner.get_data().await;
                    build_miner_info(ip, data, capabilities)
                })
                .buffer_unordered(data_fetch_concurrency)
                .collect()
                .await;

            {
                let mut miners_map = new_miners.lock().unwrap();

                for miner_info in miner_infos {
                    if let Some(hashrate) = miner_info.hashrate_th {
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs_f64();

                        let mut history_map = hashrate_history.lock().unwrap();
                        let history = history_map.entry(miner_info.ip.clone()).or_default();
                        history.push(HashratePoint {
                            timestamp,
                            hashrate,
                        });

                        if history.len() > crate::models::MAX_HISTORY_POINTS {
                            history.drain(0..history.len() - crate::models::MAX_HISTORY_POINTS);
                        }
                    }

                    miners_map.insert(miner_info.ip.clone(), miner_info);
                }

                let mut progress = scan_progress.lock().unwrap();
                progress.found_miners = miners_map.len();
                progress.scanned_ips = progress.total_ips;
            }
            // Mark all ranges as scanned
            {
                let mut progress = scan_progress.lock().unwrap();
                progress.scanned_ranges = ranges.len();
            }

            // Atomically replace miners list with new data (convert HashMap to Vec)
            {
                let new_miners_map = new_miners.lock().unwrap();
                let mut miners_lock = miners.lock().unwrap();
                *miners_lock = new_miners_map.values().cloned().collect();
            }

            {
                let mut progress = scan_progress.lock().unwrap();
                // Preserve scan duration before clearing
                if let Some(start_time) = progress.scan_start_time {
                    progress.scan_duration_secs = start_time.elapsed().as_secs();
                }
                progress.scanning = false;
                progress.current_ip.clear();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{calculate_total_ips, parse_ip_range};

    #[test]
    fn parse_ip_range_supports_single_ip() {
        let range = parse_ip_range("192.168.1.42", "192.168.1.42").unwrap();
        assert_eq!(range, "192.168.1.42");
    }

    #[test]
    fn parse_ip_range_supports_subnet_range() {
        let range = parse_ip_range("10.0.81.1", "10.0.81.254").unwrap();
        assert_eq!(range, "10.0.81.1-254");
    }

    #[test]
    fn parse_ip_range_rejects_invalid_start_address() {
        let err = parse_ip_range("10.0.999.1", "10.0.81.254").unwrap_err();
        assert_eq!(err, "Invalid start IP address");
    }

    #[test]
    fn calculate_total_ips_handles_range_and_single_ip() {
        assert_eq!(calculate_total_ips("10.0.81.1-254"), 254);
        assert_eq!(calculate_total_ips("10.0.81.42"), 1);
    }
}
