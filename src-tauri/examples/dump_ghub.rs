//! Phase 4 verification harness: print battery state as seen by the G HUB
//! WebSocket source alone (no HID++).
//!
//!     cargo run --example dump_ghub --manifest-path src-tauri/Cargo.toml

use logitech_widget_lib::battery::{ghub::GHubSource, BatterySource};

fn main() {
    let devices = GHubSource.poll();
    if devices.is_empty() {
        println!("no devices via G HUB (is G HUB running?)");
        return;
    }
    for d in &devices {
        let pct = d
            .percentage
            .map(|p| format!("{p:3}%"))
            .unwrap_or_else(|| "  ?".to_string());
        let state = if !d.online {
            "offline"
        } else if d.charging {
            "charging"
        } else {
            "discharging"
        };
        println!("{pct}  {state:<12} {:<10} {}  [{}]", d.device_type, d.name, d.id);
    }
}
