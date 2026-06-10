//! Diagnostic: raw-probe every receiver slot — ping plus both codename
//! register layouts (Unifying 0x40|n and Bolt 0x60|n pages of reg 0xB5).
//!
//!     cargo run --example probe_receiver --manifest-path src-tauri/Cargo.toml

use logitech_widget_lib::battery::hidpp::diag;

fn main() {
    if std::env::args().any(|a| a == "--listen") {
        diag::listen_arrivals();
    } else if let Some(slot) = std::env::args()
        .skip_while(|a| a != "--ping")
        .nth(1)
        .and_then(|s| s.parse().ok())
    {
        diag::ping_dump(slot);
    } else {
        diag::probe_all();
    }
}
