# ASIC-RS Miner Scanner

A high-performance, real-time ASIC miner management and monitoring application built with Rust and egui. Scan, monitor, and control Bitcoin ASIC miners across your network with an intuitive GUI.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

## Features

- 🔍 **Network Scanning**: Automatically discover ASIC miners on your network
- 📊 **Real-time Monitoring**: Live hashrate, temperature, power consumption, and efficiency metrics
- 📈 **Historical Data**: Track performance over time with interactive graphs
- 🎛️ **Remote Control**: Start, stop, and manage fault lights on miners
- 🔎 **Search & Filter**: Quickly find miners by IP, hostname, model, or pool
- 💾 **Saved Ranges**: Save and reuse IP ranges for quick scanning
- 🔄 **Auto-scan**: Automatically refresh miner data at configurable intervals
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

**`detail.rs`** - Detail Modal
- Shows comprehensive miner information
- Renders real-time graphs (hashrate, temperature, power)
- Provides individual miner controls
- Auto-refreshes every 10 seconds

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
