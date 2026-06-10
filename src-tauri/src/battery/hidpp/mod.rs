//! Primary battery source: direct HID++ communication over hidapi.
//! Protocol details: .claude/skills/hidpp-battery/SKILL.md.

pub mod diag;
mod proto;
mod transport;

use hidapi::HidApi;

use super::{BatterySource, DeviceBattery};
use proto::BatteryReading;
use transport::{PhysicalDevice, RpcError};

const SOURCE: &str = "hidpp";
const PING_MARKER: u8 = 0x5A;
/// Max paired-device slots on Unifying/Bolt/Lightspeed receivers.
const MAX_SLOTS: u8 = 6;

pub struct HidppSource {
    api: HidApi,
}

impl HidppSource {
    pub fn new() -> Result<Self, hidapi::HidError> {
        Ok(Self { api: HidApi::new()? })
    }
}

impl BatterySource for HidppSource {
    fn name(&self) -> &'static str {
        SOURCE
    }

    fn poll(&mut self) -> Vec<DeviceBattery> {
        if let Err(e) = self.api.refresh_devices() {
            eprintln!("hidpp: device refresh failed: {e}");
        }
        let mut out = Vec::new();
        for phys in transport::enumerate(&self.api) {
            poll_physical(&phys, &mut out);
        }
        dedup_stale_pairings(out)
    }
}

/// Re-pairing a device can leave a stale extra slot on the receiver with the
/// same codename. Collapse same-name entries on the same receiver: online
/// entries win over offline ones, and offline duplicates collapse to one.
fn dedup_stale_pairings(devices: Vec<DeviceBattery>) -> Vec<DeviceBattery> {
    let receiver_of = |id: &str| id.rsplit_once(':').map(|(r, _)| r.to_string());
    let mut out: Vec<DeviceBattery> = Vec::new();
    for d in devices {
        let existing = out
            .iter()
            .position(|e| e.name == d.name && receiver_of(&e.id) == receiver_of(&d.id));
        match existing {
            Some(_) if !d.online => {}                     // drop offline duplicate
            Some(i) if !out[i].online => out[i] = d,       // online beats offline
            Some(_) => out.push(d), // both online: two real identical devices
            None => out.push(d),
        }
    }
    out
}

/// Probe one physical device (receiver / wired / Bluetooth).
fn poll_physical(phys: &PhysicalDevice, out: &mut Vec<DeviceBattery>) {
    match phys.rpc(&proto::ping(proto::DEV_IDX_DIRECT, PING_MARKER)) {
        // HID++ 2.0 at 0xFF → direct-connected device (wired or Bluetooth).
        // Do NOT probe slots 1-6: some wired devices answer on every index.
        Ok(p) if proto::parse_pong(&p, PING_MARKER) => {
            if let Some(dev) = read_hidpp2_device(phys, proto::DEV_IDX_DIRECT, None) {
                out.push(dev);
            }
        }
        // HID++ 1.0 at 0xFF → a receiver; probe its paired-device slots.
        Err(RpcError::Hidpp10(proto::ERR_INVALID_SUBID)) => {
            for slot in 1..=MAX_SLOTS {
                poll_receiver_slot(phys, slot, out);
            }
        }
        // Asleep Bluetooth device (no receiver to answer for it) → offline entry.
        Err(e) if e.is_offline() => out.push(offline_entry(phys, proto::DEV_IDX_DIRECT, None)),
        Ok(_) | Err(_) => {}
    }
}

fn poll_receiver_slot(phys: &PhysicalDevice, slot: u8, out: &mut Vec<DeviceBattery>) {
    // Receiver-side codename works even when the device is asleep. Unifying/
    // Lightspeed and Bolt receivers use different register layouts — try both.
    let codename = phys
        .rpc(&proto::read_codename(slot))
        .ok()
        .and_then(|p| proto::parse_codename(&p))
        .or_else(|| {
            phys.rpc(&proto::read_codename_bolt(slot))
                .ok()
                .and_then(|p| proto::parse_codename_bolt(&p))
        });

    match phys.rpc(&proto::ping(slot, PING_MARKER)) {
        Ok(p) if proto::parse_pong(&p, PING_MARKER) => {
            if let Some(dev) = read_hidpp2_device(phys, slot, codename) {
                out.push(dev);
            }
        }
        Err(RpcError::Hidpp10(proto::ERR_INVALID_SUBID)) => {
            if let Some(dev) = read_hidpp1_device(phys, slot, codename) {
                out.push(dev);
            }
        }
        Err(e) if e.is_offline() => {
            // Only report slots we know are paired; empty slots have no codename.
            if codename.is_some() {
                out.push(offline_entry(phys, slot, codename));
            }
        }
        Ok(_) | Err(_) => {}
    }
}

/// HID++ 2.0: find the best battery feature (0x1004 → 0x1001 → 0x1000) and read it.
fn read_hidpp2_device(phys: &PhysicalDevice, idx: u8, codename: Option<String>) -> Option<DeviceBattery> {
    let reading = read_battery2(phys, idx)?;
    let (name, device_type) = read_name_type(phys, idx);
    Some(DeviceBattery {
        id: device_id(phys, idx),
        name: name.or(codename).unwrap_or_else(|| phys.product.clone()),
        device_type: device_type.unwrap_or("device").to_string(),
        percentage: reading.percentage,
        charging: reading.charging,
        online: true,
        source: SOURCE,
    })
}

