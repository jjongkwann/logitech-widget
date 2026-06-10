//! hidapi I/O: enumeration of Logitech vendor HID collections and a synchronous
//! request/reply transaction primitive. All byte-level logic lives in `proto`.

use std::ffi::CStr;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use super::proto::{self, Reply};

pub const LOGITECH_VID: u16 = 0x046D;

/// Overall deadline for one request/reply transaction.
const RPC_TIMEOUT: Duration = Duration::from_millis(300);
/// Per-handle read poll slice.
const READ_SLICE_MS: i32 = 10;

#[derive(Debug)]
#[allow(dead_code)] // error payloads are read via Debug when logging
pub enum RpcError {
    /// HID++ 1.0 error reply (0x8F). For sleeping devices the receiver answers
    /// with 0x09 RESOURCE_ERROR / 0x08 UNKNOWN_DEVICE / 0x04 CONNECT_FAIL.
    Hidpp10(u8),
    /// HID++ 2.0 error reply (sub id 0xFF).
    Hidpp20(u8),
    /// No matching reply within the deadline (device asleep or absent).
    Timeout,
    Io(hidapi::HidError),
}

impl RpcError {
    /// Errors that mean "paired but unreachable right now".
    pub fn is_offline(&self) -> bool {
        matches!(self, RpcError::Timeout | RpcError::Hidpp10(0x04 | 0x08 | 0x09))
    }
}

/// One physical Logitech device (receiver, wired or Bluetooth device): the
/// openable vendor-page HID collections grouped together.
pub struct PhysicalDevice {
    /// Stable key derived from the Windows device path (collection bits removed).
    pub key: String,
    /// Product string from the USB/BT descriptor (fallback display name).
    pub product: String,
    short: Option<HidDevice>,
    long: Option<HidDevice>,
}

/// Group key: Windows splits one device into one path per HID collection, e.g.
/// `\\?\hid#vid_046d&pid_c52b&mi_02&col01#9&2f7a47&0&0000#{...}` — the col index
/// and the trailing ordinal are the only parts that differ between the short and
/// long collections of the same device.
fn group_key(path: &CStr) -> String {
    let p = path.to_string_lossy().to_lowercase();
    let p = regex_lite::Regex::new(r"&?col\d+").unwrap().replace_all(&p, "");
    regex_lite::Regex::new(r"&\d{4}#").unwrap().replace_all(&p, "#").into_owned()
}

/// Find all openable Logitech HID++ devices. Filters by VID + vendor-defined
/// usage page (0xFFxx); usage 0x01 = short channel, 0x02/0x0202 = long channel.
pub fn enumerate(api: &HidApi) -> Vec<PhysicalDevice> {
    let mut groups: Vec<PhysicalDevice> = Vec::new();
    for info in api.device_list() {
        if info.vendor_id() != LOGITECH_VID || info.usage_page() & 0xFF00 != 0xFF00 {
            continue;
        }
        let is_short = info.usage() == 0x0001;
        let is_long = info.usage() == 0x0002 || info.usage() == 0x0202;
        if !is_short && !is_long {
            continue;
        }
        let Ok(dev) = api.open_path(info.path()) else {
            continue; // opened exclusively by something else; skip
        };
        let key = group_key(info.path());
        let entry = match groups.iter_mut().find(|g| g.key == key) {
            Some(g) => g,
            None => {
                groups.push(PhysicalDevice {
                    key,
                    product: info.product_string().unwrap_or("Logitech device").to_string(),
                    short: None,
                    long: None,
                });
                groups.last_mut().unwrap()
            }
        };
        if is_short {
            entry.short.get_or_insert(dev);
        } else {
            entry.long.get_or_insert(dev);
        }
    }
    groups.retain(|g| g.short.is_some() || g.long.is_some());
    groups
}

impl PhysicalDevice {
    /// Send a short request and wait for the matching reply. On long-only
    /// devices (Bluetooth) the request is upgraded to a 20-byte report.
    /// Unrelated traffic (events, stale replies) is skipped, not treated as
    /// the answer.
    ///
    /// A short request that times out is retried once as a long report:
    /// Bolt receivers are BLE-based and only forward long messages to their
    /// paired devices (receiver-local register reads still answer short).
    pub fn rpc(&self, req: &[u8; proto::SHORT_LEN]) -> Result<Vec<u8>, RpcError> {
        match (&self.short, &self.long) {
            (Some(s), long) => {
                let result = self.transact(s, &req[..]);
                if matches!(result, Err(RpcError::Timeout)) {
                    if let Some(l) = long {
                        return self.transact(l, &proto::to_long(req)[..]);
                    }
                }
                result
            }
            (None, Some(l)) => self.transact(l, &proto::to_long(req)[..]),
            (None, None) => Err(RpcError::Timeout),
        }
    }

    /// Diagnostic: send a raw request and collect every report from both
    /// channels for `window_ms`. Used by the probe examples only.
    pub fn dump_traffic(&self, wire: &[u8], window_ms: u64) -> Result<Vec<Vec<u8>>, RpcError> {
        // Route by wire length: long frames must go out the long collection.
        let write_to = match (&self.short, &self.long) {
            (Some(s), _) if wire.len() == proto::SHORT_LEN => s,
            (_, Some(l)) => l,
            (Some(s), None) => s,
            (None, None) => return Err(RpcError::Timeout),
        };
        write_to.write(wire).map_err(RpcError::Io)?;
        let deadline = Instant::now() + Duration::from_millis(window_ms);
        let mut out = Vec::new();
        let mut buf = [0u8; 64];
        while Instant::now() < deadline {
            for ch in [self.short.as_ref(), self.long.as_ref()].into_iter().flatten() {
                let n = ch.read_timeout(&mut buf, READ_SLICE_MS).map_err(RpcError::Io)?;
                if n > 0 {
                    out.push(buf[..n].to_vec());
                }
            }
        }
        Ok(out)
    }

    fn transact(&self, write_to: &HidDevice, wire: &[u8]) -> Result<Vec<u8>, RpcError> {
        write_to.write(wire).map_err(RpcError::Io)?;

        let deadline = Instant::now() + RPC_TIMEOUT;
        let mut buf = [0u8; 64];
        while Instant::now() < deadline {
            for ch in [self.short.as_ref(), self.long.as_ref()].into_iter().flatten() {
                let n = ch.read_timeout(&mut buf, READ_SLICE_MS).map_err(RpcError::Io)?;
                if n == 0 {
                    continue;
                }
                match proto::match_reply(wire, &buf[..n]) {
                    Reply::Ok(params) => return Ok(params.to_vec()),
                    Reply::Err10(code) => return Err(RpcError::Hidpp10(code)),
                    Reply::Err20(code) => return Err(RpcError::Hidpp20(code)),
                    Reply::NoMatch => continue,
                }
            }
        }
        Err(RpcError::Timeout)
    }
}
