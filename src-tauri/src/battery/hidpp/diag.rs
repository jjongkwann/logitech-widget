//! Diagnostic helpers for the probe_receiver example. Not used by the app.

use hidapi::HidApi;

use super::proto;
use super::transport::{self, RpcError};

pub fn probe_all() {
    let api = HidApi::new().expect("hidapi init");
    for phys in transport::enumerate(&api) {
        println!("== {} [{}]", phys.product, phys.key);
        let show = |label: &str, r: &Result<Vec<u8>, RpcError>| match r {
            Ok(p) => println!("   {label}: OK {p:02x?}"),
            Err(e) => println!("   {label}: ERR {e:?}"),
        };
        show("ping(0xFF)", &phys.rpc(&proto::ping(proto::DEV_IDX_DIRECT, 0x5A)));
        for slot in 1..=6u8 {
            let ping = phys.rpc(&proto::ping(slot, 0x5A));
            // Skip slots that error fast with "unknown device" to keep output short.
            if matches!(ping, Err(RpcError::Hidpp10(0x08))) {
                continue;
            }
            show(&format!("slot{slot} ping"), &ping);
            show(
                &format!("slot{slot} codename 0x40"),
                &phys.rpc(&proto::read_long_register(proto::DEV_IDX_DIRECT, proto::REG_PAIRING_INFO, 0x40 | (slot - 1))),
            );
            show(
                &format!("slot{slot} codename 0x60"),
                &phys.rpc(&proto::read_long_register(proto::DEV_IDX_DIRECT, proto::REG_PAIRING_INFO, 0x60 + slot)),
            );
        }
    }
}