fn read_battery2(phys: &PhysicalDevice, idx: u8) -> Option<BatteryReading> {
    let feature_index = |id: u16| -> Option<u8> {
        phys.rpc(&proto::get_feature(idx, id))
            .ok()
            .and_then(|p| proto::parse_feature_index(&p))
    };
    if let Some(fi) = feature_index(proto::FEAT_UNIFIED_BATTERY) {
        // function 1 = get_status
        let p = phys.rpc(&proto::short_msg(idx, fi, 0x01, [0, 0, 0])).ok()?;
        return Some(proto::parse_unified_battery(&p));
    }
    if let Some(fi) = feature_index(proto::FEAT_BATTERY_VOLTAGE) {
        let p = phys.rpc(&proto::short_msg(idx, fi, 0x00, [0, 0, 0])).ok()?;
        return Some(proto::parse_battery_voltage(&p));
    }
    if let Some(fi) = feature_index(proto::FEAT_BATTERY_STATUS) {
        let p = phys.rpc(&proto::short_msg(idx, fi, 0x00, [0, 0, 0])).ok()?;
        return Some(proto::parse_battery_status(&p));
    }
    None // no battery feature (e.g. a corded keyboard) → don't list
}

/// Feature 0x0005 DEVICE_NAME/TYPE. Best-effort: any failure falls back to None.
fn read_name_type(phys: &PhysicalDevice, idx: u8) -> (Option<String>, Option<&'static str>) {
    let Some(fi) = phys
        .rpc(&proto::get_feature(idx, proto::FEAT_DEVICE_NAME))
        .ok()
        .and_then(|p| proto::parse_feature_index(&p))
    else {
        return (None, None);
    };

    let name = (|| {
        let len = phys.rpc(&proto::short_msg(idx, fi, 0x00, [0, 0, 0])).ok()?[0] as usize;
        let len = len.min(64);
        let mut bytes = Vec::with_capacity(len);
        while bytes.len() < len {
            let chunk = phys
                .rpc(&proto::short_msg(idx, fi, 0x01, [bytes.len() as u8, 0, 0]))
                .ok()?;
            if chunk.is_empty() {
                return None;
            }
            bytes.extend_from_slice(&chunk[..chunk.len().min(len - bytes.len())]);
        }
        Some(String::from_utf8_lossy(&bytes).trim_end_matches('\0').to_string())
    })();

    let device_type = phys
        .rpc(&proto::short_msg(idx, fi, 0x02, [0, 0, 0]))
        .ok()
        .map(|p| proto::device_type_name(p[0]));

    (name.filter(|n| !n.is_empty()), device_type)
}

/// HID++ 1.0 paired device: battery registers 0x0D then 0x07.
fn read_hidpp1_device(phys: &PhysicalDevice, slot: u8, codename: Option<String>) -> Option<DeviceBattery> {
    let reading = phys
        .rpc(&proto::read_register(slot, proto::REG_BATTERY_MILEAGE))
        .map(|p| proto::parse_reg_mileage(&p))
        .or_else(|_| {
            phys.rpc(&proto::read_register(slot, proto::REG_BATTERY_STATUS))
                .map(|p| proto::parse_reg_status(&p))
        })
        .ok()?;
    Some(DeviceBattery {
        id: device_id(phys, slot),
        name: codename.unwrap_or_else(|| phys.product.clone()),
        device_type: "device".to_string(),
        percentage: reading.percentage,
        charging: reading.charging,
        online: true,
        source: SOURCE,
    })
}

fn offline_entry(phys: &PhysicalDevice, idx: u8, codename: Option<String>) -> DeviceBattery {
    DeviceBattery {
        id: device_id(phys, idx),
        name: codename.unwrap_or_else(|| phys.product.clone()),
        device_type: "device".to_string(),
        percentage: None,
        charging: false,
        online: false,
        source: SOURCE,
    }
}

fn device_id(phys: &PhysicalDevice, idx: u8) -> String {
    format!("hidpp:{}:{}", phys.key, idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(id: &str, name: &str, online: bool) -> DeviceBattery {
        DeviceBattery {
            id: id.to_string(),
            name: name.to_string(),
            device_type: "device".to_string(),
            percentage: online.then_some(50),
            charging: false,
            online,
            source: SOURCE,
        }
    }

    #[test]
    fn stale_pairing_slots_collapse() {
        // Real case: MX Master 3S paired at two Bolt slots, both asleep.
        let out = dedup_stale_pairings(vec![
            dev("hidpp:bolt:1", "MX KEYS S", false),
            dev("hidpp:bolt:2", "MX Master 3S", false),
            dev("hidpp:bolt:3", "MX Master 3S", false),
        ]);
        assert_eq!(out.len(), 2);

        // Online entry replaces the stale offline one.
        let out = dedup_stale_pairings(vec![
            dev("hidpp:bolt:2", "MX Master 3S", false),
            dev("hidpp:bolt:3", "MX Master 3S", true),
        ]);
        assert_eq!(out.len(), 1);
        assert!(out[0].online);

        // Same name on different receivers is not a duplicate.
        let out = dedup_stale_pairings(vec![
            dev("hidpp:a:1", "MX Master 3S", false),
            dev("hidpp:b:1", "MX Master 3S", false),
        ]);
        assert_eq!(out.len(), 2);
    }
}
