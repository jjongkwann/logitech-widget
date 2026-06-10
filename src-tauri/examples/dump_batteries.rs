//! Phase 1 verification harness: print battery state of all reachable
//! Logitech devices to the console.
//!
//!     cargo run --example dump_batteries --manifest-path src-tauri/Cargo.toml

use logitech_widget_lib::battery::{hidpp::HidppSource, BatterySource};

fn main() {
    let mut source = HidppSource::new().expect("hidapi init failed");
    let devices = source.poll();

    if devices.is_empty() {
        println!("no Logitech HID++ devices found");
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
