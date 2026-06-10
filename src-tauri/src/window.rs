//! Overlay window behavior: pin-to-desktop (survive Win+D) and position
//! persistence. See .claude/skills/tauri-widget/SKILL.md.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

#[derive(Serialize, Deserialize)]
struct SavedPosition {
    x: i32,
    y: i32,
}

fn position_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("position.json"))
}

pub fn restore_position(app: &AppHandle, window: &WebviewWindow) {
    let Some(path) = position_file(app) else { return };
    let Ok(text) = std::fs::read_to_string(path) else { return };
    if let Ok(pos) = serde_json::from_str::<SavedPosition>(&text) {
        let _ = window.set_position(PhysicalPosition::new(pos.x, pos.y));
    }
}

pub fn save_position(app: &AppHandle, pos: &PhysicalPosition<i32>) {
    let Some(path) = position_file(app) else { return };
    let saved = SavedPosition { x: pos.x, y: pos.y };
    let _ = std::fs::write(path, serde_json::to_string(&saved).unwrap());
}

/// Keep the widget on the desktop when Win+D / "Show Desktop" fires.
/// Show Desktop "minimizes" every top-level window by moving it to
/// (-32000, -32000); subclass the window proc and veto exactly that move
/// (same technique as tauri-plugin-wallpaper's pin).
#[cfg(windows)]
pub fn pin_to_desktop(window: &WebviewWindow) {
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        SWP_NOMOVE, SWP_NOSIZE, WINDOWPOS, WM_WINDOWPOSCHANGING,
    };

    unsafe extern "system" fn veto_hide(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if msg == WM_WINDOWPOSCHANGING {
            let pos = lparam.0 as *mut WINDOWPOS;
            if !pos.is_null() {
                let p = unsafe { &mut *pos };
                if !p.flags.contains(SWP_NOMOVE) && p.x == -32000 && p.y == -32000 {
                    p.flags |= SWP_NOMOVE | SWP_NOSIZE;
                }
            }
        }
        unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
    }

    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let _ = SetWindowSubclass(HWND(hwnd.0), Some(veto_hide), 1, 0);
        }
    }
}

#[cfg(not(windows))]
pub fn pin_to_desktop(_window: &WebviewWindow) {}
