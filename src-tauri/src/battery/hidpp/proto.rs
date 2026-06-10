//! Pure HID++ message encoding/parsing. No I/O — everything here is unit-testable
//! against fixture bytes. Byte layouts follow Solaar, LGSTrayBattery and the Linux
//! kernel hid-logitech-hidpp driver (see .claude/skills/hidpp-battery/SKILL.md).

pub const REPORT_SHORT: u8 = 0x10;
pub const REPORT_LONG: u8 = 0x11;
pub const SHORT_LEN: usize = 7;
pub const LONG_LEN: usize = 20;

/// 4-bit software id echoed back in replies; must be nonzero (0 = device events).
pub const SW_ID: u8 = 0x0A;

/// Device index for the receiver itself or direct-connected (wired/BT) devices.
pub const DEV_IDX_DIRECT: u8 = 0xFF;

// HID++ 2.0 feature IDs (IRoot is always feature index 0x00, no lookup needed)
pub const FEAT_DEVICE_NAME: u16 = 0x0005;
pub const FEAT_BATTERY_STATUS: u16 = 0x1000;
pub const FEAT_BATTERY_VOLTAGE: u16 = 0x1001;
pub const FEAT_UNIFIED_BATTERY: u16 = 0x1004;

// HID++ 1.0 sub ids
pub const SUB_GET_REGISTER: u8 = 0x81;
pub const SUB_GET_LONG_REGISTER: u8 = 0x83;
pub const SUB_ERROR_1_0: u8 = 0x8F;
pub const SUB_ERROR_2_0: u8 = 0xFF;

// HID++ 1.0 registers
pub const REG_BATTERY_STATUS: u8 = 0x07;
pub const REG_BATTERY_MILEAGE: u8 = 0x0D;
pub const REG_PAIRING_INFO: u8 = 0xB5;

// HID++ 1.0 error codes (reply sub id 0x8F)
pub const ERR_INVALID_SUBID: u8 = 0x01;

/// Build a short (7-byte) HID++ 2.0 request: feature index + (function << 4 | SW_ID).
pub fn short_msg(dev_idx: u8, feat_idx: u8, fn_id: u8, params: [u8; 3]) -> [u8; SHORT_LEN] {
    [
        REPORT_SHORT,
        dev_idx,
        feat_idx,
        (fn_id << 4) | SW_ID,
        params[0],
        params[1],
        params[2],
    ]
}

/// Upgrade a short message to a long (20-byte) one — required on Bluetooth-direct
/// devices, which expose only the long-report collection.
pub fn to_long(short: &[u8; SHORT_LEN]) -> [u8; LONG_LEN] {
    let mut long = [0u8; LONG_LEN];
    long[0] = REPORT_LONG;
    long[1..SHORT_LEN].copy_from_slice(&short[1..]);
    long
}

// --- request builders -------------------------------------------------------

/// IRoot ping (function 1). `marker` is echoed at params[2] of the pong.
pub fn ping(dev_idx: u8, marker: u8) -> [u8; SHORT_LEN] {
    short_msg(dev_idx, 0x00, 0x01, [0, 0, marker])
}

/// IRoot getFeature (function 0): feature ID as big-endian 16-bit in params.
pub fn get_feature(dev_idx: u8, feature_id: u16) -> [u8; SHORT_LEN] {
    short_msg(dev_idx, 0x00, 0x00, [(feature_id >> 8) as u8, feature_id as u8, 0])
}

/// HID++ 1.0 short register read.
pub fn read_register(dev_idx: u8, reg: u8) -> [u8; SHORT_LEN] {
    [REPORT_SHORT, dev_idx, SUB_GET_REGISTER, reg, 0, 0, 0]
}

/// HID++ 1.0 long register read (reply comes back as a long report).
pub fn read_long_register(dev_idx: u8, reg: u8, p0: u8) -> [u8; SHORT_LEN] {
    [REPORT_SHORT, dev_idx, SUB_GET_LONG_REGISTER, reg, p0, 0, 0]
}

/// Receiver register 0xB5, page 0x40|n: codename of paired device at slot `idx` (1-based).
pub fn read_codename(idx: u8) -> [u8; SHORT_LEN] {
    read_long_register(DEV_IDX_DIRECT, REG_PAIRING_INFO, 0x40 | (idx - 1))
}

