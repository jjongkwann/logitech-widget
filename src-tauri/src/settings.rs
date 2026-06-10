//! User settings, read once at startup from `settings.json` in the app config
//! dir (no UI yet — edit the file and restart). Missing file or fields fall
//! back to defaults.

use serde::Deserialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub poll_interval_secs: u64,
    /// Notify when an online, discharging device drops to this percentage.
    pub low_battery_threshold: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 30,
            low_battery_threshold: 15,
        }
    }
}

pub fn load(app: &AppHandle) -> Settings {
    let Ok(dir) = app.path().app_config_dir() else {
        return Settings::default();
    };
    let Ok(text) = std::fs::read_to_string(dir.join("settings.json")) else {
        return Settings::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|e| {
        eprintln!("settings.json invalid ({e}), using defaults");
        Settings::default()
    })
}
