pub mod ghub;
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

/// Merge a fallback source into the primary result. The same physical device
/// has different ids per source, so dedup falls back to a name heuristic:
/// normalized names where one contains the other are treated as one device,
/// and the primary (HID++) entry wins.
pub fn merge(mut primary: Vec<DeviceBattery>, fallback: Vec<DeviceBattery>) -> Vec<DeviceBattery> {
    for dev in fallback {
        if !primary.iter().any(|p| same_device(p, &dev)) {
            primary.push(dev);
        }
    }
    primary
}

/// Same device if the shorter name's tokens are all contained in the longer
/// one's (sources decorate names differently: HID++ says "PRO X 2", G HUB says
/// "PRO X SUPERLIGHT 2 Wireless Mouse"). Generic words don't count as evidence,
/// and clearly different device types never merge.
fn same_device(a: &DeviceBattery, b: &DeviceBattery) -> bool {
    if a.device_type != b.device_type && a.device_type != "device" && b.device_type != "device" {
        return false;
    }
    let (a, b) = (a.name.as_str(), b.name.as_str());
    const NOISE: &[&str] = &["wireless", "wired", "gaming", "mouse", "keyboard", "headset"];
    let tokens = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_ascii_alphanumeric())
            .map(|t| t.to_ascii_lowercase())
            .filter(|t| !t.is_empty() && !NOISE.contains(&t.as_str()))
            .collect()
    };
    let ta = tokens(a);
    let tb = tokens(b);
    if ta.is_empty() || tb.is_empty() {
        return false;
    }
    let (small, big) = if ta.len() <= tb.len() { (&ta, &tb) } else { (&tb, &ta) };
    small.iter().all(|t| big.contains(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, source: &'static str) -> DeviceBattery {
        DeviceBattery {
            id: format!("{source}:{name}"),
            name: name.to_string(),
            device_type: "mouse".to_string(),
            percentage: Some(50),
            charging: false,
            online: true,
            source,
        }
    }

    #[test]
    fn merge_prefers_primary_on_name_overlap() {
        // Real-world pair observed on hardware: HID++ 0x0005 name vs G HUB
        // extendedDisplayName for the same mouse.
        let merged = merge(
            vec![dev("PRO X 2", "hidpp")],
            vec![
                dev("PRO X SUPERLIGHT 2 Wireless Mouse", "ghub"),
                dev("G915 TKL", "ghub"),
            ],
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].source, "hidpp");
        assert_eq!(merged[1].name, "G915 TKL");
    }

    #[test]
    fn merge_keeps_distinct_devices() {
        let merged = merge(vec![dev("MX Master 3S", "hidpp")], vec![dev("G502 X", "ghub")]);
        assert_eq!(merged.len(), 2);
    }
}
