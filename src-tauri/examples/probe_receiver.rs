//! Diagnostic: raw-probe every receiver slot — ping plus both codename
//! register layouts (Unifying 0x40|n and Bolt 0x60|n pages of reg 0xB5).
//!
//!     cargo run --example probe_receiver --manifest-path src-tauri/Cargo.toml

use logitech_widget_lib::battery::hidpp::diag;

fn main() {
    diag::probe_all();
}
