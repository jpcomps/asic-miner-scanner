# ASIC-RS Miner Scanner

A high-performance, real-time ASIC miner management and monitoring application built with Rust and egui. Scan, monitor, and control Bitcoin ASIC miners across your network with an intuitive GUI.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Features

- 🔍 **Network Scanning**: Automatically discover ASIC miners on your network with real-time progress tracking
- 📊 **Real-time Monitoring**: Live hashrate, temperature, power consumption, and efficiency metrics
- 📈 **Historical Data**: Track performance over time with interactive graphs showing timestamped data
- 📝 **Metrics Recording**: Record miner performance data to CSV files for long-term analysis
- 🎛️ **Remote Control**: Start, stop, and manage fault lights on miners
- 🔎 **Search & Filter**: Quickly find miners by IP, hostname, model, or pool
- 💾 **Saved Ranges**: Save and reuse IP ranges for quick scanning
- 🔄 **Auto-scan**: Automatically refresh miner data at configurable intervals
- ⚙️ **Configurable Refresh**: Adjust detail view refresh interval from 5-60 seconds
- 📱 **Web Interface**: One-click access to miner web interfaces
- 🎨 **Dark Theme**: Easy-on-the-eyes interface for long monitoring sessions

## Quick Start

### Prerequisites

- Rust 1.70 or higher
- Network access to ASIC miners

### Installation

```bash
git clone https://github.com/yourusername/asic-miner-scanner.git
cd asic-miner-scanner
cargo build --release
```

### Running

```bash
cargo run --release
```

## Usage

### Scanning for Miners

1. Enter your IP range (e.g., `10.0.81.0` to `10.0.81.255`)
2. Optionally save the range with a name for future use
3. Click "⟳ SCAN ALL" to discover miners
4. Miners will appear in the table as they're discovered

**Note:** Saved IP ranges are stored in `~/asic-miner-scanner/scanner_config.json` and persist between sessions.

### Monitoring Miners

- Click on any miner IP to open detailed information
- View real-time metrics including:
  - Hashrate (total and per-board)
  - Temperature (average and per-board)
  - Power consumption
  - Fan speeds
  - Pool information
  - Hashboard details

### Controlling Miners

**Individual Control:**
- Click a miner IP to open the detail modal
- Use the control buttons to:
  - Start/Stop mining
  - Toggle fault light
  - Open web interface
  - Manually refresh data

**Bulk Operations:**
- Select multiple miners using checkboxes
- Use bulk action buttons to:
  - Start all selected miners
  - Stop all selected miners
  - Toggle fault lights

### Recording Metrics

The metrics recording feature allows you to capture detailed performance data over time for analysis, troubleshooting, or compliance purposes.

**Starting a Recording:**
1. Open the detail view for a miner by clicking its IP address
2. Under "Metrics Recording", click "🔴 START RECORDING"
3. Data will be automatically collected every refresh interval (default: 10 seconds)
4. The recording status shows elapsed time and number of rows captured

**Stopping a Recording:**
1. Click "⏹ STOP RECORDING" to pause data collection
2. The recording is saved in `~/asic-miner-scanner/recordings/`
3. File format: `recording_<IP>_<Model>_<MAC>_<timestamp>.csv`

**Exporting Recordings:**
1. After stopping a recording, click "💾 EXPORT TO CSV"
2. Choose a destination and filename
3. The CSV file contains comprehensive metrics:
   - Timestamp, IP, MAC address, model, firmware
   - Total hashrate and per-board hashrates (TH/s)
   - Power consumption (W) and efficiency (W/TH)
   - Average temperature and per-board temperatures (°C)
   - Fan speeds (RPM)

**Use Cases:**
- **Performance Analysis**: Track hashrate stability over hours/days
- **Thermal Monitoring**: Identify temperature trends and cooling issues
- **Efficiency Studies**: Analyze power consumption patterns
- **Warranty Claims**: Document performance issues with timestamped data
- **Fleet Optimization**: Compare performance across multiple miners

**Tips:**
- Recordings continue even if the detail modal is closed
- You can record multiple miners simultaneously
- Lower refresh intervals (5s) provide more granular data
- **Important:** Temporary recording files are automatically deleted when closing the detail modal
- Make sure to export recordings before closing if you want to keep them
- CSV files can be imported into Excel, Google Sheets, or analysis tools

### Configurable Refresh Interval

Adjust how frequently the detail view updates:
1. Open any miner's detail view
2. Use the "Auto-refresh interval" slider (5-60 seconds)
3. Lower values provide near real-time updates
4. Higher values reduce network traffic and API load
5. The setting applies globally to all detail views

### Auto-scan

Enable auto-scan to automatically refresh miner data:
1. Check the "AUTO-SCAN" checkbox
2. Set the interval (default: 2 minutes)
3. Miners will be automatically re-scanned at the specified interval

## Architecture

### Project Structure

```
asic-miner-scanner/
├── src/
│   ├── main.rs              # App entry point & coordination
│   ├── models.rs            # Data structures & types
│   ├── config.rs            # Configuration save/load
│   ├── scanner.rs           # Network scanning logic
│   ├── recording.rs         # CSV metrics recording
│   └── ui/
│       ├── mod.rs           # UI module exports
│       ├── stats.rs         # Fleet overview component
│       ├── scan_control.rs  # Scan control panel
│       ├── table.rs         # Miners table component
│       └── detail.rs        # Detail modal component
├── logo.svg                 # Application logo
├── Cargo.toml              # Dependencies & configuration
└── README.md               # This file
```

### Architecture Overview

