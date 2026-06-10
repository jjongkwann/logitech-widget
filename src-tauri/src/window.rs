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

/// Desktop-widget z-order management (behavior probed on Win11 26200).
///
/// - **Stay at the bottom normally.** NOT via tao's `alwaysOnBottom` — tao
///   rewrites every z-change (whatever the source) back to HWND_BOTTOM in its
///   WM_WINDOWPOSCHANGING handler, which also defeats any Win+D counter-
///   measure. Our own subclass does the bottom-rewrite instead, gated on
///   DESKTOP_MODE, and additionally vetoes the (-32000,-32000) "minimize"
///   move that Show Desktop applies to every top-level window.
/// - **Win+D.** Show Desktop raises the desktop layer (Progman) above normal
///   windows, hiding a bottom window even when unminimized. A window CAN be
///   shown above the raised desktop (a topmost window is — verified
///   visually), but SetWindowPos(HWND_TOPMOST) on this window is silently
///   ineffective when issued from this process or any process it spawns,
///   while the identical call from an unrelated process works. As the best
///   available in-process primitive, an EVENT_SYSTEM_FOREGROUND hook detects
///   the desktop becoming foreground and activates the widget
///   (SwitchToThisWindow — the same mechanism that brings an app above the
///   desktop when its taskbar button is clicked); the next app focus sinks it
///   back to the bottom.
/// - Re-parenting into WorkerW (live-wallpaper "attach") was rejected: no
///   mouse input there (kills dragging), coordinates shift on multi-monitor
///   setups, and the window is destroyed when the shell recreates WorkerW.
#[cfg(windows)]
pub fn pin_to_desktop(window: &WebviewWindow) {
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, KillTimer, SetTimer, SetWindowPos,
        SwitchToThisWindow, EVENT_SYSTEM_FOREGROUND, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOSIZE, SWP_NOZORDER, WINDOWPOS, WINEVENT_OUTOFCONTEXT, WM_TIMER,
        WM_WINDOWPOSCHANGING,
    };

    static WIDGET: AtomicIsize = AtomicIsize::new(0);
    /// True while the desktop itself is the foreground (Win+D state).
    static DESKTOP_MODE: AtomicBool = AtomicBool::new(false);
    const SETTLE_TIMER: usize = 0x4C57; // "LW"

    fn apply_band() {
        let raw = WIDGET.load(Ordering::Relaxed);
        if raw == 0 {
            return;
        }
        let desktop = unsafe {
            let fg = GetForegroundWindow();
            let mut cls = [0u16; 32];
            let n = GetClassNameW(fg, &mut cls) as usize;
            let cls = String::from_utf16_lossy(&cls[..n]);
            cls == "Progman" || cls == "WorkerW"
        };
        DESKTOP_MODE.store(desktop, Ordering::Relaxed);
        unsafe {
            if desktop {
                SwitchToThisWindow(HWND(raw as _), true);
            } else {
                // Sink back; the subclass rewrites any z-change to bottom too.
                let _ = SetWindowPos(
                    HWND(raw as _),
                    Some(HWND_BOTTOM),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
    }

    unsafe extern "system" fn subclass_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        unsafe {
            match msg {
                WM_WINDOWPOSCHANGING => {
                    let pos = lparam.0 as *mut WINDOWPOS;
                    if !pos.is_null() {
                        let p = &mut *pos;
                        // Veto the Show Desktop "minimize" move.
                        if !p.flags.contains(SWP_NOMOVE) && p.x == -32000 && p.y == -32000 {
                            p.flags |= SWP_NOMOVE | SWP_NOSIZE;
                        }
                        // Hand-rolled always-on-bottom, suspended in desktop
                        // mode so the widget can sit above the raised desktop.
                        if !DESKTOP_MODE.load(Ordering::Relaxed)
                            && !p.flags.contains(SWP_NOZORDER)
                        {
                            p.hwndInsertAfter = HWND_BOTTOM;
                        }
                    }
                }
                WM_TIMER if wparam.0 == SETTLE_TIMER => {
                    let _ = KillTimer(Some(hwnd), SETTLE_TIMER);
                    apply_band();
                    return LRESULT(0);
                }
                _ => {}
            }
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
    }

    unsafe extern "system" fn on_foreground(
        _hook: HWINEVENTHOOK,
        _event: u32,
        _foreground: HWND,
        _id_object: i32,
        _id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        let raw = WIDGET.load(Ordering::Relaxed);
        if raw == 0 {
            return;
        }
        unsafe {
            // React now for responsiveness, and re-check after the shell's
            // transient staging windows settle (Win+D briefly focuses
            // ForegroundStaging before Progman, racing a single check).
            apply_band();
            SetTimer(Some(HWND(raw as _)), SETTLE_TIMER, 500, None);
        }
    }

    if let Ok(raw) = window.hwnd() {
        WIDGET.store(raw.0 as isize, Ordering::Relaxed);
        unsafe {
            let _ = SetWindowSubclass(HWND(raw.0), Some(subclass_proc), 1, 0);
            let _ = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(on_foreground),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
        }
        apply_band(); // initial placement: sink to bottom
    }
}

#[cfg(not(windows))]
pub fn pin_to_desktop(_window: &WebviewWindow) {}