// --- reply matching ----------------------------------------------------------

#[derive(Debug, PartialEq)]
pub enum Reply<'a> {
    /// Successful reply; params start after the 4-byte header.
    Ok(&'a [u8]),
    /// HID++ 1.0 error (0x8F) — also what the receiver sends for sleeping devices.
    Err10(u8),
    /// HID++ 2.0 error (sub id 0xFF).
    Err20(u8),
    /// Unrelated traffic (event broadcast, reply to something else) — keep reading.
    NoMatch,
}

/// Match an incoming report against a pending request.
/// Matches on (device index, echoed bytes 2-3), never on report ID — replies may
/// come back on a different report length than the request.
pub fn match_reply<'a>(req: &[u8], buf: &'a [u8]) -> Reply<'a> {
    if buf.len() < 6 || (buf[0] != REPORT_SHORT && buf[0] != REPORT_LONG) {
        return Reply::NoMatch;
    }
    // Some Bluetooth devices echo device index 0x00 instead of 0xFF.
    let dev_ok = buf[1] == req[1] || (req[1] == DEV_IDX_DIRECT && buf[1] == 0x00);
    if !dev_ok {
        return Reply::NoMatch;
    }
    if buf[2] == SUB_ERROR_1_0 && buf[3] == req[2] && buf[4] == req[3] {
        return Reply::Err10(buf[5]);
    }
    if buf[2] == SUB_ERROR_2_0 && buf[3] == req[2] && buf[4] == req[3] {
        return Reply::Err20(buf[5]);
    }
    if buf[2] == req[2] && buf[3] == req[3] {
        return Reply::Ok(&buf[4..]);
    }
    Reply::NoMatch
}

// --- response parsers --------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatteryReading {
    pub percentage: Option<u8>,
    pub charging: bool,
}

/// IRoot getFeature reply → feature index (None if the feature is unsupported).
pub fn parse_feature_index(params: &[u8]) -> Option<u8> {
    match params[0] {
        0 => None,
        idx => Some(idx),
    }
}

/// Pong reply → echoed marker matches?
pub fn parse_pong(params: &[u8], marker: u8) -> bool {
    params.len() >= 3 && params[2] == marker
}

/// 0x1004 UNIFIED_BATTERY get_status (function 1):
/// p0 = state-of-charge %, p2 = 0 discharging / 1 charging / 2 charging-slow /
/// 3 full / 4 error.
pub fn parse_unified_battery(params: &[u8]) -> BatteryReading {
    BatteryReading {
        percentage: Some(params[0].min(100)),
        charging: matches!(params[2], 1..=3),
    }
}

/// 0x1000 BATTERY_STATUS function 0: p0 = % (0 ⇒ device doesn't report a
/// percentage), p2 = 0 discharging / 1-2 recharging / 3 full / 4 slow.
pub fn parse_battery_status(params: &[u8]) -> BatteryReading {
    BatteryReading {
        percentage: match params[0] {
            0 => None,
            p => Some(p.min(100)),
        },
        charging: matches!(params[2], 1..=4),
    }
}

/// 0x1001 BATTERY_VOLTAGE function 0: p0-p1 = mV big-endian, p2 bit7 = external
/// power (then bits0-2: 0 charging / 1 full / 2 error).
pub fn parse_battery_voltage(params: &[u8]) -> BatteryReading {
    let mv = u16::from_be_bytes([params[0], params[1]]);
    let flags = params[2];
    BatteryReading {
        percentage: Some(voltage_to_percent(mv)),
        charging: flags & 0x80 != 0 && (flags & 0x07) <= 1,
    }
}

