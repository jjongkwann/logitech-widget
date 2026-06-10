//! Diagnostic: list every Logitech HID interface visible to hidapi, with the
//! usage page/usage our enumeration filters on.
//!
//!     cargo run --example dump_hid --manifest-path src-tauri/Cargo.toml

fn main() {
    let api = hidapi::HidApi::new().expect("hidapi init");
    for d in api.device_list() {
        if d.vendor_id() != 0x046D {
            continue;
        }
        println!(
            "pid={:04x} usage_page={:04x} usage={:04x} if={} product={:?} path={}",
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.interface_number(),
            d.product_string().unwrap_or("?"),
            d.path().to_string_lossy(),
        );
    }
}
