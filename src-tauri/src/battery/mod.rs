pub mod hidpp;

/// Normalized battery state shared by all sources (HID++, G HUB).
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceBattery {
    /// Stable identifier (per source) used for merging/dedup.
    pub id: String,
    pub name: String,
    pub device_type: String,
    /// None when the device doesn't report a percentage or is offline.
    pub percentage: Option<u8>,
    pub charging: bool,
    /// False when the device is paired but asleep/unreachable.
    pub online: bool,
    pub source: &'static str,
}

pub trait BatterySource {
    fn name(&self) -> &'static str;
    /// One polling sweep. Must never panic or block indefinitely.
    fn poll(&mut self) -> Vec<DeviceBattery>;
}
