//! Diagnostic helpers for the probe_receiver example. Not used by the app.

use hidapi::HidApi;

use super::proto;
use super::transport::{self, RpcError};

/// Send a LONG ping to a specific slot on every receiver and dump all raw
/// traffic that follows — catches replies in unexpected formats.
pub fn ping_dump(slot: u8) {
    let api = HidApi::new().expect("hidapi init");
    let long = proto::to_long(&proto::ping(slot, 0x5A));
    for phys in transport::enumerate(&api) {
        println!("== {} [{}]", phys.product, phys.key);
        match phys.dump_traffic(&long, 2000) {
            Ok(reports) => {
                for r in reports {
                    println!("   {r:02x?}");
                }
            }
            Err(e) => println!("   error: {e:?}"),
        }
    }
}

/// Trigger a "fake device arrival" (write reg 0x02 = 0x02) on every receiver
/// and dump all raw traffic for a few seconds. 0x41 connection notifications
/// reveal the receiver's live view of each slot (byte4 & 0x40 = link DOWN).
pub fn listen_arrivals() {
    let api = HidApi::new().expect("hidapi init");
    for phys in transport::enumerate(&api) {
        println!("== {} [{}]", phys.product, phys.key);
        match phys.dump_traffic(&[0x10, 0xFF, 0x80, 0x02, 0x02, 0x00, 0x00], 3000) {
            Ok(reports) => {
                for r in reports {
                    let note = if r.len() >= 5 && r[2] == 0x41 {
                        let slot = r[1];
                        let down = r[4] & 0x40 != 0;
                        format!("  <- 0x41 slot {slot} link {}", if down { "DOWN" } else { "UP" })
                    } else {
                        String::new()
                    };
                    println!("   {r:02x?}{note}");
                }
            }
            Err(e) => println!("   error: {e:?}"),
        }
    }
}

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
