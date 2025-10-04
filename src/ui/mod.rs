pub mod detail;
pub mod scan_control;
pub mod stats;
pub mod table;

pub use detail::draw_miner_detail_modal;
pub use scan_control::{draw_scan_and_ranges_card, ScanControlState};
pub use stats::draw_stats_card;
pub use table::draw_miners_table;