/// Li-ion discharge curve from the Linux kernel hid-logitech-hidpp driver
/// (`hidpp20_map_battery_capacity`); index i = voltage floor for (100 - i)%.
const VOLTAGE_CURVE: [u16; 100] = [
    4186, 4156, 4143, 4133, 4122, 4113, 4103, 4094, 4086, 4075, //
    4067, 4059, 4051, 4043, 4035, 4027, 4019, 4011, 4003, 3997, //
    3989, 3983, 3976, 3969, 3961, 3955, 3949, 3942, 3935, 3929, //
    3922, 3916, 3909, 3902, 3896, 3890, 3883, 3877, 3870, 3865, //
    3859, 3853, 3848, 3842, 3837, 3833, 3828, 3824, 3819, 3815, //
    3811, 3808, 3804, 3800, 3797, 3793, 3790, 3787, 3784, 3781, //
    3778, 3775, 3772, 3770, 3767, 3764, 3762, 3759, 3757, 3754, //
    3751, 3748, 3744, 3741, 3737, 3734, 3730, 3726, 3724, 3720, //
    3717, 3714, 3710, 3706, 3702, 3697, 3693, 3688, 3683, 3677, //
    3671, 3666, 3662, 3658, 3654, 3646, 3633, 3612, 3579, 3537,
];

pub fn voltage_to_percent(mv: u16) -> u8 {
    for (i, &floor) in VOLTAGE_CURVE.iter().enumerate() {
        if mv >= floor {
            return (VOLTAGE_CURVE.len() - i) as u8;
        }
    }
    0
}

/// HID++ 1.0 register 0x0D (battery mileage): p0 = %, p2 & 0xF0 = 0x30
/// discharging / 0x50 recharging / 0x90 full.
pub fn parse_reg_mileage(params: &[u8]) -> BatteryReading {
    BatteryReading {
        percentage: Some(params[0].min(100)),
        charging: matches!(params[2] & 0xF0, 0x50 | 0x90),
    }
}

/// HID++ 1.0 register 0x07 (battery status): p0 = level code 7 full / 5 good /
/// 3 low / 1 critical (approximate percentages); p1 = charging codes.
pub fn parse_reg_status(params: &[u8]) -> BatteryReading {
    let percentage = match params[0] {
        7 => Some(90),
        5 => Some(50),
        3 => Some(20),
        1 => Some(5),
        _ => None,
    };
    BatteryReading {
        percentage,
        charging: matches!(params[1], 0x21 | 0x24 | 0x25 | 0x22 | 0x26),
    }
}

/// Receiver reg 0xB5 page 0x40|n reply: p0 echoes the page, p1 = name length,
/// name ASCII from p2.
pub fn parse_codename(params: &[u8]) -> Option<String> {
    let len = params.get(1).copied()? as usize;
    let bytes = params.get(2..)?;
    let len = len.min(bytes.len());
    if len == 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes[..len]).trim_end_matches('\0').to_string())
}

