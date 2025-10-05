use crate::models::{MinerInfo, RecordingState};
use chrono::Local;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

const RECORDINGS_DIR: &str = "asic-miner-scanner/recordings";

pub fn get_recordings_dir() -> Result<PathBuf, std::io::Error> {
    let home = dirs::home_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found")
    })?;
    let recordings_path = home.join(RECORDINGS_DIR);
    create_dir_all(&recordings_path)?;
    Ok(recordings_path)
}

pub fn start_recording(miner: &MinerInfo) -> Result<RecordingState, std::io::Error> {
    let recordings_dir = get_recordings_dir()?;

    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let model_clean = miner.model.replace(" ", "");
    let filename = format!(
        "recording_{}_{}_{}_{}.csv",
        miner.ip,
        model_clean,
        miner.mac_address().unwrap_or_else(|| "unknown".to_string()),
        timestamp
    );

    let file_path = recordings_dir.join(filename);

    // Determine number of boards and fans from miner data
    let (num_boards, num_fans) = if let Some(data) = &miner.full_data {
        (data.hashboards.len(), data.fans.len())
    } else {
        (0, 0)
    };

    // Create file and write dynamic header
    let mut file = File::create(&file_path)?;

    // Base header
    let mut header = "Miner IP,MAC Address,Model,Firmware,Timestamp,Total Hashrate (TH/s),Power (W),Efficiency (W/TH),Avg Temperature (°C)".to_string();

    // Add board hashrate columns
    for i in 0..num_boards {
        header.push_str(&format!(",Board {} Hashrate", i));
    }

    // Add board temperature columns
    for i in 0..num_boards {
        header.push_str(&format!(",Board {} Temp", i));
    }

    // Add fan RPM columns
    for i in 1..=num_fans {
        header.push_str(&format!(",Fan {} RPM", i));
    }

    writeln!(file, "{}", header)?;

    Ok(RecordingState {
        file_path: file_path.to_string_lossy().to_string(),
        start_time: Instant::now(),
        row_count: 0,
        is_recording: true,
    })
}

pub fn append_data_point(
    recording: &mut RecordingState,
    miner: &MinerInfo,
) -> Result<(), std::io::Error> {
    if !recording.is_recording {
        return Ok(());
    }

    let data = match &miner.full_data {
        Some(d) => d,
        None => return Ok(()), // No data to record
    };

    let mut file = OpenOptions::new().append(true).open(&recording.file_path)?;

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

    // Extract total hashrate
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

    // Extract power
    let power = if let Some(wattage) = data.wattage {
        wattage.as_watts()
    } else {
        0.0
    };

    // Extract efficiency
    let efficiency = data.efficiency.unwrap_or(0.0);

    // Extract average temperature
    let avg_temp = if let Some(temp) = data.average_temperature {
        temp.as_celsius()
    } else {
        0.0
    };

    // Extract per-board hashrates (dynamic)
    let board_hashrates: Vec<f64> = data
        .hashboards
        .iter()
        .map(|board| {
            if let Some(hr) = &board.hashrate {
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
            }
        })
        .collect();

    // Extract per-board temperatures (dynamic)
    let board_temps: Vec<f64> = data
        .hashboards
        .iter()
        .map(|board| {
            board
                .board_temperature
                .map(|temp| temp.as_celsius())
                .unwrap_or(0.0)
        })
        .collect();

    // Extract fan speeds (dynamic)
    let fan_rpms: Vec<f64> = data
        .fans
        .iter()
        .map(|fan| {
            fan.rpm
                .map(|rpm| rpm.as_radians_per_second() * 60.0 / (2.0 * std::f64::consts::PI))
                .unwrap_or(0.0)
        })
        .collect();

    // Build CSV row dynamically
    let mut row = format!(
        "{},{},{},{},{},{:.2},{:.0},{:.2},{:.2}",
        miner.ip,
        data.mac
            .map(|m| m.to_string())
            .unwrap_or_else(|| "N/A".to_string()),
        miner.model,
        miner.firmware_version,
        timestamp,
        total_hashrate,
        power,
        efficiency,
        avg_temp
    );

    // Add board hashrates
    for hashrate in &board_hashrates {
        row.push_str(&format!(",{:.2}", hashrate));
    }

    // Add board temperatures
    for temp in &board_temps {
        row.push_str(&format!(",{:.2}", temp));
    }

    // Add fan RPMs
    for rpm in &fan_rpms {
        row.push_str(&format!(",{:.0}", rpm));
    }

    writeln!(file, "{}", row)?;

    recording.row_count += 1;
    Ok(())
}

pub fn stop_recording(recording: &mut RecordingState) {
    recording.is_recording = false;
}

pub fn export_recording(
    recording: &RecordingState,
    destination: &str,
) -> Result<(), std::io::Error> {
    std::fs::copy(&recording.file_path, destination)?;
    Ok(())
}

pub fn delete_recording(recording: &RecordingState) -> Result<(), std::io::Error> {
    std::fs::remove_file(&recording.file_path)?;
    Ok(())
}

// Helper extension for MinerInfo
impl MinerInfo {
    pub fn mac_address(&self) -> Option<String> {
        self.full_data
            .as_ref()
            .and_then(|d| d.mac.map(|m| m.to_string()))
    }
}
