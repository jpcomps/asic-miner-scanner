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
        miner.ip, model_clean, miner.mac_address().unwrap_or_else(|| "unknown".to_string()), timestamp
    );

    let file_path = recordings_dir.join(filename);

    // Create file and write header
    let mut file = File::create(&file_path)?;
    writeln!(
        file,
        "Miner IP,MAC Address,Model,Firmware,Timestamp,Total Hashrate (TH/s),Power (W),Efficiency (W/TH),Avg Temperature (°C),Board 0 Hashrate,Board 1 Hashrate,Board 2 Hashrate,Board 3 Hashrate,Board 0 Temp,Board 1 Temp,Board 2 Temp,Board 3 Temp,Fan 1 RPM,Fan 2 RPM,Fan 3 RPM,Fan 4 RPM"
    )?;

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

    let mut file = OpenOptions::new()
        .append(true)
        .open(&recording.file_path)?;

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");

    // Extract total hashrate
    let total_hashrate = if let Some(hr) = &data.hashrate {
        let converted = hr.clone().as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash);
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

    // Extract per-board hashrates (up to 4 boards)
    let mut board_hashrates = vec![0.0; 4];
    for (i, board) in data.hashboards.iter().take(4).enumerate() {
        if let Some(hr) = &board.hashrate {
            let converted = hr.clone().as_unit(asic_rs::data::hashrate::HashRateUnit::TeraHash);
            let display_str = format!("{converted}");
            if let Some(val) = display_str.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                board_hashrates[i] = val;
            }
        }
    }

    // Extract per-board temperatures (up to 4 boards)
    let mut board_temps = vec![0.0; 4];
    for (i, board) in data.hashboards.iter().take(4).enumerate() {
        if let Some(temp) = board.board_temperature {
            board_temps[i] = temp.as_celsius();
        }
    }

    // Extract fan speeds (up to 4 fans)
    let mut fan_rpms = vec![0.0; 4];
    for (i, fan) in data.fans.iter().take(4).enumerate() {
        if let Some(rpm) = fan.rpm {
            let rpm_value = rpm.as_radians_per_second() * 60.0 / (2.0 * std::f64::consts::PI);
            fan_rpms[i] = rpm_value;
        }
    }

    // Write CSV row
    writeln!(
        file,
        "{},{},{},{},{},{:.2},{:.0},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.0},{:.0},{:.0},{:.0}",
        miner.ip,
        data.mac.map(|m| m.to_string()).unwrap_or_else(|| "N/A".to_string()),
        miner.model,
        miner.firmware_version,
        timestamp,
        total_hashrate,
        power,
        efficiency,
        avg_temp,
        board_hashrates[0],
        board_hashrates[1],
        board_hashrates[2],
        board_hashrates[3],
        board_temps[0],
        board_temps[1],
        board_temps[2],
        board_temps[3],
        fan_rpms[0],
        fan_rpms[1],
        fan_rpms[2],
        fan_rpms[3],
    )?;

    recording.row_count += 1;
    Ok(())
}

pub fn stop_recording(recording: &mut RecordingState) {
    recording.is_recording = false;
}

pub fn export_recording(recording: &RecordingState, destination: &str) -> Result<(), std::io::Error> {
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
        self.full_data.as_ref().and_then(|d| d.mac.map(|m| m.to_string()))
    }
}