/// Feature 0x0005 function 2 (getDeviceType) → human-readable type.
pub fn device_type_name(code: u8) -> &'static str {
    match code {
        0 => "keyboard",
        1 => "remote",
        2 => "numpad",
        3 => "mouse",
        4 => "touchpad",
        5 => "trackball",
        6 => "presenter",
        7 => "receiver",
        _ => "device",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_msg_layout() {
        // get_feature(0x1004) on paired slot 1
        let m = get_feature(1, FEAT_UNIFIED_BATTERY);
        assert_eq!(m, [0x10, 0x01, 0x00, 0x00 | SW_ID, 0x10, 0x04, 0x00]);
    }

    #[test]
    fn ping_and_pong() {
        let req = ping(0xFF, 0x5A);
        assert_eq!(req, [0x10, 0xFF, 0x00, 0x10 | SW_ID, 0x00, 0x00, 0x5A]);
        // pong arrives as a short report echoing header + marker
        let pong = [0x10, 0xFF, 0x00, 0x10 | SW_ID, 0x04, 0x02, 0x5A];
        match match_reply(&req, &pong) {
            Reply::Ok(params) => assert!(parse_pong(params, 0x5A)),
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn to_long_pads() {
        let s = ping(0xFF, 1);
        let l = to_long(&s);
        assert_eq!(l[0], REPORT_LONG);
        assert_eq!(&l[1..7], &s[1..7]);
        assert!(l[7..].iter().all(|&b| b == 0));
    }

    #[test]
    fn hidpp10_error_detected() {
        // ping to a HID++ 1.0 receiver → 0x8F with INVALID_SUBID
        let req = ping(0xFF, 0x00);
        let err = [0x10, 0xFF, 0x8F, 0x00, 0x10 | SW_ID, 0x01, 0x00];
        assert_eq!(match_reply(&req, &err), Reply::Err10(ERR_INVALID_SUBID));
    }

    #[test]
    fn sleeping_device_resource_error() {
        let req = ping(0x02, 0x00);
        let err = [0x10, 0x02, 0x8F, 0x00, 0x10 | SW_ID, 0x09, 0x00];
        assert_eq!(match_reply(&req, &err), Reply::Err10(0x09));
    }

    #[test]
    fn unrelated_event_is_no_match() {
        let req = ping(0x01, 0x00);
        // 0x41 connection notification from the receiver
        let event = [0x10, 0x01, 0x41, 0x04, 0x61, 0x10, 0x40];
        assert_eq!(match_reply(&req, &event), Reply::NoMatch);
        // reply for another device index
        let other = [0x10, 0x02, 0x00, 0x10 | SW_ID, 0x04, 0x02, 0x00];
        assert_eq!(match_reply(&req, &other), Reply::NoMatch);
    }

    #[test]
    fn bt_zero_index_echo_accepted() {
        let req = ping(0xFF, 0x33);
        let pong = [0x11, 0x00, 0x00, 0x10 | SW_ID, 0x04, 0x05, 0x33];
        assert!(matches!(match_reply(&req, &pong), Reply::Ok(_)));
    }

    #[test]
    fn unified_battery_parsing() {
        // 73%, charging
        let r = parse_unified_battery(&[73, 0x02, 0x01, 0x01]);
        assert_eq!(r, BatteryReading { percentage: Some(73), charging: true });
        // 100%, full-on-cable counts as charging
        let r = parse_unified_battery(&[100, 0x08, 0x03, 0x01]);
        assert_eq!(r, BatteryReading { percentage: Some(100), charging: true });
        // discharging
        let r = parse_unified_battery(&[40, 0x04, 0x00, 0x00]);
        assert_eq!(r, BatteryReading { percentage: Some(40), charging: false });
    }

    #[test]
    fn battery_status_zero_percent_means_unsupported() {
        let r = parse_battery_status(&[0, 0, 0]);
        assert_eq!(r.percentage, None);
        let r = parse_battery_status(&[55, 50, 1]);
        assert_eq!(r, BatteryReading { percentage: Some(55), charging: true });
    }

    #[test]
    fn battery_voltage_parsing() {
        // 4186 mV discharging → 100%
        let r = parse_battery_voltage(&[0x10, 0x5A, 0x00]); // 0x105A = 4186
        assert_eq!(r, BatteryReading { percentage: Some(100), charging: false });
        // 3537 mV → 1%
        let r = parse_battery_voltage(&[0x0D, 0xD1, 0x00]); // 0x0DD1 = 3537
        assert_eq!(r, BatteryReading { percentage: Some(1), charging: false });
        // below curve → 0%
        assert_eq!(voltage_to_percent(3400), 0);
        // external power, charging
        let r = parse_battery_voltage(&[0x0F, 0x00, 0x80]);
        assert!(r.charging);
        // external power, charge complete
        let r = parse_battery_voltage(&[0x0F, 0x00, 0x81]);
        assert!(r.charging);
        // external power, charge error (bits 0-2 = 2) → not charging
        let r = parse_battery_voltage(&[0x0F, 0x00, 0x82]);
        assert!(!r.charging);
    }

    #[test]
    fn hidpp10_battery_registers() {
        let r = parse_reg_mileage(&[80, 0, 0x50]);
        assert_eq!(r, BatteryReading { percentage: Some(80), charging: true });
        let r = parse_reg_mileage(&[80, 0, 0x30]);
        assert!(!r.charging);
        let r = parse_reg_status(&[5, 0x00, 0]);
        assert_eq!(r.percentage, Some(50));
        let r = parse_reg_status(&[7, 0x25, 0]);
        assert!(r.charging);
    }

    #[test]
    fn codename_parsing() {
        // page echo, len=5, "M510\0..."
        let mut params = vec![0x40, 4];
        params.extend_from_slice(b"M510");
        params.extend_from_slice(&[0; 8]);
        assert_eq!(parse_codename(&params), Some("M510".to_string()));
        assert_eq!(parse_codename(&[0x40, 0, 0, 0]), None);
    }
}