The application follows a modular architecture with clear separation of concerns:

#### Core Modules

**`models.rs`** - Data Models
- Defines all core data structures (`MinerInfo`, `ScanProgress`, etc.)
- Contains enums for sorting and UI state
- Defines constants used across the application

**`config.rs`** - Configuration Management
- Handles loading and saving of saved IP ranges
- Uses JSON serialization for persistent storage
- Single responsibility: config I/O operations

**`scanner.rs`** - Network Scanning
- Implements network discovery using `asic-rs` library
- Handles concurrent miner scanning with adaptive concurrency
- Manages scan progress updates
- Collects and structures miner data

**`recording.rs`** - Metrics Recording
- Manages CSV file creation and data appending
- Writes performance metrics to timestamped files
- Handles export and cleanup operations
- Stores recordings in `~/asic-miner-scanner/recordings/`

**`main.rs`** - Application Coordinator
- Entry point and app initialization
- Manages application state (`MinerScannerApp`)
- Coordinates between UI and business logic
- Handles the main update loop

#### UI Components (`ui/` module)

**`stats.rs`** - Fleet Overview
- Displays aggregate statistics
- Calculates totals and averages
- Renders the orange stats card

**`scan_control.rs`** - Scan Control Panel
- IP range input controls
- Saved ranges management
- Auto-scan configuration
- Scan progress display

**`table.rs`** - Miners Table
- Displays discovered miners in a sortable table
- Handles search/filter functionality
- Manages bulk operations
- Provides selection controls
- Shows scanning progress when no miners found

**`detail.rs`** - Detail Modal
- Shows comprehensive miner information
- Renders real-time graphs with timestamps (hashrate, temperature, efficiency, power)
- Provides individual miner controls
- Configurable auto-refresh interval (5-60 seconds)
- Integrated metrics recording controls

### Data Flow

```
User Input → UI Components → main.rs → scanner.rs → asic-rs library
                ↓                           ↓
         Update State               Fetch Miner Data
                ↓                           ↓
         Render UI ← Updated State ← Process Results
```

### State Management

- **Shared State**: Uses `Arc<Mutex<T>>` for thread-safe state sharing
- **Miners List**: Central list of discovered miners
- **Scan Progress**: Real-time scan status updates
- **History Data**: Per-miner historical metrics
- **UI State**: Sorting, selection, search queries

### Concurrency Model

- **Main Thread**: UI rendering (egui event loop)
- **Background Threads**: Network scanning, miner control operations
- **Tokio Runtime**: Async operations (asic-rs library calls)
- **Thread Safety**: Mutex-protected shared state

## Dependencies

### Core
- **eframe/egui** - Immediate mode GUI framework
- **asic-rs** - ASIC miner communication library
- **tokio** - Async runtime

### UI
- **egui_extras** - Additional egui widgets (tables)
- **egui_plot** - Plotting library for graphs
- **resvg/usvg** - SVG rendering
- **tiny-skia** - 2D graphics library

### Utilities
- **serde/serde_json** - Serialization
- **webbrowser** - Opening web interfaces
- **ipnetwork** - IP address handling
- **csv** - CSV file generation
- **chrono** - Timestamp handling
- **rfd** - Native file dialogs
- **dirs** - Cross-platform directory paths

## Optimization Settings

The scanner uses several optimizations for fast discovery:

- **Adaptive Concurrency**: Automatically adjusts concurrent connections
- **Reduced Timeouts**: 5-second identification timeout
- **Connection Retries**: 2 retry attempts for reliability
- **Port Checking**: Pre-checks ports before full connection

You can modify these in `src/scanner.rs`:

```rust
MinerFactory::new()
    .with_adaptive_concurrency()
    .with_identification_timeout_secs(5)  // Adjust timeout
    .with_connectivity_retries(2)         // Adjust retries
    .with_port_check(true)
    .scan_by_range(&range)
```

## File Locations

When you ship the binary, all configuration and data files are stored in the user's home directory:

**Configuration:**
- `~/asic-miner-scanner/scanner_config.json` - Saved IP ranges

**Recordings:**
- `~/asic-miner-scanner/recordings/` - CSV metric recordings
- Filename format: `recording_<IP>_<Model>_<MAC>_<timestamp>.csv`

**Cross-Platform Paths:**
- **Linux/macOS**: `/home/username/asic-miner-scanner/`
- **Windows**: `C:\Users\username\asic-miner-scanner\`

The application automatically creates these directories on first run, so no manual setup is required.

## Troubleshooting

### Miners Not Found
- Verify network connectivity to the miner subnet
- Check firewall rules (miners typically use ports 80, 4028)
- Ensure miners are powered on and connected
- Try increasing timeout in `scanner.rs`

### Slow Scanning
- Reduce the IP range
- Check your network bandwidth
- Adjust concurrency settings in `scanner.rs`

### UI Not Updating
- Check the auto-scan interval setting
- Manually refresh using the "🔄 Refresh" button
- Verify background threads are running (check console for errors)

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [asic-rs](https://github.com/AspectUnk/asic-rs) - ASIC miner library
- UI powered by [egui](https://github.com/emilk/egui) - Immediate mode GUI
- Icons and design inspired by mining dashboard best practices

## Support

- 🐛 Report bugs via [GitHub Issues](https://github.com/yourusername/asic-miner-scanner/issues)
- 💬 Discuss features in [GitHub Discussions](https://github.com/yourusername/asic-miner-scanner/discussions)
- 📧 Contact: your.email@example.com

---

**Happy Mining! ⛏️**
