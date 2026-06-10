//! Phase 4 verification harness: both sources merged exactly as the poller
//! does it — the same physical device must appear only once (hidpp wins).
//!
//!     cargo run --example dump_merged --manifest-path src-tauri/Cargo.toml

use logitech_widget_lib::battery::{ghub::GHubSource, hidpp::HidppSource, merge, BatterySource};

fn main() {
    let hidpp = HidppSource::new().map(|mut s| s.poll()).unwrap_or_default();
    let ghub = GHubSource.poll();
    println!("hidpp: {} device(s), ghub: {} device(s)", hidpp.len(), ghub.len());
    for d in merge(hidpp, ghub) {
        let pct = d
            .percentage
            .map(|p| format!("{p:3}%"))
            .unwrap_or_else(|| "  ?".to_string());
        println!("{pct}  charging={} online={} [{}] {} ({})", d.charging, d.online, d.source, d.name, d.device_type);
    }
}
