---
name: tauri-widget
description: Tauri v2 recipes for this project's desktop-widget shell — transparent frameless overlay window, pin-to-desktop so it survives Win+D, system tray, autostart, Rust→webview event pattern, Windows build prerequisites. Read before changing window.rs, tray.rs, tauri.conf.json, or startup behavior.
---

# Tauri v2 desktop-widget shell (Windows)

## Overlay window config (`tauri.conf.json` → `app.windows[]`)

```json
{
  "label": "widget",
  "decorations": false,
  "transparent": true,
  "skipTaskbar": true,
  "alwaysOnBottom": true,
  "shadow": false,
  "resizable": false,
  "focus": false
}
```

- `shadow: false` only takes effect with `decorations: false` on Windows.
- `visibleOnAllWorkspaces` is unsupported on Windows — don't bother.
- Rust equivalents on `WebviewWindowBuilder`: `.decorations(false).transparent(true).skip_taskbar(true).always_on_bottom(true).shadow(false)`.
- Dragging a frameless window: `data-tauri-drag-region` attribute on the widget's root element (requires `core:window:allow-start-dragging` capability).

## Surviving Win+D (pin to desktop)

`alwaysOnBottom` alone does NOT survive Win+D — Show Desktop moves all top-level windows to (-32000,-32000).

Use **`tauri-plugin-wallpaper`** (github.com/meslzy/tauri-plugin-wallpaper, Windows-only; crate + npm package same name). Two modes:
- `pin(PinRequest::new("widget"))` — subclasses the window proc, blocks the `WM_WINDOWPOSCHANGING` move on Win+D. Widget stays above desktop icons. **← use this.**
- `attach(...)` — re-parents under WorkerW (behind desktop icons, wallpaper layer). Only if we ever want wallpaper-style rendering.

If the plugin ever breaks, the hand-rolled fallback is `window.hwnd()` + `windows` crate: `SetWindowSubclass`, intercept `WM_WINDOWPOSCHANGING` and zero out the move (or the classic Progman `SendMessageTimeout(0x052C)` → WorkerW `SetParent` technique — skip the WorkerW that contains `SHELLDLL_DefView`).

## System tray

- Cargo: `tauri = { version = "2", features = ["tray-icon"] }`
- In `.setup()`: `TrayIconBuilder::new().menu(&menu).show_menu_on_left_click(true).on_menu_event(...).build(app)?` with `tauri::menu::{Menu, MenuItem}` (e.g. `MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?`).
- Minimum menu: per-device battery lines (disabled items, updated on poll) / widget show·hide toggle / autostart toggle / quit.

## Autostart

- Add: `npm run tauri add autostart`
- Init: `tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)`; control via `app.autolaunch()` or JS `@tauri-apps/plugin-autostart`.
- Capabilities needed: `autostart:allow-enable`, `autostart:allow-disable`, `autostart:allow-is-enabled`.

## Rust → frontend data flow (the project standard)

Poll on the Rust side; push to the webview:

```rust
// poller task: tauri::async_runtime::spawn + tokio interval
use tauri::Emitter;
app_handle.emit("battery-update", &devices)?;   // Vec<DeviceBattery>
```

```ts
import { listen } from "@tauri-apps/api/event";
const unlisten = await listen<DeviceBattery[]>("battery-update", (e) => render(e.payload));
```

Do not `setInterval` + `invoke` from the frontend — connection/device state belongs in Rust. (`tauri::ipc::Channel` exists for high-throughput streams; overkill here.)

## Windows build prerequisites / scaffold

- VS C++ Build Tools ("Desktop development with C++"), WebView2 runtime (preinstalled on Win11), Rust `stable-msvc`, Node.js LTS.
- Scaffold: `npm create tauri-app@latest` · Dev: `npm run tauri dev` · Build: `npm run tauri build`.

## Known issues

- Transparency regressions on some setups in v2: tauri#8308; `decorations:false` + `shadow:false` title-bar artifact: tauri#14859. If the window shows a ghost titlebar or black background, check these first.
